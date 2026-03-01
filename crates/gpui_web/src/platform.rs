use crate::dispatcher::WebDispatcher;
use crate::display::WebDisplay;
use crate::keyboard::WebKeyboardLayout;
use crate::window::WebWindow;
use anyhow::Result;
use futures::channel::oneshot;
use gpui::{
    Action, AnyWindowHandle, BackgroundExecutor, ClipboardItem, CursorStyle, DummyKeyboardMapper,
    ForegroundExecutor, Keymap, Menu, MenuItem, PathPromptOptions, Platform, PlatformDisplay,
    PlatformKeyboardLayout, PlatformKeyboardMapper, PlatformTextSystem, PlatformWindow, Task,
    ThermalState, WindowAppearance, WindowParams,
};
use gpui_wgpu::WgpuContext;
use std::{
    borrow::Cow,
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};

static BUNDLED_FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-Italic.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/ibm-plex-sans/IBMPlexSans-SemiBoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-Regular.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-Bold.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-Italic.ttf"),
    include_bytes!("../../../assets/fonts/lilex/Lilex-BoldItalic.ttf"),
];

pub struct WebPlatform {
    browser_window: web_sys::Window,
    background_executor: BackgroundExecutor,
    foreground_executor: ForegroundExecutor,
    text_system: Arc<dyn PlatformTextSystem>,
    active_window: RefCell<Option<AnyWindowHandle>>,
    active_display: Rc<dyn PlatformDisplay>,
    callbacks: RefCell<WebPlatformCallbacks>,
    wgpu_context: Rc<RefCell<Option<WgpuContext>>>,
}

#[derive(Default)]
struct WebPlatformCallbacks {
    open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    quit: Option<Box<dyn FnMut()>>,
    reopen: Option<Box<dyn FnMut()>>,
    app_menu_action: Option<Box<dyn FnMut(&dyn Action)>>,
    will_open_app_menu: Option<Box<dyn FnMut()>>,
    validate_app_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    keyboard_layout_change: Option<Box<dyn FnMut()>>,
    thermal_state_change: Option<Box<dyn FnMut()>>,
}

impl WebPlatform {
    pub fn new() -> Self {
        let browser_window =
            web_sys::window().expect("must be running in a browser window context");
        let dispatcher = Arc::new(WebDispatcher::new(browser_window.clone()));
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);
        let text_system = Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            "IBM Plex Sans",
        ));
        let fonts = BUNDLED_FONTS
            .iter()
            .map(|bytes| Cow::Borrowed(*bytes))
            .collect();
        if let Err(error) = text_system.add_fonts(fonts) {
            log::error!("failed to load bundled fonts: {error:#}");
        }
        let text_system: Arc<dyn PlatformTextSystem> = text_system;
        let active_display: Rc<dyn PlatformDisplay> =
            Rc::new(WebDisplay::new(browser_window.clone()));

        Self {
            browser_window,
            background_executor,
            foreground_executor,
            text_system,
            active_window: RefCell::new(None),
            active_display,
            callbacks: RefCell::new(WebPlatformCallbacks::default()),
            wgpu_context: Rc::new(RefCell::new(None)),
        }
    }
}

type PathPromptSender = Rc<RefCell<Option<oneshot::Sender<Result<Option<Vec<PathBuf>>>>>>>;
fn js_error_to_anyhow(error: JsValue) -> anyhow::Error {
    let message = error
        .as_string()
        .or_else(|| {
            js_sys::JSON::stringify(&error)
                .ok()
                .and_then(|value| value.as_string())
        })
        .unwrap_or_else(|| format!("{error:?}"));
    anyhow::anyhow!(message)
}

fn add_event_listener_once(target: &JsValue, event_name: &str, callback: &JsValue) -> Result<()> {
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"once".into(), &true.into()).map_err(js_error_to_anyhow)?;

    let add_event_listener = js_sys::Reflect::get(target, &"addEventListener".into())
        .map_err(js_error_to_anyhow)?
        .dyn_into::<js_sys::Function>()
        .map_err(|error| anyhow::anyhow!("addEventListener is not callable: {error:?}"))?;

    add_event_listener
        .call3(target, &event_name.into(), callback, &options)
        .map_err(js_error_to_anyhow)?;
    Ok(())
}

