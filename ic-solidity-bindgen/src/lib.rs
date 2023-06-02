/// A module for implementation that needs to be exposed to macros
#[doc(hidden)]
pub mod internal;

mod context;
mod providers;
mod transform;
mod web3_provider;

pub use providers::{CallProvider, SendProvider};
pub use transform::*;
pub use web3_provider::Web3Provider;

// Re-export the macros
pub use ic_solidity_bindgen_macros::*;

pub use context::{Context, Web3Context};
