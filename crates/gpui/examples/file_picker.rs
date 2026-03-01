#![cfg_attr(target_family = "wasm", no_main)]

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    App, Bounds, ClickEvent, Context, Image, ImageFormat, Task, Window, WindowBounds,
    WindowOptions, div, img, prelude::*, px, rgb, size,
};
use gpui_platform::application;

fn image_format_for_path(path: &PathBuf) -> Option<ImageFormat> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        "bmp" => Some(ImageFormat::Bmp),
        _ => None,
    }
}

enum FileContent {
    Empty,
    Text(String),
    Image(Arc<Image>),
}

struct FilePicker {
    folder_files: Vec<(PathBuf, Vec<u8>)>,
    content: FileContent,
    current_path: Option<PathBuf>,
    _tasks: Vec<Task<()>>,
}

impl FilePicker {
    fn new() -> Self {
        Self {
            folder_files: Vec::new(),
            content: FileContent::Empty,
            current_path: None,
            _tasks: Vec::new(),
        }
    }

    fn open_file(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_file_bytes(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });

        let task = cx.spawn(async move |this, cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            let Some(mut files) = result.ok().flatten() else {
                return;
            };
            let Some((path, bytes)) = files.pop() else {
                return;
            };
            FilePicker::display_file(this, path, bytes, cx);
        });
        self._tasks.push(task);
    }

    fn open_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_directory();

        let task = cx.spawn(async move |this, cx| {
            let Ok(result) = receiver.await else {
                return;
            };
            let Some(files) = result.ok().flatten() else {
                return;
            };

            this.update(cx, |picker, cx| {
                picker.folder_files = files;
                cx.notify();
            })
            .ok();
        });
        self._tasks.push(task);
    }

    fn save_file(&mut self, cx: &mut Context<Self>) {
        let FileContent::Text(text) = &self.content else {
            return;
        };
        let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
        let directory = self
            .current_path
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let suggested_name = self
            .current_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let receiver = cx.save_file_as(&directory, suggested_name.as_deref(), bytes);

        let task = cx.spawn(async move |_this, _cx| {
            receiver.await.ok();
        });
        self._tasks.push(task);
    }

    fn display_file(
        this: gpui::WeakEntity<Self>,
        path: PathBuf,
        bytes: Vec<u8>,
        cx: &mut gpui::AsyncApp,
    ) {
        let content = if let Some(format) = image_format_for_path(&path) {
            FileContent::Image(Arc::new(Image::from_bytes(format, bytes)))
        } else {
            FileContent::Text(String::from_utf8_lossy(&bytes).into_owned())
        };

        this.update(cx, |picker, cx| {
            picker.current_path = Some(path);
            picker.content = content;
            cx.notify();
        })
        .ok();
    }

    fn load_file_from_list(&mut self, path: PathBuf, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let content = if let Some(format) = image_format_for_path(&path) {
            FileContent::Image(Arc::new(Image::from_bytes(format, bytes)))
        } else {
            FileContent::Text(String::from_utf8_lossy(&bytes).into_owned())
        };
        self.current_path = Some(path);
        self.content = content;
        cx.notify();
    }
}

fn button(
    label: &str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(label.to_string())
        .px_3()
        .py_1()
        .bg(rgb(0xf0f0f0))
        .border_1()
        .border_color(rgb(0xcccccc))
        .rounded_sm()
        .cursor_pointer()
        .hover(|this| this.bg(rgb(0xe0e0e0)))
        .active(|this| this.opacity(0.85))
        .child(label.to_string())
        .on_click(on_click)
}

impl Render for FilePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let can_save = matches!(&self.content, FileContent::Text(_));

        let toolbar = div()
            .flex()
            .flex_row()
            .gap_2()
            .p_2()
            .border_b_1()
            .border_color(rgb(0xdddddd))
            .child(button(
                "Open File",
                cx.listener(|this, _event, _window, cx| {
                    this.open_file(cx);
                }),
            ))
            .child(button(
                "Open Folder",
                cx.listener(|this, _event, _window, cx| {
                    this.open_folder(cx);
                }),
            ))
            .when(can_save, |div| {
                div.child(button(
                    "Save",
                    cx.listener(|this, _event, _window, cx| {
                        this.save_file(cx);
                    }),
                ))
            });

        let show_sidebar = !self.folder_files.is_empty();
        let folder_files = self.folder_files.clone();

        let sidebar = div()
            .id("sidebar")
            .flex_none()
            .w(px(200.))
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(rgb(0xdddddd))
            .children(folder_files.into_iter().map(|(path, bytes)| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                div()
                    .id(path.to_string_lossy().to_string())
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .hover(|this| this.bg(rgb(0xf0f0f0)))
                    .text_sm()
                    .child(name)
                    .on_click(cx.listener(move |this, _event: &ClickEvent, _, cx| {
                        this.load_file_from_list(path.clone(), bytes.clone(), cx);
                    }))
            }));

        let content_area = div()
            .id("content")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .p_4()
            .child(match &self.content {
                FileContent::Empty => div()
                    .text_color(rgb(0x999999))
                    .child("Open a file or folder to get started.")
                    .into_any_element(),
                FileContent::Text(text) => div()
                    .font_family("monospace")
                    .text_sm()
                    .child(text.clone())
                    .into_any_element(),
                FileContent::Image(image) => div()
                    .size_full()
                    .flex()
                    .justify_center()
                    .child(img(image.clone()).max_w_full())
                    .into_any_element(),
            });

        let body = div()
            .flex_1()
            .flex()
            .flex_row()
            .overflow_hidden()
            .when(show_sidebar, |div| div.child(sidebar))
            .child(content_area);

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x333333))
            .child(toolbar)
            .child(body)
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(800.), px(600.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| FilePicker::new()),
        )
        .unwrap();
        cx.activate(true);
    });
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    run_example();
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    gpui_platform::web_init();
    run_example();
}
