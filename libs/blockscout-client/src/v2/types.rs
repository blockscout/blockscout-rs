use crate::deserialize_null_default;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Address {
    pub creator_address_hash: Option<ethers_core::types::Address>,
    pub creation_tx_hash: Option<ethers_core::types::TxHash>,
    pub token: Option<TokenInfo>,
    pub coin_balance: Option<String>,
    pub exchange_rate: Option<String>,
    pub implementation_address: Option<ethers_core::types::Address>,
    pub block_number_balance_updated_at: Option<u128>,
    pub hash: ethers_core::types::Address,
    pub implementation_name: Option<String>,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_contract: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub private_tags: Vec<AddressTag>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub watchlist_names: Vec<WatchlistName>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub public_tags: Vec<AddressTag>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_verified: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_beacon_chain_withdrawals: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_custom_methods_read: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_custom_methods_write: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_decompiled_code: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_logs: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_methods_read: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_methods_write: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_methods_read_proxy: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_methods_write_proxy: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_token_transfers: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_tokens: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_validated_blocks: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AddressParam {
    pub hash: ethers_core::types::Address,
    pub implementation_name: Option<String>,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_contract: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub private_tags: Vec<AddressTag>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub watchlist_names: Vec<WatchlistName>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub public_tags: Vec<AddressTag>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_verified: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AddressTag {
    pub address_hash: ethers_core::types::Address,
    pub display_name: Option<String>,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ConstructorArguments {
    // TODO
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct DecodedInput {
    // TODO
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ImportSmartContractResponse {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Fee {
    pub r#type: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SmartContract {
    pub verified_twin_address_hash: Option<ethers_core::types::Address>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_verified: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_changed_bytecode: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_partially_verified: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_fully_verified: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_verified_via_sourcify: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_verified_via_eth_bytecode_db: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_vyper_contract: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub is_self_destructed: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub can_be_visualized_via_sol2uml: bool,
    pub minimal_proxy_address_hash: Option<ethers_core::types::Address>,
    pub sourcify_repo_url: Option<String>,
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub optimization_enabled: bool,
    pub optimizations_runs: Option<u32>,
    pub compiler_version: Option<String>,
    pub evm_version: Option<String>,
    pub verified_at: Option<String>,
    pub abi: Option<serde_json::Value>,
    pub source_code: Option<String>,
    pub file_path: Option<String>,
    pub compiler_settings: Option<serde_json::Value>,
    pub constructor_args: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub additional_sources: Vec<BTreeMap<String, String>>,
    // pub decoded_constructor_args: Vec<ConstructorArguments>,
    pub deployed_bytecode: Option<ethers_core::types::Bytes>,
    pub creation_bytecode: Option<ethers_core::types::Bytes>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub external_libraries: Vec<BTreeMap<String, ethers_core::types::Address>>,
    pub language: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TokenInfo {
    pub circulating_market_cap: Option<String>,
    pub icon_url: Option<String>,
    pub name: Option<String>,
    pub decimals: Option<String>,
    pub symbol: Option<String>,
    pub address: ethers_core::types::Address,
    pub r#type: Option<String>,
    pub holders: Option<String>,
    pub exchange_rate: Option<String>,
    pub total_supply: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TokenTransfer {
    // TODO
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Transaction {
    pub timestamp: Option<String>,
    pub fee: Option<Fee>,
    pub gas_limit: Option<String>,
    pub block: u128,
    pub status: Option<String>,
    pub method: Option<String>,
    pub confirmations: Option<u128>,
    pub r#type: Option<u8>,
    pub exchange_rate: Option<String>,
    pub to: Option<AddressParam>,
    pub tx_burnt_fee: Option<String>,
    pub max_fee_per_gas: Option<String>,
    pub result: Option<String>,
    pub hash: Option<ethers_core::types::TxHash>,
    pub gas_price: Option<String>,
    pub priority_fee: Option<String>,
    pub base_fee_per_gas: Option<String>,
    pub from: Option<AddressParam>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub token_transfers: Vec<TokenTransfer>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub tx_types: Vec<String>,
    pub gas_used: Option<String>,
    pub created_contract: Option<AddressParam>,
    pub position: u32,
    pub nonce: Option<u128>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub has_error_in_internal_txs: bool,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub actions: Vec<TransactionAction>,
    pub decoded_input: Option<DecodedInput>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub token_transfers_overflow: bool,
    pub raw_input: Option<ethers_core::types::Bytes>,
    pub value: Option<String>,
    pub max_priority_fee_per_gas: Option<String>,
    pub revert_reason: Option<String>,
    // pub confirmation_duration: Option<Vec<String>>,
    pub tx_tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TransactionAction {
    // TODO
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct WatchlistName {
    pub display_name: String,
    pub label: String,
}
