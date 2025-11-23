use gpui::{
    App, AppContext, Application, Bounds, ParentElement, Render, Styled, WindowBounds,
    WindowOptions, deferred, div, px, relative, rgb, size,
};

struct Root {}

impl Render for Root {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .flex()
            .size_full()
            .bg(rgb(0x202020))
            .items_center()
            .justify_center()
            .child(deferred(
                div()
                    .flex()
                    .bg(gpui::red())
                    .size(relative(0.75))
                    .items_center()
                    .justify_center()
                    .child(deferred("Deferred"))
                    .child(div().bg(gpui::blue()).size_20().absolute()),
            ))
            .child(div().size_full().bg(gpui::green()).absolute())
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.0), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| cx.new(|_| Root {}),
        )
        .unwrap();
    });
}
