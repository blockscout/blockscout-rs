use alloy::{
    dyn_abi::{DynSolValue, JsonAbiExt},
    primitives::{Address, B256, Selector, U256},
};
use anyhow::{Context, Result};

use super::{
    abi::AbiRegistry,
    types::{DecodedPayload, DestinationTransferDetails},
};

pub(crate) trait PayloadProcessor: Send + Sync {
    fn matches(&self, dst_chain_id: i64, executor: Address) -> bool;
    fn decode(&self, ctx: &PayloadDecodeContext<'_>) -> Result<Option<DecodedPayload>>;
}

pub(crate) struct PayloadDecodeContext<'a> {
    pub(crate) dst_chain_id: i64,
    pub(crate) executor: Address,
    pub(crate) sender: Address,
    pub(crate) message_id: B256,
    pub(crate) application_calldata: &'a [u8],
    pub(crate) destination_transfer: Option<&'a DestinationTransferDetails>,
    pub(crate) abi_registry: &'a AbiRegistry,
}

pub(crate) struct OmnibridgePayloadProcessor {
    dst_chain_id: i64,
    mediator: Address,
}

impl OmnibridgePayloadProcessor {
    pub(crate) fn new(dst_chain_id: i64, mediator: Address) -> Self {
        Self {
            dst_chain_id,
            mediator,
        }
    }
}

impl PayloadProcessor for OmnibridgePayloadProcessor {
    fn matches(&self, dst_chain_id: i64, executor: Address) -> bool {
        self.dst_chain_id == dst_chain_id && self.mediator == executor
    }

