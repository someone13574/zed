#![cfg_attr(target_family = "wasm", no_main)]

use gpui::{
    App, Bounds, Context, FragmentShader, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, shader_element, size,
};
use gpui_platform::application;

struct ShaderExample {}

impl Render for ShaderExample {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().size_full().bg(rgb(0x202020)).child(
            shader_element(FragmentShader::new(
                "
            return vec4<f32>((position - bounds.origin) / bounds.size, 0.0, 1.0);
            ",
            ))
            .size_full(),
        )
    }
}

fn run_example() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ShaderExample {}),
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
