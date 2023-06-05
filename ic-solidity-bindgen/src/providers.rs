use std::collections::HashMap;

use async_trait::async_trait;
use ic_web3_rs::contract::tokens::{Detokenize, Tokenize};
use ic_web3_rs::contract::{Contract, Options};
use ic_web3_rs::ethabi::Log;
use ic_web3_rs::transports::ic_http_client::CallOptions;
use ic_web3_rs::transports::ICHttp;
use ic_web3_rs::types::Log as EthLog;
use ic_web3_rs::types::H256;
use ic_web3_rs::Error;

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
        contract: Contract<ICHttp>,
        event_name: &str,
        from: u64,
        to: u64,
        call_options: CallOptions,
    ) -> Result<HashMap<u64, Vec<EventLog>>, ic_web3_rs::Error>;
}

pub struct EventLog {
    pub event: Log,
    pub log: EthLog,
}
