//! Minimal wasm surface for the project crate.
//!
//! Real project functionality is currently native-only.

pub fn unsupported() -> &'static str {
    "project is not yet implemented on wasm"
}
