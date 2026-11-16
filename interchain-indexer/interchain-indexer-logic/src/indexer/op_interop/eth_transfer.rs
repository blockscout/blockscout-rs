// SPDX-License-Identifier: LicenseRef-Blockscout

//! Native ETH transfers sent through `SuperchainETHBridge`.
//!
//! The transfer is derived from the `SentMessage` payload alone: a message
//! whose sender and target are both the bridge predeploy and whose payload is
//! exactly `relayETH(from, to, amount)`. `SendETH` / `RelayETH` are
//! diagnostics; they never create, suppress or change a transfer. See ADR-019.

use std::str::FromStr;

use alloy::primitives::{Address, ChainId, U256};
use interchain_indexer_entity::{
    crosschain_transfers, new_transfer, sea_orm_active_enums::TransferAssetLinkage,
};
use sea_orm::{ActiveValue, prelude::BigDecimal};

use crate::{indexer::xdai::NATIVE_SENTINEL, message_buffer::Key};

use super::{
    types::RelayEth,
    version::{RELAY_ETH_SELECTOR, SUPERCHAIN_ETH_BRIDGE},
};

/// `selector ‖ from ‖ to ‖ amount`.
const RELAY_ETH_PAYLOAD_LEN: usize = 4 + 3 * 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EthTransfer {
    pub(crate) from: Address,
    pub(crate) to: Address,
    pub(crate) amount: U256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EthPayload {
    /// Not a bridge-to-bridge message. Ordinary traffic.
    NotBridgeMessage,
    Transfer(EthTransfer),
    /// Bridge-to-bridge, but not the payload grammar version 1 knows.
    Unrecognized,
}

/// The single decoder of an ETH transfer payload.
pub(crate) fn classify(sender: Address, target: Address, message: &[u8]) -> EthPayload {
    if sender != SUPERCHAIN_ETH_BRIDGE || target != SUPERCHAIN_ETH_BRIDGE {
        return EthPayload::NotBridgeMessage;
    }
    if message.len() != RELAY_ETH_PAYLOAD_LEN || message[..4] != RELAY_ETH_SELECTOR {
        return EthPayload::Unrecognized;
    }
    let from_word = &message[4..36];
    let to_word = &message[36..68];
    let amount_word = &message[68..100];
    if from_word[..12].iter().any(|byte| *byte != 0) || to_word[..12].iter().any(|byte| *byte != 0)
    {
        return EthPayload::Unrecognized;
    }
    EthPayload::Transfer(EthTransfer {
        from: Address::from_slice(&from_word[12..]),
        to: Address::from_slice(&to_word[12..]),
        amount: U256::from_be_slice(amount_word),
    })
}

/// The one transfer row of an ETH message: index 0, the native sentinel on
/// both chains, equal amounts, `Mirror` linkage. `None` only if the amount
/// cannot be turned into a decimal, which does not happen for a `U256`.
pub(crate) fn transfer_model(
    key: &Key,
    source_chain_id: i64,
    destination_chain_id: i64,
    transfer: &EthTransfer,
) -> Option<crosschain_transfers::ActiveModel> {
    let amount = match BigDecimal::from_str(&transfer.amount.to_string()) {
        Ok(amount) => amount,
        Err(err) => {
            tracing::error!(
                err = ?err,
                bridge_id = key.bridge_id,
                message_id = key.message_id,
                "failed to convert an OP interop ETH transfer amount; writing no transfer"
            );
            return None;
        }
    };
    Some(crosschain_transfers::ActiveModel {
        message_id: ActiveValue::Set(key.message_id),
        bridge_id: ActiveValue::Set(key.bridge_id as i32),
        index: ActiveValue::Set(0),
        token_src_chain_id: ActiveValue::Set(source_chain_id),
        token_dst_chain_id: ActiveValue::Set(destination_chain_id),
        token_src_address: ActiveValue::Set(Some(NATIVE_SENTINEL.to_vec())),
        token_dst_address: ActiveValue::Set(Some(NATIVE_SENTINEL.to_vec())),
        src_amount: ActiveValue::Set(Some(amount.clone())),
        dst_amount: ActiveValue::Set(Some(amount)),
        sender_address: ActiveValue::Set(Some(transfer.from.to_vec())),
        recipient_address: ActiveValue::Set(Some(transfer.to.to_vec())),
        token_ids: ActiveValue::Set(None),
        ..new_transfer(TransferAssetLinkage::Mirror)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelayEthCheck {
    Ok,
    Missing,
    Mismatch,
}

/// Diagnostic cross-check of the destination `RelayETH` against the payload.
/// Never an outcome.
pub(crate) fn check_relay_eth(
    payload: &EthPayload,
    source_chain_id: ChainId,
    relay_eth: Option<&RelayEth>,
) -> RelayEthCheck {
    match (payload, relay_eth) {
        (EthPayload::Transfer(_), None) => RelayEthCheck::Missing,
        (EthPayload::Transfer(transfer), Some(relay)) => {
            if relay.from == transfer.from
                && relay.to == transfer.to
                && relay.amount == transfer.amount
                && relay.source == U256::from(source_chain_id)
            {
                RelayEthCheck::Ok
            } else {
                RelayEthCheck::Mismatch
            }
        }
        (_, Some(_)) => RelayEthCheck::Mismatch,
        (_, None) => RelayEthCheck::Ok,
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::address;
    use interchain_indexer_entity::sea_orm_active_enums::TokenType;
    use sea_orm::ActiveValue::Set;

    use super::{
        super::{
            abi::v1,
            test_fixtures::{DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, sent_message_logs},
            types::key_from_message_hash,
        },
        *,
    };

    const FROM: Address = address!("1111111111111111111111111111111111111111");
    const TO: Address = address!("2222222222222222222222222222222222222222");
    const ONE_ETH: u128 = 1_000_000_000_000_000_000;

    fn fixture_payload(index: usize) -> Vec<u8> {
        let logs = sent_message_logs();
        logs[index]
            .log_decode::<v1::SentMessage>()
            .expect("fixture decodes")
            .inner
            .data
            .message
            .to_vec()
    }

    fn eth_payload() -> Vec<u8> {
        fixture_payload(0)
    }

    fn transfer() -> EthTransfer {
        EthTransfer {
            from: FROM,
            to: TO,
            amount: U256::from(ONE_ETH),
        }
    }

    fn relay_eth() -> RelayEth {
        RelayEth {
            from: FROM,
            to: TO,
            amount: U256::from(ONE_ETH),
            source: U256::from(SOURCE_CHAIN_ID),
        }
    }

    fn bridge_classify(message: &[u8]) -> EthPayload {
        classify(SUPERCHAIN_ETH_BRIDGE, SUPERCHAIN_ETH_BRIDGE, message)
    }

    #[test]
    fn test_classify_fixture_message_zero_is_transfer() {
        assert_eq!(
            bridge_classify(&eth_payload()),
            EthPayload::Transfer(transfer())
        );
    }

    #[test]
    fn test_classify_fixture_message_one_is_not_bridge_message() {
        let logs = sent_message_logs();
        let event = logs[1]
            .log_decode::<v1::SentMessage>()
            .expect("fixture decodes")
            .inner
            .data;
        assert_eq!(
            classify(event.sender, event.target, &event.message),
            EthPayload::NotBridgeMessage
        );
    }

    #[test]
    fn test_classify_foreign_sender_or_target_is_not_bridge_message() {
        let payload = eth_payload();
        assert_eq!(
            classify(FROM, SUPERCHAIN_ETH_BRIDGE, &payload),
            EthPayload::NotBridgeMessage
        );
        assert_eq!(
            classify(SUPERCHAIN_ETH_BRIDGE, TO, &payload),
            EthPayload::NotBridgeMessage
        );
    }

    #[test]
    fn test_classify_bad_bridge_payloads_are_unrecognized() {
        let valid = eth_payload();

        let mut wrong_selector = valid.clone();
        wrong_selector[0] ^= 0xff;

        let mut dirty_from = valid.clone();
        dirty_from[4] = 1;
        let mut dirty_from_low = valid.clone();
        dirty_from_low[15] = 1;

        let mut dirty_to = valid.clone();
        dirty_to[36] = 1;
        let mut dirty_to_low = valid.clone();
        dirty_to_low[47] = 1;

        let mut long = valid.clone();
        long.push(0);

        for (name, payload) in [
            ("wrong selector", wrong_selector),
            ("99 bytes", valid[..99].to_vec()),
            ("101 bytes", long),
            ("empty", Vec::new()),
            ("dirty from word", dirty_from),
            ("dirty from word, last dirty byte", dirty_from_low),
            ("dirty to word", dirty_to),
            ("dirty to word, last dirty byte", dirty_to_low),
        ] {
            assert_eq!(
                bridge_classify(&payload),
                EthPayload::Unrecognized,
                "{name}"
            );
        }
    }

    #[test]
    fn test_classify_extreme_amounts_are_transfers() {
        for (word, amount) in [(0u8, U256::ZERO), (0xff, U256::MAX)] {
            let mut payload = eth_payload();
            payload[68..].fill(word);
            assert_eq!(
                bridge_classify(&payload),
                EthPayload::Transfer(EthTransfer {
                    from: FROM,
                    to: TO,
                    amount
                })
            );
        }
    }

    #[test]
    fn test_transfer_model_columns() {
        let key = key_from_message_hash(&alloy::primitives::B256::repeat_byte(7), 2004).unwrap();
        let model = transfer_model(
            &key,
            SOURCE_CHAIN_ID as i64,
            DESTINATION_CHAIN_ID as i64,
            &transfer(),
        )
        .expect("model builds");
        let amount: BigDecimal = "1000000000000000000".parse().unwrap();

        assert_eq!(model.message_id, Set(key.message_id));
        assert_eq!(model.bridge_id, Set(2004));
        assert_eq!(model.index, Set(0));
        assert_eq!(model.token_src_chain_id, Set(SOURCE_CHAIN_ID as i64));
        assert_eq!(model.token_dst_chain_id, Set(DESTINATION_CHAIN_ID as i64));
        assert_eq!(model.token_src_address, Set(Some(vec![0; 20])));
        assert_eq!(model.token_dst_address, Set(Some(vec![0; 20])));
        assert_eq!(model.src_amount, Set(Some(amount.clone())));
        assert_eq!(model.dst_amount, Set(Some(amount)));
        assert_eq!(model.sender_address, Set(Some(FROM.to_vec())));
        assert_eq!(model.recipient_address, Set(Some(TO.to_vec())));
        assert_eq!(model.token_ids, Set(None));
        assert_eq!(model.asset_linkage, Set(Some(TransferAssetLinkage::Mirror)));
        assert_eq!(model.stats_processed, Set(0));
    }

    #[test]
    fn test_transfer_model_amounts_are_exact_decimal_strings() {
        let key = key_from_message_hash(&alloy::primitives::B256::repeat_byte(7), 2004).unwrap();
        for (amount, expected) in [
            (U256::ZERO, "0".to_string()),
            (U256::MAX, U256::MAX.to_string()),
        ] {
            let model = transfer_model(
                &key,
                1,
                2,
                &EthTransfer {
                    from: FROM,
                    to: TO,
                    amount,
                },
            )
            .expect("model builds");
            let expected: BigDecimal = expected.parse().unwrap();
            assert_eq!(model.src_amount, Set(Some(expected.clone())));
            assert_eq!(model.dst_amount, Set(Some(expected)));
        }
    }

    #[test]
    fn test_check_relay_eth_equal_is_ok() {
        assert_eq!(
            check_relay_eth(
                &EthPayload::Transfer(transfer()),
                SOURCE_CHAIN_ID,
                Some(&relay_eth())
            ),
            RelayEthCheck::Ok
        );
    }

    #[test]
    fn test_check_relay_eth_each_changed_field_is_mismatch() {
        let mut from = relay_eth();
        from.from = TO;
        let mut to = relay_eth();
        to.to = FROM;
        let mut amount = relay_eth();
        amount.amount += U256::from(1);
        let mut source = relay_eth();
        source.source = U256::from(DESTINATION_CHAIN_ID);
        for relay in [from, to, amount, source] {
            assert_eq!(
                check_relay_eth(
                    &EthPayload::Transfer(transfer()),
                    SOURCE_CHAIN_ID,
                    Some(&relay)
                ),
                RelayEthCheck::Mismatch
            );
        }
    }

    #[test]
    fn test_check_relay_eth_presence_rules() {
        let payload = EthPayload::Transfer(transfer());
        assert_eq!(
            check_relay_eth(&payload, SOURCE_CHAIN_ID, None),
            RelayEthCheck::Missing
        );
        for other in [EthPayload::NotBridgeMessage, EthPayload::Unrecognized] {
            assert_eq!(
                check_relay_eth(&other, SOURCE_CHAIN_ID, Some(&relay_eth())),
                RelayEthCheck::Mismatch
            );
            assert_eq!(
                check_relay_eth(&other, SOURCE_CHAIN_ID, None),
                RelayEthCheck::Ok
            );
        }
    }

    #[test]
    fn test_native_sentinel_is_native_token_type() {
        assert_eq!(
            TokenType::from_address(NATIVE_SENTINEL.as_slice()),
            TokenType::Native
        );
    }
}
