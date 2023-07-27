use ic_web3_rs::ethabi::Log;
use ic_web3_rs::types::Log as EthLog;

#[derive(Debug, PartialEq, Clone)]
pub struct EventLog {
    pub event: Log,
    pub log: EthLog,
}
