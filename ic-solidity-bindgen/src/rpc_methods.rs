use ic_web3_rs::{
    helpers,
    types::{Address, BlockNumber, U256},
};
use serde_json::Value;

pub enum EVMRpcMethod {
    TransactionCount(Address, BlockNumber),
    GasPrice,
    /// BlockCount, BlockTag, RewardPercentile
    FeeHistory(U256, BlockNumber, Option<Vec<f64>>),
    MaxPriorityFeePerGas,
}

impl EVMRpcMethod {
    pub fn method(&self) -> &str {
        match self {
            Self::TransactionCount(_, _) => "eth_getTransactionCount",
            Self::GasPrice => "eth_gasPrice",
            Self::MaxPriorityFeePerGas => "eth_maxPriorityFeePerGas",
            Self::FeeHistory(_, _, _) => "eth_feeHistory",
        }
    }
    pub fn params(&self) -> Vec<Value> {
        match self {
            EVMRpcMethod::FeeHistory(block_count, newest_block, reward_percentiles) => {
                vec![
                    helpers::serialize(&block_count),
                    helpers::serialize(&newest_block),
                    helpers::serialize(&reward_percentiles),
                ]
            }
            EVMRpcMethod::TransactionCount(address, block_number) => vec![
                helpers::serialize(&address),
                helpers::serialize(&block_number),
            ],
            _ => vec![],
        }
    }
}
