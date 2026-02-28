#![allow(rustdoc::private_intra_doc_links)]

#[cfg(not(target_family = "wasm"))]
include!("editor_native_impl.rs");

#[cfg(target_family = "wasm")]
include!("editor_native_impl.rs");