fn complete_path_prompt(
    sender: &PathPromptSender,
    input: &web_sys::HtmlInputElement,
    result: Result<Option<Vec<PathBuf>>>,
) {
    if let Some(sender) = sender.borrow_mut().take()
        && sender.send(result).is_err()
    {
        log::debug!("path prompt receiver dropped before result was delivered");
    }
    input.remove();
}

fn selected_paths_from_input(
    input: &web_sys::HtmlInputElement,
    select_directories: bool,
) -> Vec<PathBuf> {
    let Some(files) = input.files() else {
        return Vec::new();
    };

    if select_directories {
        let mut directories = Vec::new();
        for index in 0..files.length() {
            let Some(file) = files.get(index) else {
                continue;
            };

            let relative_path = js_sys::Reflect::get(file.as_ref(), &"webkitRelativePath".into())
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_default();

            let first_segment = relative_path
                .split('/')
                .next()
                .filter(|segment| !segment.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(file.name()));

            if !directories.iter().any(|path| path == &first_segment) {
                directories.push(first_segment);
            }
        }
        return directories;
    }

    let mut paths = Vec::new();
    for index in 0..files.length() {
        if let Some(file) = files.get(index) {
            paths.push(PathBuf::from(file.name()));
        }
    }
    paths
}

async fn read_files_from_input(
    input: &web_sys::HtmlInputElement,
) -> Result<Option<Vec<(PathBuf, Vec<u8>)>>> {
    let Some(files) = input.files() else {
        return Ok(None);
    };

    if files.length() == 0 {
        return Ok(None);
    }

    let mut result = Vec::new();
    for index in 0..files.length() {
        let Some(file) = files.get(index) else {
            continue;
        };

        let name = PathBuf::from(file.name());

        let array_buffer_promise = js_sys::Reflect::get(file.as_ref(), &"arrayBuffer".into())
            .map_err(js_error_to_anyhow)?
            .dyn_into::<js_sys::Function>()
            .map_err(|error| anyhow::anyhow!("arrayBuffer is not a function: {error:?}"))?
            .call0(file.as_ref())
            .map_err(js_error_to_anyhow)?
            .dyn_into::<js_sys::Promise>()
            .map_err(|error| anyhow::anyhow!("arrayBuffer did not return a Promise: {error:?}"))?;

        let array_buffer = wasm_bindgen_futures::JsFuture::from(array_buffer_promise)
            .await
            .map_err(js_error_to_anyhow)?;

        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        result.push((name, bytes));
    }

    Ok(Some(result))
}

async fn read_directory_files_from_input(
    input: &web_sys::HtmlInputElement,
) -> Result<Option<Vec<(PathBuf, Vec<u8>)>>> {
    let Some(files) = input.files() else {
        return Ok(None);
    };

    if files.length() == 0 {
        return Ok(None);
    }

    let mut result = Vec::new();
    for index in 0..files.length() {
        let Some(file) = files.get(index) else {
            continue;
        };

        let relative_path = js_sys::Reflect::get(file.as_ref(), &"webkitRelativePath".into())
            .ok()
            .and_then(|value| value.as_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(file.name()));

        let array_buffer_promise = js_sys::Reflect::get(file.as_ref(), &"arrayBuffer".into())
            .map_err(js_error_to_anyhow)?
            .dyn_into::<js_sys::Function>()
            .map_err(|error| anyhow::anyhow!("arrayBuffer is not a function: {error:?}"))?
            .call0(file.as_ref())
            .map_err(js_error_to_anyhow)?
            .dyn_into::<js_sys::Promise>()
            .map_err(|error| anyhow::anyhow!("arrayBuffer did not return a Promise: {error:?}"))?;

        let array_buffer = wasm_bindgen_futures::JsFuture::from(array_buffer_promise)
            .await
            .map_err(js_error_to_anyhow)?;

        let bytes = js_sys::Uint8Array::new(&array_buffer).to_vec();
        result.push((relative_path, bytes));
    }

    result.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(Some(result))
}

fn start_save_file_picker(
    browser_window: &web_sys::Window,
    suggested_name: &str,
) -> Result<js_sys::Promise> {
    let function = js_sys::Reflect::get(browser_window.as_ref(), &"showSaveFilePicker".into())
        .map_err(js_error_to_anyhow)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| anyhow::anyhow!("showSaveFilePicker is not available"))?;

    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"suggestedName".into(), &suggested_name.into())
        .map_err(js_error_to_anyhow)?;

    function
        .call1(browser_window.as_ref(), &options)
        .map_err(js_error_to_anyhow)?
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| anyhow::anyhow!("showSaveFilePicker did not return a Promise"))
}

