#[cfg(not(target_family = "wasm"))]
include!("project_native_impl.rs");

#[cfg(target_family = "wasm")]
include!("project_native_impl.rs");
