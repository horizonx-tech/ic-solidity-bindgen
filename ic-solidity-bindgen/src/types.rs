use ic_web3_rs::ethabi::Log;
use ic_web3_rs::types::Log as EthLog;

pub struct EventLog {
    pub event: Log,
    pub log: EthLog,
}
