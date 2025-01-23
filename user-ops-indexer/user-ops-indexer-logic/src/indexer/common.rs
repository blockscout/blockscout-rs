use alloy::{
    json_abi::JsonAbi,
    primitives::{Address, Bytes, Selector, U256},
    rpc::types::Log,
    sol_types,
    sol_types::SolValue,
};
use entity::sea_orm_active_enums::SponsorType;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref EXECUTE_SELECTORS: Vec<Selector> = JsonAbi::parse([
        "function execute(address,uint256,bytes,uint8) external",
        "function execute(address,uint256,bytes) external",
        "function execute_ncC(address,uint256,bytes) external",
        "function execTransactionFromEntrypoint(address,uint256,bytes) external",
        "function executeAndRevert(address,uint256,bytes,uint8) external",
        "function execFromEntryPoint(address,uint256,bytes) external",
        "function execTransactionFromEntrypoint(address,uint256,bytes,uint8,address,address,uint256)",
        "function executeCall(address,uint256,bytes)",
        "function executeUserOp(address,uint256,bytes,uint8)",
        "function execFromEntryPointWithFee(address,uint256,bytes,tuple(address,address,uint256))",
        "function execTransactionFromEntrypoint(address,uint256,bytes,uint8)",
        "function send(address,uint256,bytes)",
        "function execute(address,uint256,bytes,bytes)",
        "function callContract(address,uint256,bytes,bool)",
        "function exec(address,uint256,bytes)",
    ])
    .unwrap()
    .functions()
    .map(|f| f.selector())
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
    while l < r
        && (logs[r - 1].address() == entry_point || Some(logs[r - 1].address()) == paymaster)
    {
        r -= 1
    }
    while l < r && logs[l].address() == entry_point {
        l += 1
    }
    (
        logs.get(l).and_then(|l| l.log_index).unwrap_or(0) as u32,
        (r - l) as u32,
    )
}

pub fn unpack_uints(data: &[u8]) -> (U256, U256) {
    (
        U256::from_be_slice(&data[..16]),
        U256::from_be_slice(&data[16..]),
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
        .any(|sig| call_data.starts_with(sig.as_slice()))
    {
        let res: sol_types::Result<(Address, U256, Bytes)> =
            SolValue::abi_decode_params(&call_data[4..], false);
        match res {
            Ok((execute_target, _, execute_call_data)) => {
                (Some(execute_target), Some(execute_call_data))
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
    use alloy::{
        primitives::{address, bytes, LogData},
        rpc::types::Log,
    };

    #[test]
    fn test_extract_user_logs_boundaries() {
        let entry_point = address!("0000000000000000000000000000000000000001");
        let paymaster = address!("0000000000000000000000000000000000000002");
        let other = address!("0000000000000000000000000000000000000003");
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
            inner: alloy::primitives::Log {
                address: a,
                data: LogData::empty(),
            },
            block_hash: None,
            block_number: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: Some((i + 10) as u64),
            removed: false,
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
        let call_data = bytes!("5194544700000000000000000000000014778860e937f509e651192a90589de711fb88a90000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000044a9059cbb0000000000000000000000001d993968fbd7669690384eab1b4d23aeb1132bf40000000000000000000000000000000000000000000000004563918244f4000000000000000000000000000000000000000000000000000000000000");
        let (execute_target, execute_call_data) = decode_execute_call_data(&call_data);
        assert_eq!(
            execute_target,
            Some(address!("14778860E937f509e651192a90589dE711Fb88a9"))
        );
        assert_eq!(execute_call_data, Some(bytes!("a9059cbb0000000000000000000000001d993968fbd7669690384eab1b4d23aeb1132bf40000000000000000000000000000000000000000000000004563918244f40000")));
        let (execute_target, execute_call_data) =
            decode_execute_call_data(&execute_call_data.unwrap());
        assert_eq!(execute_target, None);
        assert_eq!(execute_call_data, None);
    }
}
