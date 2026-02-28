#[cfg(not(target_family = "wasm"))]
mod native;
#[cfg(target_family = "wasm")]
mod wasm;

#[cfg(not(target_family = "wasm"))]
pub use native::*;
#[cfg(target_family = "wasm")]
pub use wasm::*;

#[cfg(not(target_family = "wasm"))]
pub use native::{blame, commit, hosting_provider, remote, repository, stash, status};
#[cfg(target_family = "wasm")]
pub use wasm::{blame, commit, hosting_provider, remote, repository, stash, status};
