use std::collections::HashMap;

use async_trait::async_trait;
use ic_web3_rs::contract::tokens::{Detokenize, Tokenize};
use ic_web3_rs::contract::Options;
use ic_web3_rs::transports::ic_http_client::CallOptions;
use ic_web3_rs::Error;

use crate::types::EventLog;

#[async_trait]
pub trait CallProvider {
    async fn call<Out: Detokenize + Unpin + Send, Params: Tokenize + Send>(
        &self,
        name: &'static str,
        params: Params,
    ) -> Result<Out, Error>;
}

#[async_trait]
pub trait SendProvider {
    type Out;
    async fn send<Params: Tokenize + Send>(
        &self,
        func: &'static str,
        params: Params,
        options: Option<Options>,
        confirmations: Option<usize>,
    ) -> Result<Self::Out, ic_web3_rs::Error>;
}

#[async_trait]
pub trait LogProvider {
    async fn find(
        &self,
        event_name: &str,
        from: u64,
        to: u64,
        call_options: CallOptions,
    ) -> Result<HashMap<u64, Vec<EventLog>>, ic_web3_rs::Error>;
}
