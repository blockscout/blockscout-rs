use entity::sea_orm_active_enums::SponsorType;
use ethers::prelude::{
    abi::{Error, RawLog},
    Address, Bytes, EthEvent, Log, U256,
};

pub fn parse_event<T: EthEvent>(log: &Log) -> Result<T, Error> {
    T::decode_log(&RawLog::from(log.clone()))
}

pub fn extract_address(b: &Bytes) -> Option<Address> {
    if b.len() >= 20 {
        Some(Address::from_slice(&b[..20]))
    } else {
        None
    }
}

pub fn extract_sponsor_type(
    sender: Address,
    paymaster: Option<Address>,
    tx_deposits: &[Address],
) -> SponsorType {
    let sender_deposit = tx_deposits.iter().any(|&e| e == sender);
    let paymaster_deposit = tx_deposits.iter().any(|&e| Some(e) == paymaster);
    match (paymaster, sender_deposit, paymaster_deposit) {
        (None, false, _) => SponsorType::WalletBalance,
        (None, true, _) => SponsorType::WalletDeposit,
        (Some(_), _, false) => SponsorType::PaymasterSponsor,
        (Some(_), _, true) => SponsorType::PaymasterHybrid,
    }
}

pub fn extract_user_logs_boundaries(
    logs: &[Log],
    entry_point: Address,
    paymaster: Option<Address>,
) -> (u32, u32) {
    let mut user_logs_count = logs.len();
    while user_logs_count > 0
        && (logs[user_logs_count - 1].address == entry_point
            || Some(logs[user_logs_count - 1].address) == paymaster)
    {
        user_logs_count -= 1;
    }

    let user_logs_start_index = logs
        .first()
        .map_or(0, |l| l.log_index.map_or(0, |v| v.as_u32()));
    (user_logs_start_index, user_logs_count as u32)
}

pub fn unpack_uints(data: &[u8]) -> (U256, U256) {
    (
        U256::from_big_endian(&data[..16]),
        U256::from_big_endian(&data[16..]),
    )
}

pub fn none_if_empty(b: Bytes) -> Option<Bytes> {
    if b.is_empty() {
        None
    } else {
        Some(b)
    }
}
