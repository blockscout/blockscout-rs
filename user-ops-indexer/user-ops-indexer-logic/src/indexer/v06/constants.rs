use ethabi::{Error, RawLog};
use ethers::prelude::{abigen, EthEvent};
use ethers_core::types::Log;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref ENTRYPOINT_V06: ethers::types::Address =
        "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
            .parse()
            .unwrap();
}

abigen!(IEntrypointV06, "./src/indexer/v06/abi.json");

pub fn matches_entrypoint_event<T: EthEvent>(log: &Log) -> bool {
    log.address == *ENTRYPOINT_V06 && log.topics.get(0) == Some(&T::signature())
}

pub fn parse_event<T: EthEvent>(log: &Log) -> Result<T, Error> {
    T::decode_log(&RawLog::from(log.clone()))
}
