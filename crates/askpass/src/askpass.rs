#[cfg(not(target_family = "wasm"))]
include!("askpass_native_impl.rs");

#[cfg(target_family = "wasm")]
mod encrypted_password;
#[cfg(target_family = "wasm")]
pub use encrypted_password::{EncryptedPassword, IKnowWhatIAmDoingAndIHaveReadTheDocs};

#[cfg(target_family = "wasm")]
mod wasm;
#[cfg(target_family = "wasm")]
pub use wasm::*;