    fn decode(&self, ctx: &PayloadDecodeContext<'_>) -> Result<Option<DecodedPayload>> {
        if ctx.application_calldata.len() < 4 {
            return Ok(None);
        }

        let selector = Selector::from_slice(&ctx.application_calldata[..4]);
        let Some(function) =
            ctx.abi_registry
                .function_for_selector(ctx.dst_chain_id, self.mediator, selector)
        else {
            return Ok(None);
        };

        let decoded = function
            .abi_decode_input(&ctx.application_calldata[4..])
            .with_context(|| format!("failed to decode Omnibridge calldata {}", function.name))?;

        let (token, recipient, amount) = match function.name.as_str() {
            "handleNativeTokens"
            | "handleNativeTokensAndCall"
            | "handleBridgedTokens"
            | "handleBridgedTokensAndCall" => (
                expect_address(&decoded, 0, &function.name)?,
                expect_address(&decoded, 1, &function.name)?,
                expect_uint(&decoded, 2, &function.name)?,
            ),
            "deployAndHandleBridgedTokens" | "deployAndHandleBridgedTokensAndCall" => (
                expect_address(&decoded, 0, &function.name)?,
                expect_address(&decoded, 4, &function.name)?,
                expect_uint(&decoded, 5, &function.name)?,
            ),
            _ => return Ok(None),
        };

        let (token_dst_address, final_recipient, dst_amount) = ctx
            .destination_transfer
            .map(|transfer| (Some(transfer.token), transfer.recipient, transfer.amount))
            .unwrap_or((None, recipient, amount));

        if token_dst_address.is_none() {
            tracing::debug!(
                dst_chain_id = ctx.dst_chain_id,
                executor = %ctx.executor,
                message_id = %ctx.message_id,
                "Omnibridge recipient resolution fell back to calldata recipient"
            );
        }

        Ok(Some(DecodedPayload::OmnibridgeTransfer {
            token_src_address: token,
            token_dst_address,
            src_amount: amount,
            dst_amount,
            sender: ctx.sender,
            recipient: final_recipient,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy::{
        dyn_abi::{DynSolValue, JsonAbiExt},
        json_abi::Function,
        primitives::{Address, B256, U256, address},
    };

    use crate::indexer::amb::{
        abi::{AbiRegistry, ContractAbi, ContractKind},
        types::{DecodedPayload, DestinationTransferDetails},
    };

    use super::{OmnibridgePayloadProcessor, PayloadDecodeContext, PayloadProcessor};

    #[test]
    fn function_decode_uses_selector_stripped_calldata() {
        let function: Function = serde_json::from_str(
            r#"{
                "inputs": [
                    {"internalType":"address","name":"_token","type":"address"},
                    {"internalType":"string","name":"_name","type":"string"},
                    {"internalType":"string","name":"_symbol","type":"string"},
                    {"internalType":"uint8","name":"_decimals","type":"uint8"},
                    {"internalType":"address","name":"_recipient","type":"address"},
                    {"internalType":"uint256","name":"_value","type":"uint256"}
                ],
                "name": "deployAndHandleBridgedTokens",
                "outputs": [],
                "stateMutability": "nonpayable",
                "type": "function"
            }"#,
        )
        .expect("function ABI");
        let token = address!("1111111111111111111111111111111111111111");
        let recipient = address!("2222222222222222222222222222222222222222");
        let values = vec![
            DynSolValue::Address(token),
            DynSolValue::String("Token".into()),
            DynSolValue::String("TKN".into()),
            DynSolValue::Uint(U256::from_limbs([18, 0, 0, 0]), 8),
            DynSolValue::Address(recipient),
            DynSolValue::Uint(U256::from_limbs([1000, 0, 0, 0]), 256),
        ];
        let calldata = function.abi_encode_input(&values).expect("encoded input");

        let decoded = function
            .abi_decode_input(&calldata[4..])
            .expect("selector-stripped decode");

        assert_eq!(decoded, values);
        assert!(function.abi_decode_input(&calldata).is_err());
    }

    #[test]
    fn omnibridge_payload_decode_works_before_destination_execution() {
        let function = handle_native_tokens_function();
        let mediator = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let token = address!("1111111111111111111111111111111111111111");
        let recipient = address!("2222222222222222222222222222222222222222");
        let sender = address!("3333333333333333333333333333333333333333");
        let amount = U256::from(1000);
        let calldata = function
            .abi_encode_input(&[
                DynSolValue::Address(token),
                DynSolValue::Address(recipient),
                DynSolValue::Uint(amount, 256),
            ])
            .expect("encoded input");
        let registry = registry_with_function(1, mediator, function);
        let processor = OmnibridgePayloadProcessor::new(1, mediator);

        let decoded = processor
            .decode(&PayloadDecodeContext {
                dst_chain_id: 1,
                executor: mediator,
                sender,
                message_id: B256::ZERO,
                application_calldata: &calldata,
                destination_transfer: None,
                abi_registry: &registry,
            })
            .expect("payload decode")
            .expect("omnibridge payload");

        assert_eq!(
            decoded,
            DecodedPayload::OmnibridgeTransfer {
                token_src_address: token,
                token_dst_address: None,
                src_amount: amount,
                dst_amount: amount,
                sender,
                recipient,
            }
        );
    }

    #[test]
    fn omnibridge_payload_decode_uses_destination_transfer_when_present() {
        let function = handle_native_tokens_function();
        let mediator = address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        let token = address!("1111111111111111111111111111111111111111");
        let calldata_recipient = address!("2222222222222222222222222222222222222222");
        let sender = address!("3333333333333333333333333333333333333333");
        let destination_token = address!("4444444444444444444444444444444444444444");
        let final_recipient = address!("5555555555555555555555555555555555555555");
        let calldata_amount = U256::from(1000);
        let destination_amount = U256::from(990);
        let calldata = function
            .abi_encode_input(&[
                DynSolValue::Address(token),
                DynSolValue::Address(calldata_recipient),
                DynSolValue::Uint(calldata_amount, 256),
            ])
            .expect("encoded input");
        let registry = registry_with_function(1, mediator, function);
        let processor = OmnibridgePayloadProcessor::new(1, mediator);
        let destination_transfer = DestinationTransferDetails {
            token: destination_token,
            recipient: final_recipient,
            amount: destination_amount,
        };

        let decoded = processor
            .decode(&PayloadDecodeContext {
                dst_chain_id: 1,
                executor: mediator,
                sender,
                message_id: B256::ZERO,
                application_calldata: &calldata,
                destination_transfer: Some(&destination_transfer),
                abi_registry: &registry,
            })
            .expect("payload decode")
            .expect("omnibridge payload");

        assert_eq!(
            decoded,
            DecodedPayload::OmnibridgeTransfer {
                token_src_address: token,
                token_dst_address: Some(destination_token),
                src_amount: calldata_amount,
                dst_amount: destination_amount,
                sender,
                recipient: final_recipient,
            }
        );
    }

    fn handle_native_tokens_function() -> Function {
        serde_json::from_str(
            r#"{
                "inputs": [
                    {"internalType":"address","name":"_token","type":"address"},
                    {"internalType":"address","name":"_recipient","type":"address"},
                    {"internalType":"uint256","name":"_value","type":"uint256"}
                ],
                "name": "handleNativeTokens",
                "outputs": [],
                "stateMutability": "nonpayable",
                "type": "function"
            }"#,
        )
        .expect("function ABI")
    }

    fn registry_with_function(chain_id: i64, mediator: Address, function: Function) -> AbiRegistry {
        let mut functions_by_selector = HashMap::new();
        functions_by_selector.insert(function.selector(), function);
        AbiRegistry::from_contracts_for_test(vec![ContractAbi {
            chain_id,
            address: mediator,
            kind: ContractKind::OmnibridgeMediator,
            events_by_topic: HashMap::new(),
            functions_by_selector,
        }])
    }
}

fn expect_address(values: &[DynSolValue], index: usize, function_name: &str) -> Result<Address> {
    match values.get(index) {
        Some(DynSolValue::Address(value)) => Ok(*value),
        other => {
            anyhow::bail!("expected address argument {index} in {function_name}, got {other:?}")
        }
    }
}

fn expect_uint(values: &[DynSolValue], index: usize, function_name: &str) -> Result<U256> {
    match values.get(index) {
        Some(DynSolValue::Uint(value, _)) => Ok(*value),
        other => anyhow::bail!("expected uint argument {index} in {function_name}, got {other:?}"),
    }
}
