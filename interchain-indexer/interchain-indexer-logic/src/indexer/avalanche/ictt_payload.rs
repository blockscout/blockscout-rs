// SPDX-License-Identifier: LicenseRef-Blockscout

//! Decode and classify the ICTT `TransferrerMessage` payload carried inside
//! `TeleporterMessage.message` (`message == abi.encode(TransferrerMessage)`).
//!
//! Pure decode + classification only: no DB access, no buffer types, no
//! metrics, no logging. Callers (`consolidation.rs`) own observability, per
//! `.memory-bank/rules/error-handling.md` ("log at the handling point").

use alloy::sol_types::SolValue;

use super::abi::{
    MultiHopCallMessage, MultiHopSendMessage, RegisterRemoteMessage, SingleHopCallMessage,
    SingleHopSendMessage, TransferrerMessage,
};

/// Decoded and classified ICTT payload.
///
/// Ordinal order is load-bearing and mirrors
/// `icm-contracts/contracts/ictt/interfaces/ITokenTransferrer.sol`:
/// `REGISTER_REMOTE = 0`, `SINGLE_HOP_SEND = 1`, `SINGLE_HOP_CALL = 2`,
/// `MULTI_HOP_SEND = 3`, `MULTI_HOP_CALL = 4`. `SINGLE_HOP_SEND = 1` is
/// confirmed against real mainnet bytes (see tests below).
#[derive(Debug)]
pub(crate) enum IcttPayload {
    RegisterRemote,
    SingleHopSend(SingleHopSendMessage),
    SingleHopCall(SingleHopCallMessage),
    // `MULTI_HOP_*` fields are decoded (as part of the canonicity guard and to
    // keep the ordinal map explicit) but never read: multi-hop reconstruction
    // is out of scope (Decision 2 — skipped with a metric), so only variant
    // identity matters today. Retained rather than discarded so a future
    // multi-hop consumer does not have to re-decode.
    #[allow(dead_code)]
    MultiHopSend(MultiHopSendMessage),
    #[allow(dead_code)]
    MultiHopCall(MultiHopCallMessage),
}

/// Whether a destination-side ICTT credit (`TokensWithdrawn` / `CallSucceeded`
/// / `CallFailed`) can ever be emitted for *this* message id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreditExpectation {
    Expected,
    NotExpected,
}

impl IcttPayload {
    /// `SINGLE_HOP_SEND` / `SINGLE_HOP_CALL` credit the recipient under this
    /// same message id. `REGISTER_REMOTE` never credits anyone.
    /// `MULTI_HOP_SEND` / `MULTI_HOP_CALL` arriving at a home is a routing
    /// intermediate: it re-sends under a *new* message id, so no credit
    /// belongs to this one (the `multiHopFallback` path, if routing fails,
    /// still emits `TokensWithdrawn` under this id — that is the `OR` in
    /// `is_ictt_complete`, not something this classification needs to know
    /// about).
    pub(crate) fn credit_expectation(&self) -> CreditExpectation {
        match self {
            IcttPayload::SingleHopSend(_) | IcttPayload::SingleHopCall(_) => {
                CreditExpectation::Expected
            }
            IcttPayload::RegisterRemote
            | IcttPayload::MultiHopSend(_)
            | IcttPayload::MultiHopCall(_) => CreditExpectation::NotExpected,
        }
    }
}

/// Why a payload was not accepted as an ICTT transfer message. Maps 1:1 onto
/// a metric `reason` label at the call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadRejection {
    OuterDecodeFailed,
    OuterNotCanonical,
    UnknownMessageType(u8),
    InnerDecodeFailed,
    InnerNotCanonical,
}

/// Decode `TeleporterMessage.message` as `abi.encode(TransferrerMessage)`.
///
/// Four-layer false-positive guard, in order:
/// 1. `TransferrerMessage::abi_decode_validate` — token-shape validation.
/// 2. Canonicity round trip (`abi_decode_validate` type-checks tokens but does
///    **not** reject trailing bytes or non-canonical offsets — this is the
///    layer that does).
/// 3. Ordinal range check (`0..=4`).
/// 4. Inner struct decode with the same validate + round-trip pair.
pub(crate) fn decode_transferrer_message(bytes: &[u8]) -> Result<IcttPayload, PayloadRejection> {
    let outer = TransferrerMessage::abi_decode_validate(bytes)
        .map_err(|_| PayloadRejection::OuterDecodeFailed)?;

    if outer.abi_encode().as_slice() != bytes {
        return Err(PayloadRejection::OuterNotCanonical);
    }

    match outer.messageType {
        0 => {
            decode_inner::<RegisterRemoteMessage>(&outer.payload)?;
            Ok(IcttPayload::RegisterRemote)
        }
        1 => decode_inner::<SingleHopSendMessage>(&outer.payload).map(IcttPayload::SingleHopSend),
        2 => decode_inner::<SingleHopCallMessage>(&outer.payload).map(IcttPayload::SingleHopCall),
        3 => decode_inner::<MultiHopSendMessage>(&outer.payload).map(IcttPayload::MultiHopSend),
        4 => decode_inner::<MultiHopCallMessage>(&outer.payload).map(IcttPayload::MultiHopCall),
        other => Err(PayloadRejection::UnknownMessageType(other)),
    }
}

