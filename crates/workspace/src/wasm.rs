//! Minimal wasm surface for the workspace crate.
//!
//! Real workspace functionality is currently native-only.

use anyhow::{Result, bail};

pub fn unsupported() -> Result<()> {
    bail!("workspace is not yet implemented on wasm")
}
