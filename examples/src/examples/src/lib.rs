use std::str::FromStr;

use candid::{candid_method, CandidType, Deserialize};
use ic_cdk::{
    api::management_canister::http_request::{HttpResponse, TransformArgs},
    query,
};
use ic_solidity_bindgen::{contract_abis, Web3Context, Web3Provider};
use ic_web3_rs::{ethabi::Address, transports::ic_http_client::CallOptions, types::U256};
contract_abis!("abis");

const DAI_ADDRESS: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F";

const CURVE_POOL_ADDRESS: &str = "0xbEbc44782C7dB0a1A60Cb6fe97d0b483032FF1C7";

const MAINNET_RPC_URL: &str = "https://mainnet.infura.io/v3/YOUR_INFURA_KEY";

#[derive(CandidType, Debug, Clone, PartialEq, PartialOrd, Deserialize)]
pub struct WrappedU256 {
    value: String,
}

impl WrappedU256 {
    pub fn from(val: U256) -> Self {
        Self {
            value: val.to_string(),
        }
    }
    pub fn value(&self) -> U256 {
        U256::from_dec_str(self.value.as_str()).unwrap()
    }
}

fn dai_addr() -> Address {
    addr_of(DAI_ADDRESS)
}

fn curve_pool_addr() -> Address {
    addr_of(CURVE_POOL_ADDRESS)
}

fn addr_of(addr: &str) -> Address {
    Address::from_str(addr).unwrap()
}

#[ic_cdk::update]
async fn balance() -> WrappedU256 {
    let val = erc20_contract()
        .balance_of(curve_pool_addr(), None)
        .await
        .unwrap();
    WrappedU256::from(val)
}

#[ic_cdk::update]
async fn find_total_transfer_amount_between(from: u64, to: u64) -> WrappedU256 {
    let val = erc20_contract()
        .event_transfer(from, to, CallOptions::default())
        .await
        .unwrap()
        .iter()
        .map(|(_, v)| {
            v.iter().map(|log| {
                log.event
                    .params
                    .iter()
                    .find(|p| p.name == "value")
                    .unwrap()
                    .clone()
                    .value
                    .into_uint()
                    .unwrap()
            })
        })
        .flatten()
        .fold(U256::default(), |acc, v| acc + v);
    WrappedU256::from(val)
}

fn erc20_contract() -> ERC20<Web3Provider> {
    ERC20::new(
        dai_addr(),
        &Web3Context::new(
            MAINNET_RPC_URL,
            Address::from_low_u64_be(0),
            1,
            "test_key_1".to_string(),
        )
        .unwrap(),
    )
}
#[query(name = "transform")]
#[candid_method(query, rename = "transform")]
fn transform(response: TransformArgs) -> HttpResponse {
    let res = response.response;
    // remove header
    HttpResponse {
        status: res.status,
        headers: Vec::default(),
        body: res.body,
    }
}
