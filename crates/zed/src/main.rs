#![cfg_attr(target_family = "wasm", no_main)]

#[cfg(not(target_family = "wasm"))]
include!("main_native_impl.rs");

#[cfg(target_family = "wasm")]
mod main_wasm;
