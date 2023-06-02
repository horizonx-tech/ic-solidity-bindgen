use crate::context::Web3Context;
use crate::providers::{CallProvider, SendProvider};
use crate::transform::TransformProcessorBuilder;
use async_trait::async_trait;
use ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};
use ic_web3::contract::tokens::{Detokenize, Tokenize};
use ic_web3::contract::Contract;
use ic_web3::contract::Options;
use ic_web3::ic::{get_public_key, pubkey_to_address, KeyInfo};
use ic_web3::transports::ic_http_client::CallOptions;
use ic_web3::transports::ICHttp;
use ic_web3::types::{Address, TransactionReceipt, U64};
use std::future::Future;
use std::marker::Unpin;

const RPC_CALL_MAX_RETRY: u8 = 3;
/// Mostly exists to map to the new futures.
/// This is the "untyped" API which the generated types will use.
pub struct Web3Provider {
    contract: Contract<ICHttp>,
    context: Web3Context,
    rpc_call_max_retry: u8,
}

impl Web3Provider {
    pub fn contract(&self) -> ic_web3::ethabi::Contract {
        self.contract.abi().clone()
    }
    async fn with_retry<T, E, Fut, F: FnMut() -> Fut>(&self, mut f: F) -> Result<T, E>
    where
        Fut: Future<Output = Result<T, E>>,
    {
        let mut count = 0;
        loop {
            let result = f().await;

            if result.is_ok() {
                break result;
            } else {
                if count > self.rpc_call_max_retry {
                    break result;
                }
                count += 1;
            }
        }
    }
}

#[async_trait]
impl CallProvider for Web3Provider {
    async fn call<O: Detokenize + Unpin + Send, Params: Tokenize + Send>(
        &self,
        name: &'static str,
        params: Params,
    ) -> Result<O, ic_web3::Error> {
        match self
            .contract
            .query(
                name,
                params,
                Some(self.context.from()),
                Default::default(),
                None,
            )
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => match e {
                ic_web3::contract::Error::Api(e) => Err(e),
                // The other variants InvalidOutputType and Abi should be
                // prevented by the code gen. It is useful to convert the error
                // type to be restricted to the web3::Error type for a few
                // reasons. First, the web3::Error type (unlike the
                // web3::contract::Error type) implements Send. This makes it
                // usable in async methods. Also for consistency it's easier to
                // mix methods using both call and send to use the ? operator if
                // they have the same error type. It is the opinion of this
                // library that ABI sorts of errors are irrecoverable and should
                // panic anyway.
                e => panic!("The ABI is out of date. Name: {}. Inner: {}", name, e),
            },
        }
    }
}

pub fn default_derivation_key() -> Vec<u8> {
    ic_cdk::id().as_slice().to_vec()
}

async fn public_key(key_name: String) -> Result<Vec<u8>, String> {
    get_public_key(
        None,
        vec![default_derivation_key()],
        // tmp: this should be a random string
        key_name,
    )
    .await
}

fn to_ethereum_address(pub_key: Vec<u8>) -> Result<Address, String> {
    pubkey_to_address(&pub_key)
}

pub async fn ethereum_address(key_name: String) -> Result<Address, String> {
    let pub_key = public_key(key_name).await?;
    to_ethereum_address(pub_key)
}

#[async_trait]
impl SendProvider for Web3Provider {
    type Out = TransactionReceipt;
    async fn send<Params: Tokenize + Send>(
        &self,
        func: &'static str,
        params: Params,
        options: Option<Options>,
        confirmations: Option<usize>,
    ) -> Result<Self::Out, ic_web3::Error> {
        let canister_addr = ethereum_address(self.context.key_name().to_string()).await?;
        let gas_price = self
            .with_retry(|| self.context.eth().gas_price(CallOptions::default()))
            .await?;
        let nonce = self
            .with_retry(|| {
                self.context
                    .eth()
                    .transaction_count(canister_addr, None, CallOptions::default())
            })
            .await?;
        self.contract
            .signed_call_with_confirmations(
                func,
                params,
                match options {
                    None => Options::with(|op| {
                        op.gas_price = Some(gas_price);
                        op.transaction_type = Some(U64::from(2)); // EIP1559_TX_ID
                        op.nonce = Some(nonce);
                    }),
                    Some(options) => options,
                },
                hex::encode(canister_addr),
                match confirmations {
                    // TODO
                    // to do confirmations, we need to fix eth_newBlockFilter
                    // [Canister xxx] unlock result: Err(Rpc(Error { code: MethodNotFound, message: "the method eth_newBlockFilter does not exist/is not available", data: None }))
                    None => 0,
                    Some(confirmations) => confirmations,
                },
                KeyInfo {
                    derivation_path: vec![default_derivation_key()],
                    key_name: self.context.key_name().to_string(),
                    ecdsa_sign_cycles: None, // use default (is there a problem with prod_key?)
                },
                self.context.chain_id(),
            )
            .await
    }
}

impl Web3Provider {
    pub fn new(contract_address: Address, context: &Web3Context, json_abi: &[u8]) -> Self {
        let context = context.clone();

        // All of the ABIs are verified at compile time, so we can just unwrap here.
        // See also 4cd1038f-56f2-4cf2-8dbe-672da9006083
        let contract = Contract::from_json(context.eth(), contract_address, json_abi).unwrap();

        Self {
            contract,
            context,
            rpc_call_max_retry: RPC_CALL_MAX_RETRY,
        }
    }
    pub fn set_max_retry(&mut self, max_retry: u8) {
        self.rpc_call_max_retry = max_retry;
    }
}
