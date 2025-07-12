use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use zetachain_cctx_entity::{cctx_status, cross_chain_tx, inbound_params, outbound_params, revert_options, sea_orm_active_enums::CoinType as DbCoinType};

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedCCTXResponse {
    #[serde(rename = "CrossChainTx")]
    pub cross_chain_tx: Vec<CrossChainTx>,
    pub pagination: Pagination,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CCTXResponse {
    #[serde(rename = "CrossChainTx")]
    pub cross_chain_tx: CrossChainTx,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CoinType {
    Zeta,
    Gas,
    ERC20,
    Cmd,
    NoAssetCall,
}

impl TryFrom<CoinType> for DbCoinType {
    type Error = String;
    fn try_from(value: CoinType) -> Result<Self, Self::Error> {
        match value {
            CoinType::Zeta => Ok(DbCoinType::Zeta),
            CoinType::Gas => Ok(DbCoinType::Gas),
            CoinType::ERC20 => Ok(DbCoinType::Erc20),
            CoinType::Cmd => Ok(DbCoinType::Cmd),
            CoinType::NoAssetCall => Ok(DbCoinType::NoAssetCall),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InboundHashToCctxResponse {
    #[serde(rename = "CrossChainTxs")]
    pub cross_chain_txs: Vec<CrossChainTx>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CctxShort {
    pub id: i32,
    pub index: String,
    pub root_id: Option<i32>,
    pub depth: i32,
    pub retries_number: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CrossChainTx {
    pub creator: String,
    pub index: String,
    pub zeta_fees: String,
    pub relayed_message: String,
    pub cctx_status: CctxStatus,
    pub inbound_params: InboundParams,
    pub outbound_params: Vec<OutboundParams>,
    pub protocol_contract_version: String,
    pub revert_options: RevertOptions,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CctxStatus {
    pub status: String,
    pub status_message: String,
    pub error_message: String,
    #[serde(rename = "lastUpdate_timestamp")]
    pub last_update_timestamp: String,
    #[serde(rename = "isAbortRefunded")]
    pub is_abort_refunded: bool,
    pub created_timestamp: String,
    pub error_message_revert: String,
    pub error_message_abort: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InboundParams {
    pub sender: String,
    pub sender_chain_id: String,
    pub tx_origin: String,
    pub coin_type: CoinType,
    pub asset: String,
    pub amount: String,
    pub observed_hash: String,
    pub observed_external_height: String,
    pub ballot_index: String,
    pub finalized_zeta_height: String,
    pub tx_finalization_status: String,
    pub is_cross_chain_call: bool,
    pub status: String,
    pub confirmation_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OutboundParams {
    pub receiver: String,
    #[serde(rename = "receiver_chainId")]
    pub receiver_chain_id: String,
    pub coin_type: CoinType,
    pub amount: String,
    pub tss_nonce: String,
    pub gas_limit: String,
    pub gas_price: String,
    pub gas_priority_fee: String,
    pub hash: String,
    pub ballot_index: String,
    pub observed_external_height: String,
    pub gas_used: String,
    pub effective_gas_price: String,
    pub effective_gas_limit: String,
    pub tss_pubkey: String,
    pub tx_finalization_status: String,
    pub call_options: Option<CallOptions>,
    pub confirmation_mode: String,
}

#[derive(Debug, Serialize, Deserialize,Clone)]
pub struct CallOptions {
    pub gas_limit: String,
    pub is_arbitrary_call: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RevertOptions {
    pub revert_address: String,
    pub call_on_revert: bool,
    pub abort_address: String,
    pub revert_message: Option<String>,
    pub revert_gas_limit: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub next_key: Option<String>,
    pub total: String,
}
#[derive(Debug)]
pub struct CompleteCctx {
    pub cctx: cross_chain_tx::Model,
    pub status: cctx_status::Model,
    pub inbound: inbound_params::Model,
    pub outbounds: Vec<outbound_params::Model>,
    pub revert: revert_options::Model,
    pub related: Vec<RelatedCctx>,
}

#[derive(Debug)]
pub struct CctxListItem {
    pub index: String,
    pub status: String,
    pub amount: String,
    pub last_update_timestamp: NaiveDateTime,
    pub source_chain_id: String,
    pub target_chain_id: String,
}

#[derive(Debug)]
pub struct RelatedCctx {
    pub index: String,
    pub depth: i32,
    pub source_chain_id: String,
    pub status: String,
    pub inbound_amount: String,
    pub inbound_coin_type: String,
    pub inbound_asset: String,
    pub outbound_params: Vec<RelatedOutboundParams>,
}

#[derive(Debug)]
pub struct RelatedOutboundParams {
    pub amount: String,
    pub chain_id: String,
    pub coin_type: String,
}

#[derive(Debug)]
pub struct CctxWithStatus {
    pub id: i32,
    pub index: String,
    pub root_id: Option<i32>,
    pub depth: i32,
    pub retries_number: i32,
}