async fn show_save_file_picker_handle(promise: js_sys::Promise) -> Result<Option<JsValue>> {
    match wasm_bindgen_futures::JsFuture::from(promise).await {
        Ok(handle) => Ok(Some(handle)),
        Err(error) => {
            let is_abort = js_sys::Reflect::get(&error, &"name".into())
                .ok()
                .and_then(|v| v.as_string())
                .is_some_and(|name| name == "AbortError");
            if is_abort {
                Ok(None)
            } else {
                Err(js_error_to_anyhow(error))
            }
        }
    }
}

async fn call_js_method(object: &JsValue, method: &str, arg: Option<&JsValue>) -> Result<JsValue> {
    let function = js_sys::Reflect::get(object, &method.into())
        .map_err(js_error_to_anyhow)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| anyhow::anyhow!("{method} is not a function"))?;
    let result = match arg {
        Some(arg) => function.call1(object, arg),
        None => function.call0(object),
    }
    .map_err(js_error_to_anyhow)?;
    let promise = result
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| anyhow::anyhow!("{method} did not return a Promise"))?;
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(js_error_to_anyhow)
}

async fn write_to_file_handle(handle: &JsValue, content: &[u8]) -> Result<()> {
    let array = js_sys::Uint8Array::from(content);
    let writable = call_js_method(handle, "createWritable", None).await?;
    call_js_method(&writable, "write", Some(&array)).await?;
    call_js_method(&writable, "close", None).await?;
    Ok(())
}

fn trigger_download(
    browser_window: &web_sys::Window,
    filename: &str,
    content: &[u8],
) -> Result<()> {
    let array = js_sys::Uint8Array::from(content);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(js_error_to_anyhow)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error_to_anyhow)?;
    let document = browser_window
        .document()
        .ok_or_else(|| anyhow::anyhow!("no document"))?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(js_error_to_anyhow)?
        .dyn_into()
        .map_err(|_| anyhow::anyhow!("created element is not an anchor"))?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.set_hidden(true);
    if let Some(body) = document.body() {
        body.append_child(&anchor).map_err(js_error_to_anyhow)?;
        anchor.click();
        anchor.remove();
    }
    web_sys::Url::revoke_object_url(&url).map_err(js_error_to_anyhow)?;
    Ok(())
}

impl Platform for WebPlatform {
    fn background_executor(&self) -> BackgroundExecutor {
        self.background_executor.clone()
    }

    fn foreground_executor(&self) -> ForegroundExecutor {
        self.foreground_executor.clone()
    }

    fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
        self.text_system.clone()
    }

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>) {
        let wgpu_context = self.wgpu_context.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match WgpuContext::new_web().await {
                Ok(context) => {
                    log::info!("WebGPU context initialized successfully");
                    *wgpu_context.borrow_mut() = Some(context);
                    on_finish_launching();
                }
                Err(err) => {
                    log::error!("Failed to initialize WebGPU context: {err:#}");
                    on_finish_launching();
                }
            }
        });
    }

    fn quit(&self) {
        log::warn!("WebPlatform::quit called, but quitting is not supported in the browser .");
    }

    fn restart(&self, _binary_path: Option<PathBuf>) {}

    fn activate(&self, _ignoring_other_apps: bool) {}

    fn hide(&self) {}

    fn hide_other_apps(&self) {}

    fn unhide_other_apps(&self) {}

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
        vec![self.active_display.clone()]
    }

    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        Some(self.active_display.clone())
    }

    fn active_window(&self) -> Option<AnyWindowHandle> {
        *self.active_window.borrow()
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        params: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>> {
        let context_ref = self.wgpu_context.borrow();
        let context = context_ref.as_ref().ok_or_else(|| {
            anyhow::anyhow!("WebGPU context not initialized. Was Platform::run() called?")
        })?;

        let window = WebWindow::new(handle, params, context, self.browser_window.clone())?;
        *self.active_window.borrow_mut() = Some(handle);
        Ok(Box::new(window))
    }

    fn window_appearance(&self) -> WindowAppearance {
        let Ok(Some(media_query)) = self
            .browser_window
            .match_media("(prefers-color-scheme: dark)")
        else {
            return WindowAppearance::Light;
        };
        if media_query.matches() {
            WindowAppearance::Dark
        } else {
            WindowAppearance::Light
        }
    }

    fn open_url(&self, url: &str) {
        if let Err(error) = self.browser_window.open_with_url(url) {
            log::warn!("Failed to open URL '{url}': {error:?}");
        }
    }

    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
        self.callbacks.borrow_mut().open_urls = Some(callback);
    }

    fn register_url_scheme(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Ok(()))
    }

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>> {
        let (sender, receiver) = oneshot::channel();

        if !options.files && !options.directories {
            if sender
                .send(Err(anyhow::anyhow!(
                    "path prompts require at least one of files/directories"
                )))
                .is_err()
            {
                log::debug!("path prompt receiver dropped before configuration error");
            }
            return receiver;
        }

        let Some(document) = self.browser_window.document() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_paths requires a browser document"
                )))
                .is_err()
            {
                log::debug!("path prompt receiver dropped before document error");
            }
            return receiver;
        };

        let input: web_sys::HtmlInputElement = match document
            .create_element("input")
            .map_err(|error| anyhow::anyhow!("failed to create file input: {error:?}"))
            .and_then(|element| {
                element
                    .dyn_into()
                    .map_err(|error| anyhow::anyhow!("created element is not an input: {error:?}"))
            }) {
            Ok(input) => input,
            Err(error) => {
                if sender.send(Err(error)).is_err() {
                    log::debug!("path prompt receiver dropped before input creation error");
                }
                return receiver;
            }
        };

        input.set_type("file");
        input.set_hidden(true);
        if options.multiple {
            input.set_multiple(true);
        }

        let select_directories = options.directories;
        if select_directories {
            input.set_webkitdirectory(true);
        }

        let Some(body) = document.body() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_paths requires a document body"
                )))
                .is_err()
            {
                log::debug!("path prompt receiver dropped before body lookup error");
            }
            return receiver;
        };

        if let Err(error) = body
            .append_child(&input)
            .map_err(|error| anyhow::anyhow!("failed to attach file input to body: {error:?}"))
        {
            if sender.send(Err(error)).is_err() {
                log::debug!("path prompt receiver dropped before append error");
            }
            return receiver;
        }

        let sender = Rc::new(RefCell::new(Some(sender)));

        let change_callback_input = input.clone();
        let change_callback_sender = sender.clone();
        let change_callback = Closure::once_into_js(move |_event: JsValue| {
            let paths = selected_paths_from_input(&change_callback_input, select_directories);
            let result = if paths.is_empty() {
                Ok(None)
            } else {
                Ok(Some(paths))
            };
            complete_path_prompt(&change_callback_sender, &change_callback_input, result);
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "change", &change_callback) {
            complete_path_prompt(&sender, &input, Err(error));
            return receiver;
        }

        let cancel_callback_input = input.clone();
        let cancel_callback_sender = sender.clone();
        let cancel_callback = Closure::once_into_js(move |_event: JsValue| {
            let delayed_input = cancel_callback_input.clone();
            let delayed_sender = cancel_callback_sender.clone();
            let delayed_callback = Closure::once_into_js(move || {
                let paths = selected_paths_from_input(&delayed_input, select_directories);
                let result = if paths.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(paths))
                };
                complete_path_prompt(&delayed_sender, &delayed_input, result);
            });

            if let Some(window) = web_sys::window() {
                let timeout_result = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    delayed_callback.unchecked_ref(),
                    150,
                );
                if timeout_result.is_ok() {
                    return;
                }
            }

            let paths = selected_paths_from_input(&cancel_callback_input, select_directories);
            let result = if paths.is_empty() {
                Ok(None)
            } else {
                Ok(Some(paths))
            };
            complete_path_prompt(&cancel_callback_sender, &cancel_callback_input, result);
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "cancel", &cancel_callback) {
            complete_path_prompt(&sender, &input, Err(error));
            return receiver;
        }

        if let Err(error) = input.show_picker() {
            log::debug!("show_picker failed, falling back to click(): {error:?}");
            input.click();
        }

        receiver
    }

    fn prompt_for_new_path(
        &self,
        _directory: &Path,
        _suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        unimplemented!("use save_file_as on web")
    }

    fn save_file_as(
        &self,
        _directory: &Path,
        suggested_name: Option<&str>,
        content: Arc<[u8]>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>> {
        let (sender, receiver) = oneshot::channel();
        let browser_window = self.browser_window.clone();
        let filename = suggested_name.unwrap_or("").to_owned();

        match start_save_file_picker(&browser_window, &filename) {
            Ok(promise) => {
                wasm_bindgen_futures::spawn_local(async move {
                    let result = match show_save_file_picker_handle(promise).await {
                        Ok(Some(handle)) => {
                            write_to_file_handle(&handle, &content).await.and_then(|_| {
                                let name = js_sys::Reflect::get(&handle, &"name".into())
                                    .map_err(js_error_to_anyhow)?
                                    .as_string()
                                    .ok_or_else(|| anyhow::anyhow!("file handle missing name"))?;
                                Ok(Some(PathBuf::from(name)))
                            })
                        }
                        Ok(None) => Ok(None),
                        Err(error) => Err(error),
                    };
                    if sender.send(result).is_err() {
                        log::debug!("save_file_as receiver dropped before completion");
                    }
                });
            }
            Err(_) => {
                let result = trigger_download(&browser_window, &filename, &content)
                    .map(|_| Some(PathBuf::from(&filename)));
                if sender.send(result).is_err() {
                    log::debug!("save_file_as receiver dropped before completion");
                }
            }
        }

        receiver
    }

    fn prompt_for_file_bytes(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<(PathBuf, Vec<u8>)>>>> {
        let (sender, receiver) = oneshot::channel();

        if !options.files {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_file_bytes only supports file selection on web"
                )))
                .is_err()
            {
                log::debug!("file bytes prompt receiver dropped before configuration error");
            }
            return receiver;
        }

        let Some(document) = self.browser_window.document() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_file_bytes requires a browser document"
                )))
                .is_err()
            {
                log::debug!("file bytes prompt receiver dropped before document error");
            }
            return receiver;
        };

        let input: web_sys::HtmlInputElement = match document
            .create_element("input")
            .map_err(|error| anyhow::anyhow!("failed to create file input: {error:?}"))
            .and_then(|element| {
                element
                    .dyn_into()
                    .map_err(|error| anyhow::anyhow!("created element is not an input: {error:?}"))
            }) {
            Ok(input) => input,
            Err(error) => {
                if sender.send(Err(error)).is_err() {
                    log::debug!("file bytes prompt receiver dropped before input creation error");
                }
                return receiver;
            }
        };

        input.set_type("file");
        input.set_hidden(true);
        if options.multiple {
            input.set_multiple(true);
        }

        let Some(body) = document.body() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_file_bytes requires a document body"
                )))
                .is_err()
            {
                log::debug!("file bytes prompt receiver dropped before body lookup error");
            }
            return receiver;
        };

        if let Err(error) = body
            .append_child(&input)
            .map_err(|error| anyhow::anyhow!("failed to attach file input to body: {error:?}"))
        {
            if sender.send(Err(error)).is_err() {
                log::debug!("file bytes prompt receiver dropped before append error");
            }
            return receiver;
        }

        let sender = Rc::new(RefCell::new(Some(sender)));

        let change_callback_input = input.clone();
        let change_callback_sender = sender.clone();
        let change_callback = Closure::once_into_js(move |_event: JsValue| {
            let input = change_callback_input.clone();
            let sender = change_callback_sender.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = read_files_from_input(&input).await;
                input.remove();
                if let Some(sender) = sender.borrow_mut().take()
                    && sender.send(result).is_err()
                {
                    log::debug!(
                        "file bytes prompt receiver dropped before result was delivered"
                    );
                }
            });
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "change", &change_callback) {
            if let Some(sender) = sender.borrow_mut().take() {
                sender.send(Err(error)).ok();
            }
            return receiver;
        }

        let cancel_callback_input = input.clone();
        let cancel_callback_sender = sender.clone();
        let cancel_callback = Closure::once_into_js(move |_event: JsValue| {
            let delayed_input = cancel_callback_input.clone();
            let delayed_sender = cancel_callback_sender.clone();
            let delayed_callback = Closure::once_into_js(move || {
                delayed_input.remove();
                if let Some(sender) = delayed_sender.borrow_mut().take()
                    && sender.send(Ok(None)).is_err()
                {
                    log::debug!(
                        "file bytes prompt receiver dropped before cancel was delivered"
                    );
                }
            });

            if let Some(window) = web_sys::window() {
                let timeout_result = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    delayed_callback.unchecked_ref(),
                    150,
                );
                if timeout_result.is_ok() {
                    return;
                }
            }

            cancel_callback_input.remove();
            if let Some(sender) = cancel_callback_sender.borrow_mut().take()
                && sender.send(Ok(None)).is_err()
            {
                log::debug!("file bytes prompt receiver dropped before cancel was delivered");
            }
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "cancel", &cancel_callback) {
            if let Some(sender) = sender.borrow_mut().take() {
                sender.send(Err(error)).ok();
            }
            return receiver;
        }

        if let Err(error) = input.show_picker() {
            log::debug!("show_picker failed, falling back to click(): {error:?}");
            input.click();
        }

        receiver
    }

    fn prompt_for_directory(
        &self,
    ) -> oneshot::Receiver<Result<Option<Vec<(PathBuf, Vec<u8>)>>>> {
        let (sender, receiver) = oneshot::channel();

        let Some(document) = self.browser_window.document() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_directory requires a browser document"
                )))
                .is_err()
            {
                log::debug!("directory prompt receiver dropped before document error");
            }
            return receiver;
        };

        let input: web_sys::HtmlInputElement = match document
            .create_element("input")
            .map_err(|error| anyhow::anyhow!("failed to create file input: {error:?}"))
            .and_then(|element| {
                element
                    .dyn_into()
                    .map_err(|error| anyhow::anyhow!("created element is not an input: {error:?}"))
            }) {
            Ok(input) => input,
            Err(error) => {
                if sender.send(Err(error)).is_err() {
                    log::debug!("directory prompt receiver dropped before input creation error");
                }
                return receiver;
            }
        };

        input.set_type("file");
        input.set_hidden(true);
        input.set_webkitdirectory(true);
        input.set_multiple(true);

        let Some(body) = document.body() else {
            if sender
                .send(Err(anyhow::anyhow!(
                    "prompt_for_directory requires a document body"
                )))
                .is_err()
            {
                log::debug!("directory prompt receiver dropped before body lookup error");
            }
            return receiver;
        };

        if let Err(error) = body
            .append_child(&input)
            .map_err(|error| anyhow::anyhow!("failed to attach file input to body: {error:?}"))
        {
            if sender.send(Err(error)).is_err() {
                log::debug!("directory prompt receiver dropped before append error");
            }
            return receiver;
        }

        let sender = Rc::new(RefCell::new(Some(sender)));

        let change_callback_input = input.clone();
        let change_callback_sender = sender.clone();
        let change_callback = Closure::once_into_js(move |_event: JsValue| {
            let input = change_callback_input.clone();
            let sender = change_callback_sender.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = read_directory_files_from_input(&input).await;
                input.remove();
                if let Some(sender) = sender.borrow_mut().take()
                    && sender.send(result).is_err()
                {
                    log::debug!(
                        "directory prompt receiver dropped before result was delivered"
                    );
                }
            });
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "change", &change_callback) {
            if let Some(sender) = sender.borrow_mut().take() {
                sender.send(Err(error)).ok();
            }
            return receiver;
        }

        let cancel_callback_input = input.clone();
        let cancel_callback_sender = sender.clone();
        let cancel_callback = Closure::once_into_js(move |_event: JsValue| {
            let delayed_input = cancel_callback_input.clone();
            let delayed_sender = cancel_callback_sender.clone();
            let delayed_callback = Closure::once_into_js(move || {
                delayed_input.remove();
                if let Some(sender) = delayed_sender.borrow_mut().take()
                    && sender.send(Ok(None)).is_err()
                {
                    log::debug!(
                        "directory prompt receiver dropped before cancel was delivered"
                    );
                }
            });

            if let Some(window) = web_sys::window() {
                let timeout_result = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    delayed_callback.unchecked_ref(),
                    150,
                );
                if timeout_result.is_ok() {
                    return;
                }
            }

            cancel_callback_input.remove();
            if let Some(sender) = cancel_callback_sender.borrow_mut().take()
                && sender.send(Ok(None)).is_err()
            {
                log::debug!("directory prompt receiver dropped before cancel was delivered");
            }
        });

        if let Err(error) = add_event_listener_once(input.as_ref(), "cancel", &cancel_callback) {
            if let Some(sender) = sender.borrow_mut().take() {
                sender.send(Err(error)).ok();
            }
            return receiver;
        }

        if let Err(error) = input.show_picker() {
            log::debug!("show_picker failed, falling back to click(): {error:?}");
            input.click();
        }

        receiver
    }

    fn can_select_mixed_files_and_dirs(&self) -> bool {
        false
    }

    fn reveal_path(&self, _path: &Path) {}

    fn open_with_system(&self, _path: &Path) {}

    fn on_quit(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().quit = Some(callback);
    }

    fn on_reopen(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().reopen = Some(callback);
    }

    fn set_menus(&self, _menus: Vec<Menu>, _keymap: &Keymap) {}

    fn set_dock_menu(&self, _menu: Vec<MenuItem>, _keymap: &Keymap) {}

    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>) {
        self.callbacks.borrow_mut().app_menu_action = Some(callback);
    }

    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().will_open_app_menu = Some(callback);
    }

    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>) {
        self.callbacks.borrow_mut().validate_app_menu_command = Some(callback);
    }

    fn thermal_state(&self) -> ThermalState {
        ThermalState::Nominal
    }

    fn on_thermal_state_change(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().thermal_state_change = Some(callback);
    }

    fn compositor_name(&self) -> &'static str {
        "Web"
    }

    fn app_path(&self) -> Result<PathBuf> {
        Err(anyhow::anyhow!("app_path is not available on the web"))
    }

    fn path_for_auxiliary_executable(&self, _name: &str) -> Result<PathBuf> {
        Err(anyhow::anyhow!(
            "path_for_auxiliary_executable is not available on the web"
        ))
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let css_cursor = match style {
            CursorStyle::Arrow => "default",
            CursorStyle::IBeam => "text",
            CursorStyle::Crosshair => "crosshair",
            CursorStyle::ClosedHand => "grabbing",
            CursorStyle::OpenHand => "grab",
            CursorStyle::PointingHand => "pointer",
            CursorStyle::ResizeLeft | CursorStyle::ResizeRight | CursorStyle::ResizeLeftRight => {
                "ew-resize"
            }
            CursorStyle::ResizeUp | CursorStyle::ResizeDown | CursorStyle::ResizeUpDown => {
                "ns-resize"
            }
            CursorStyle::ResizeUpLeftDownRight => "nesw-resize",
            CursorStyle::ResizeUpRightDownLeft => "nwse-resize",
            CursorStyle::ResizeColumn => "col-resize",
            CursorStyle::ResizeRow => "row-resize",
            CursorStyle::IBeamCursorForVerticalLayout => "vertical-text",
            CursorStyle::OperationNotAllowed => "not-allowed",
            CursorStyle::DragLink => "alias",
            CursorStyle::DragCopy => "copy",
            CursorStyle::ContextualMenu => "context-menu",
            CursorStyle::None => "none",
        };

        if let Some(document) = self.browser_window.document() {
            if let Some(body) = document.body() {
                if let Err(error) = body.style().set_property("cursor", css_cursor) {
                    log::warn!("Failed to set cursor style: {error:?}");
                }
            }
        }
    }

    fn should_auto_hide_scrollbars(&self) -> bool {
        true
    }

    fn read_from_clipboard(&self) -> Option<ClipboardItem> {
        None
    }

    fn write_to_clipboard(&self, _item: ClipboardItem) {}

    fn write_credentials(&self, _url: &str, _username: &str, _password: &[u8]) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "credential storage is not available on the web"
        )))
    }

    fn read_credentials(&self, _url: &str) -> Task<Result<Option<(String, Vec<u8>)>>> {
        Task::ready(Ok(None))
    }

    fn delete_credentials(&self, _url: &str) -> Task<Result<()>> {
        Task::ready(Err(anyhow::anyhow!(
            "credential storage is not available on the web"
        )))
    }

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout> {
        Box::new(WebKeyboardLayout)
    }

    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper> {
        Rc::new(DummyKeyboardMapper)
    }

    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
        self.callbacks.borrow_mut().keyboard_layout_change = Some(callback);
    }
}
