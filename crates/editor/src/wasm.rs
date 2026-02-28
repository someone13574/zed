//! Minimal wasm surface for the editor crate.
//!
//! Real editor functionality is currently native-only.

pub fn unsupported() -> &'static str {
    "editor is not yet implemented on wasm"
}