/// Decode + canonicity round trip for the inner (payload-of-payload) struct.
/// Shared by every `TransferrerMessageType` variant: static structs
/// (`SingleHopSendMessage`, `RegisterRemoteMessage`, `MultiHopSendMessage`,
/// encoded inline) and dynamic ones (`SingleHopCallMessage`,
/// `MultiHopCallMessage`, offset-wrapped because they contain `bytes`) both
/// round-trip through the same `SolValue::abi_decode_validate` /
/// `abi_encode` pair, because that is exactly what `abi.encode(x)` means.
fn decode_inner<T>(bytes: &[u8]) -> Result<T, PayloadRejection>
where
    T: SolValue<SolType = T> + alloy::sol_types::SolType<RustType = T>,
{
    let decoded = <T as SolValue>::abi_decode_validate(bytes)
        .map_err(|_| PayloadRejection::InnerDecodeFailed)?;

    if <T as SolValue>::abi_encode(&decoded).as_slice() != bytes {
        return Err(PayloadRejection::InnerNotCanonical);
    }

    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{Address, U256, address};

    /// Real mainnet bytes, NUMINE tx
    /// `0x596767faf86be9f297773c628bded9d3d2b4928ccc6eba852b538afb2a29a52c`,
    /// block 269775, log index 2 (`ReceiveCrossChainMessage`, messageID
    /// `0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07`).
    /// Do not hand-encode this: it is what pins the ordinal map, the struct
    /// layout, and the `abi_decode`-vs-`abi_decode_params` choice all at once.
    const MAINNET_SINGLE_HOP_SEND: &str = "\
        0000000000000000000000000000000000000000000000000000000000000020\
        0000000000000000000000000000000000000000000000000000000000000001\
        0000000000000000000000000000000000000000000000000000000000000040\
        0000000000000000000000000000000000000000000000000000000000000040\
        000000000000000000000000718245e1a9b44909f89b130e29a8908a9d6bec41\
        0000000000000000000000000000000000000000000000012c38ebbb5b754000";

    fn mainnet_bytes() -> Vec<u8> {
        alloy::hex::decode(MAINNET_SINGLE_HOP_SEND).unwrap()
    }

    #[test]
    fn test_decode_transferrer_message_mainnet_fixture_decodes_single_hop_send() {
        let payload = decode_transferrer_message(&mainnet_bytes()).unwrap();

        match payload {
            IcttPayload::SingleHopSend(msg) => {
                assert_eq!(
                    msg.recipient,
                    address!("0x718245e1a9b44909f89b130e29a8908a9d6bec41")
                );
                assert_eq!(msg.amount, U256::from(21633300000000000000u128));
            }
            other => panic!("expected SingleHopSend, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_transferrer_message_empty_bytes_rejects_outer_decode() {
        let result = decode_transferrer_message(&[]);
        assert_eq!(result.unwrap_err(), PayloadRejection::OuterDecodeFailed);
    }

    #[test]
    fn test_decode_transferrer_message_31_bytes_rejects_outer_decode() {
        let result = decode_transferrer_message(&[0u8; 31]);
        assert_eq!(result.unwrap_err(), PayloadRejection::OuterDecodeFailed);
    }

    #[test]
    fn test_decode_transferrer_message_unknown_message_type_rejects() {
        let mut bytes = mainnet_bytes();
        // Word 2 (bytes 32..64) holds `messageType`; flip it to 5.
        bytes[63] = 5;
        let result = decode_transferrer_message(&bytes);
        assert_eq!(result.unwrap_err(), PayloadRejection::UnknownMessageType(5));
    }

    #[test]
    fn test_decode_transferrer_message_truncated_inner_payload_rejects() {
        // Canonical outer envelope (messageType = 1, freshly re-encoded so its
        // own round trip is guaranteed to hold), but the inner payload is
        // truncated to fewer bytes than `SingleHopSendMessage` (address +
        // uint256 = 64 bytes) needs.
        let outer = TransferrerMessage {
            messageType: 1,
            payload: vec![0u8; 16].into(),
        };
        let bytes = outer.abi_encode();

        let result = decode_transferrer_message(&bytes);
        assert!(
            matches!(
                result,
                Err(PayloadRejection::InnerDecodeFailed | PayloadRejection::InnerNotCanonical)
            ),
            "expected an inner-decode rejection, got {result:?}"
        );
    }

    #[test]
    fn test_decode_transferrer_message_trailing_garbage_rejects_outer_canonicity() {
        let mut bytes = mainnet_bytes();
        bytes.extend_from_slice(&[0xAB; 32]);
        let result = decode_transferrer_message(&bytes);
        assert_eq!(result.unwrap_err(), PayloadRejection::OuterNotCanonical);
    }

    #[test]
    fn test_decode_transferrer_message_single_hop_call_decodes_fields() {
        let payload = TransferrerMessage {
            messageType: 2,
            payload: SingleHopCallMessage {
                sourceBlockchainID: Default::default(),
                originTokenTransferrerAddress: Address::ZERO,
                originSenderAddress: address!("0x1111111111111111111111111111111111111111"),
                recipientContract: address!("0x2222222222222222222222222222222222222222"),
                amount: U256::from(1_000u64),
                recipientPayload: Default::default(),
                recipientGasLimit: U256::from(0u64),
                fallbackRecipient: address!("0x3333333333333333333333333333333333333333"),
            }
            .abi_encode()
            .into(),
        };
        let bytes = payload.abi_encode();

        let decoded = decode_transferrer_message(&bytes).unwrap();
        match decoded {
            IcttPayload::SingleHopCall(msg) => {
                assert_eq!(
                    msg.originSenderAddress,
                    address!("0x1111111111111111111111111111111111111111")
                );
                assert_eq!(
                    msg.recipientContract,
                    address!("0x2222222222222222222222222222222222222222")
                );
                assert_eq!(
                    msg.fallbackRecipient,
                    address!("0x3333333333333333333333333333333333333333")
                );
                assert_eq!(msg.amount, U256::from(1_000u64));
            }
            other => panic!("expected SingleHopCall, got {other:?}"),
        }
    }

    #[test]
    fn test_credit_expectation_single_hop_send_is_expected() {
        let payload = IcttPayload::SingleHopSend(SingleHopSendMessage {
            recipient: Address::ZERO,
            amount: U256::ZERO,
        });
        assert_eq!(payload.credit_expectation(), CreditExpectation::Expected);
    }

    #[test]
    fn test_credit_expectation_single_hop_call_is_expected() {
        let payload = IcttPayload::SingleHopCall(SingleHopCallMessage {
            sourceBlockchainID: Default::default(),
            originTokenTransferrerAddress: Address::ZERO,
            originSenderAddress: Address::ZERO,
            recipientContract: Address::ZERO,
            amount: U256::ZERO,
            recipientPayload: Default::default(),
            recipientGasLimit: U256::ZERO,
            fallbackRecipient: Address::ZERO,
        });
        assert_eq!(payload.credit_expectation(), CreditExpectation::Expected);
    }

    #[test]
    fn test_credit_expectation_register_remote_is_not_expected() {
        assert_eq!(
            IcttPayload::RegisterRemote.credit_expectation(),
            CreditExpectation::NotExpected
        );
    }

    #[test]
    fn test_credit_expectation_multi_hop_send_is_not_expected() {
        let payload = IcttPayload::MultiHopSend(MultiHopSendMessage {
            destinationBlockchainID: Default::default(),
            destinationTokenTransferrerAddress: Address::ZERO,
            recipient: Address::ZERO,
            amount: U256::ZERO,
            secondaryFee: U256::ZERO,
            secondaryGasLimit: U256::ZERO,
            multiHopFallback: Address::ZERO,
        });
        assert_eq!(payload.credit_expectation(), CreditExpectation::NotExpected);
    }

    #[test]
    fn test_credit_expectation_multi_hop_call_is_not_expected() {
        let payload = IcttPayload::MultiHopCall(MultiHopCallMessage {
            originSenderAddress: Address::ZERO,
            destinationBlockchainID: Default::default(),
            destinationTokenTransferrerAddress: Address::ZERO,
            recipientContract: Address::ZERO,
            amount: U256::ZERO,
            recipientPayload: Default::default(),
            recipientGasLimit: U256::ZERO,
            fallbackRecipient: Address::ZERO,
            secondaryRequiredGasLimit: U256::ZERO,
            multiHopFallback: Address::ZERO,
            secondaryFee: U256::ZERO,
        });
        assert_eq!(payload.credit_expectation(), CreditExpectation::NotExpected);
    }
}
