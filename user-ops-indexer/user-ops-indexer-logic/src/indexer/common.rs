use entity::sea_orm_active_enums::SponsorType;
use ethers::prelude::{
    abi::{decode, parse_abi, ParamType, Token},
    Address, Bytes, Log, U256,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref EXECUTE_SELECTORS: Vec<[u8; 4]> = parse_abi(&[
        "function execute(address,uint256,bytes,uint8) external",
        "function execute(address,uint256,bytes) external",
        "function execute_ncC(address,uint256,bytes) external",
        "function execTransactionFromEntrypoint(address,uint256,bytes) external",
        "function executeAndRevert(address,uint256,bytes,uint8) external",
        "function execFromEntryPoint(address,uint256,bytes) external",
        "function execTransactionFromEntrypoint(address,uint256,bytes,uint8,address,address,uint256)",
        "function executeCall(address,uint256,bytes)",
        "function executeUserOp(address,uint256,bytes,uint8)",
        "struct ExecStruct { address arg1; address arg2; uint256 arg3;}",
        "function execFromEntryPointWithFee(address,uint256,bytes,ExecStruct)",
        "function execTransactionFromEntrypoint(address,uint256,bytes,uint8)",
        "function send(address,uint256,bytes)",
        "function execute(address,uint256,bytes,bytes)",
        "function callContract(address,uint256,bytes,bool)",
        "function exec(address,uint256,bytes)",
    ])
    .unwrap()
    .functions()
    .map(|f| f.short_signature())
    .collect();
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

pub fn decode_execute_call_data(call_data: &Bytes) -> (Option<Address>, Option<Bytes>) {
    if EXECUTE_SELECTORS
        .iter()
        .any(|sig| call_data.starts_with(sig))
    {
        match decode(
            &[ParamType::Address, ParamType::Uint(256), ParamType::Bytes],
            &call_data[4..],
        )
        .as_deref()
        {
            Ok([Token::Address(execute_target), _, Token::Bytes(execute_call_data)]) => (
                Some(*execute_target),
                Some(Bytes::from(execute_call_data.clone())),
            ),
            Ok(_) => {
                tracing::warn!(
                    call_data = call_data.to_string(),
                    "failed to match call_data parsing result"
                );
                (None, None)
            }
            Err(err) => {
                tracing::warn!(error = ?err, call_data = call_data.to_string(), "failed to parse call_data");
                (None, None)
            }
        }
    } else {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use crate::indexer::common::{decode_execute_call_data, extract_user_logs_boundaries};
    use ethers::prelude::{types::Log, Address, U256};
    use ethers_core::types::Bytes;
    use std::str::FromStr;

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

    #[test]
    fn test_decode_execute_call_data() {
        let call_data = Bytes::from_str("0x5194544700000000000000000000000014778860e937f509e651192a90589de711fb88a90000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000001d993968fbd7669690384eab1b4d23aeb1132bf40000000000000000000000000000000000000000000000004563918244f4000000000000000000000000000000000000000000000000000000000000").unwrap();
        let (execute_target, execute_call_data) = decode_execute_call_data(&call_data);
        assert_eq!(
            execute_target,
            Some(Address::from_str("0x14778860E937f509e651192a90589dE711Fb88a9").unwrap())
        );
        assert_eq!(execute_call_data, Some(Bytes::from_str("0xa9059cbb0000000000000000000000001d993968fbd7669690384eab1b4d23aeb1132bf40000000000000000000000000000000000000000000000004563918244f40000").unwrap()));
        let (execute_target, execute_call_data) =
            decode_execute_call_data(&execute_call_data.unwrap());
        assert_eq!(execute_target, None);
        assert_eq!(execute_call_data, None);
    }
}
