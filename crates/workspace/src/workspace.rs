#[cfg(not(target_family = "wasm"))]
include!("workspace_native_impl.rs");

#[cfg(target_family = "wasm")]
include!("workspace_native_impl.rs");
