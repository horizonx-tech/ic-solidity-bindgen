use crate::Web3Provider;
use ic_web3::api::Eth;
use ic_web3::transports::ICHttp;
use ic_web3::types::Address;
use ic_web3::Web3;
use std::sync::Arc;

/// Common data associated with multiple contracts.
#[derive(Clone)]
pub struct Web3Context(Arc<Web3ContextInner>);

pub trait Context {
    type Provider;
    fn provider(&self, contract: Address, abi: &[u8]) -> Self::Provider;
}

struct Web3ContextInner {
    from: Address,
    // We are not expecting to interact with the chain frequently,
    // and the websocket transport has problems with ping.
    // So, the Http transport seems like the best choice.
    eth: Eth<ICHttp>,
    chain_id: u64,
    key_name: String,
}

impl Web3Context {
    pub fn new(
        url: &str,
        from: Address,
        chain_id: u64,
        key_name: String,
    ) -> Result<Self, ic_web3::error::Error> {
        let transport = ICHttp::new(url, None)?;
        let web3 = Web3::new(transport);
        let eth = web3.eth();
        let inner = Web3ContextInner {
            eth,
            from,
            chain_id,
            key_name,
        };
        Ok(Self(Arc::new(inner)))
    }

    pub fn from(&self) -> Address {
        self.0.from
    }

    pub(crate) fn eth(&self) -> Eth<ICHttp> {
        self.0.eth.clone()
    }
    pub fn chain_id(&self) -> u64 {
        self.0.chain_id
    }

    pub fn key_name(&self) -> &str {
        &self.0.key_name
    }
}

impl Context for Web3Context {
    type Provider = Web3Provider;
    fn provider(&self, contract: Address, json_abi: &[u8]) -> Self::Provider {
        Web3Provider::new(contract, self, json_abi)
    }
}
