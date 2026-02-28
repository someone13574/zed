#[cfg(not(target_family = "wasm"))]
include!("lsp_native_impl.rs");

#[cfg(target_family = "wasm")]
mod wasm;

#[cfg(target_family = "wasm")]
pub use wasm::*;
