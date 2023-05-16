use crate::context::Web3Context;
use crate::providers::{CallProvider, SendProvider};
use async_trait::async_trait;
use ic_web3::contract::tokens::{Detokenize, Tokenize};
use ic_web3::contract::Contract;
use ic_web3::contract::Options;
use ic_web3::ic::{get_public_key, pubkey_to_address, KeyInfo};
use ic_web3::transports::ICHttp;
use ic_web3::types::{Address, TransactionReceipt};
use std::marker::Unpin;

/// Mostly exists to map to the new futures.
/// This is the "untyped" API which the generated types will use.
pub struct Web3Provider {
    contract: Contract<ICHttp>,
    context: Web3Context,
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

        self.contract
            .signed_call_with_confirmations(
                func,
                params,
                match options {
                    None => Default::default(),
                    Some(options) => options,
                },
                hex::encode(canister_addr),
                match confirmations {
                    // Num confirmations. From a library standpoint, this should be
                    // a parameter of the function. Choosing a correct value is very
                    // difficult, even for a consumer of the library as it would
                    // require assessing the value of the transaction, security
                    // margins, and a number of other factors for which data may not
                    // be available. So just picking a pretty high security margin
                    // for now.
                    None => 24,
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

        Self { contract, context }
    }
}
