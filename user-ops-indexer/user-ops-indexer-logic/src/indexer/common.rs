use entity::sea_orm_active_enums::SponsorType;
use ethers::prelude::{Address, Bytes, Log, U256};

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
    let (mut l, mut r) = (0usize, logs.len());
    while l < r && (logs[r - 1].address == entry_point || Some(logs[r - 1].address) == paymaster) {
        r -= 1
    }
    while l < r && logs[l].address == entry_point {
        l += 1
    }
    (
        logs.get(l)
            .and_then(|l| l.log_index)
            .map_or(0, |v| v.as_u32()),
        (r - l) as u32,
    )
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

#[cfg(test)]
mod tests {
    use crate::indexer::common::extract_user_logs_boundaries;
    use ethers::prelude::{types::Log, Address, U256};

    #[test]
    fn test_extract_user_logs_boundaries() {
        let entry_point = Address::from_low_u64_be(1);
        let paymaster = Address::from_low_u64_be(2);
        let other = Address::from_low_u64_be(3);
        let logs = vec![
            entry_point,
            entry_point,
            other,
            entry_point,
            paymaster,
            entry_point,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, a)| Log {
            address: a,
            topics: vec![],
            data: Default::default(),
            block_hash: None,
            block_number: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: Some(U256::from(i + 10)),
            transaction_log_index: None,
            log_type: None,
            removed: None,
        })
        .collect::<Vec<_>>();

        assert_eq!(
            extract_user_logs_boundaries(&logs, entry_point, Some(paymaster)),
            (12, 1)
        );
        assert_eq!(
            extract_user_logs_boundaries(&logs[..0], entry_point, Some(paymaster)),
            (0, 0)
        );
        assert_eq!(
            extract_user_logs_boundaries(&logs[..2], entry_point, Some(paymaster)),
            (10, 0)
        );
        assert_eq!(
            extract_user_logs_boundaries(&logs[2..], entry_point, Some(paymaster)),
            (12, 1)
        );
        assert_eq!(
            extract_user_logs_boundaries(&logs[2..3], entry_point, Some(paymaster)),
            (12, 1)
        );
        assert_eq!(
            extract_user_logs_boundaries(&logs[..3], entry_point, Some(paymaster)),
            (12, 1)
        );
    }
}
