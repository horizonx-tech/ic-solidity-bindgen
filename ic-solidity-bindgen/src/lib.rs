/// A module for implementation that needs to be exposed to macros
#[doc(hidden)]
pub mod internal;

mod context;
mod providers;
pub mod types;
mod web3_provider;
pub mod rpc_methods;

pub use providers::{CallProvider, LogProvider, SendProvider};
pub use web3_provider::Web3Provider;

// Re-export the macros
pub use ic_solidity_bindgen_macros::*;

pub use context::{Context, Web3Context};
