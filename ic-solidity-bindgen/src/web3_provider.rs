use crate::context::Web3Context;
use crate::providers::{CallProvider, LogProvider, SendProvider};
use crate::types::EventLog;
use async_trait::async_trait;
use ic_web3_rs::contract::tokens::{Detokenize, Tokenize};
use ic_web3_rs::contract::Contract;
use ic_web3_rs::contract::Options;
use ic_web3_rs::ethabi::{RawLog, Topic, TopicFilter};
use ic_web3_rs::ic::{get_public_key, pubkey_to_address, KeyInfo};
use ic_web3_rs::transports::ic_http_client::CallOptions;
use ic_web3_rs::transports::ICHttp;
use ic_web3_rs::types::{Address, BlockId, BlockNumber, FilterBuilder, TransactionReceipt, H256, U256, U64};
use ic_web3_rs::Transport;
use std::collections::HashMap;
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
    pub fn contract(&self) -> ic_web3_rs::ethabi::Contract {
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
    ) -> Result<O, ic_web3_rs::Error> {
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
                ic_web3_rs::contract::Error::Api(e) => Err(e),
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

fn event_sig<T: Transport>(contract: &Contract<T>, name: &str) -> Result<H256, String> {
    contract
        .abi()
        .event(name)
        .map(|e| e.signature())
        .map_err(|e| (format!("event {} not found in contract abi: {}", name, e)))
}

#[async_trait]
impl LogProvider for Web3Provider {
    async fn find(
        &self,
        event_name: &str,
        from: u64,
        to: u64,
        call_options: CallOptions,
    ) -> Result<HashMap<u64, Vec<EventLog>>, ic_web3_rs::Error> {
        let parser = self
            .contract
            .abi()
            .event(event_name)
            .map_err(|_| ic_web3_rs::Error::Internal)?;
        let logs = self
            .context
            .eth()
            .logs(
                FilterBuilder::default()
                    .from_block(BlockNumber::Number(from.into()))
                    .to_block(BlockNumber::Number(to.into()))
                    .address(vec![self.contract.address()])
                    .topic_filter(TopicFilter {
                        topic0: Topic::This(event_sig(&self.contract, event_name).unwrap()),
                        topic1: Topic::Any,
                        topic2: Topic::Any,
                        topic3: Topic::Any,
                    })
                    .build(),
                call_options,
            )
            .await?
            .into_iter()
            .filter(|log| !log.removed.unwrap_or_default())
            .filter(|log| log.transaction_index.is_some())
            .filter(|log| log.block_hash.is_some())
            .map(|log| EventLog {
                event: parser
                    .parse_log(RawLog {
                        data: log.data.0.clone(),
                        topics: log.topics.clone(),
                    })
                    .unwrap(),
                log,
            })
            .fold(HashMap::new(), |mut acc, event| {
                let block = event.log.block_number.unwrap().as_u64();
                let events = acc.entry(block).or_insert_with(Vec::new);
                events.push(event);
                acc
            });
        Ok(logs)
    }
}

impl Web3Provider {
    async fn build_eip_1559_tx_params(&self) -> Result<Options, ic_web3_rs::Error> {
        let eth = self.context.eth();
        let current_block = self.with_retry(||{
            eth.block(BlockId::Number(BlockNumber::Latest), CallOptions::default())
        }).await?.expect("block not found");
        let max_priority_fee_per_gas = self
            .with_retry(|| eth.max_priority_fee_per_gas(CallOptions::default()))
            .await?;
        let nonce = self
            .with_retry(|| {
                eth
                    .transaction_count(self.context.from(), None, CallOptions::default())
            })
            .await?;

        Ok(Options {
            max_fee_per_gas: Some(calc_max_fee_per_gas(max_priority_fee_per_gas, current_block.base_fee_per_gas.unwrap_or_default())),
            max_priority_fee_per_gas: Some(max_priority_fee_per_gas),
            nonce: Some(nonce),
            transaction_type: Some(U64::from(2)), // EIP1559_TX_ID for default
            ..Default::default()
        })
    }
}

fn calc_max_fee_per_gas(max_priority_fee_per_gas: U256, base_fee_per_gas: U256) -> U256 {
    max_priority_fee_per_gas + (base_fee_per_gas * U256::from(2))
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
    ) -> Result<Self::Out, ic_web3_rs::Error> {
        let canister_addr = ethereum_address(self.context.key_name().to_string()).await?;
        let call_option = match options {
            Some(options) => options,
            None => self.build_eip_1559_tx_params().await?,
        };

        self.contract
            .signed_call_with_confirmations(
                func,
                params,
                call_option,
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
