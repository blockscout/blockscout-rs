use std::fmt::Display;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use zetachain_cctx_entity::sea_orm_active_enums::CoinType as DbCoinType;

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum CoinType {
    Zeta,
    Gas,
    ERC20,
    Cmd,
    NoAssetCall,
}

impl TryFrom<String> for CoinType {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let coin_type = match value.as_str() {
            "Zeta" => CoinType::Zeta,
            "Gas" => CoinType::Gas,
            "Erc20" => CoinType::ERC20,
            "Cmd" => CoinType::Cmd,
            "NoAssetCall" => CoinType::NoAssetCall,
            _ => return Err(anyhow::anyhow!("Invalid coin type: {}", value)),
        };

        Ok(coin_type)
    }
}

impl Display for CoinType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoinType::Zeta => write!(f, "Zeta"),
            CoinType::Gas => write!(f, "Gas"),
            CoinType::ERC20 => write!(f, "Erc20"),
            CoinType::Cmd => write!(f, "Cmd"),
            CoinType::NoAssetCall => write!(f, "NoAssetCall"),
        }
    }
}

impl TryFrom<CoinType> for DbCoinType {
    type Error = anyhow::Error;
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
    pub token_id: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub next_key: Option<String>,
    pub total: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CallOptions {
    pub gas_limit: String,
    pub is_arbitrary_call: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RevertOptions {
    pub revert_address: String,
    pub call_on_revert: bool,
    pub abort_address: String,
    pub revert_message: Option<String>,
    pub revert_gas_limit: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CrossChainTx {
    pub creator: String,
    pub index: String,
    pub zeta_fees: String,
    pub relayed_message: String,
    pub cctx_status: CctxStatus,
    pub inbound_params: InboundParams,
    pub outbound_params: Vec<OutboundParams>,
    pub revert_options: RevertOptions,
    pub protocol_contract_version: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Filters {
    pub status_reduced: Vec<String>,
    pub sender_address: Vec<String>,
    pub receiver_address: Vec<String>,
    pub asset: Vec<String>,
    pub coin_type: Vec<String>,
    pub source_chain_id: Vec<i32>,
    pub target_chain_id: Vec<i32>,
    pub start_timestamp: Option<i64>,
    pub end_timestamp: Option<i64>,
    pub token_symbol: Vec<String>,
    pub hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncProgress {
    pub historical_watermark_timestamp: NaiveDateTime,
    pub pending_status_updates_count: i64,
    pub pending_cctxs_count: i64,
    pub realtime_gaps_count: i64,
}

#[derive(Debug)]
pub struct CctxWithStatus {
    pub id: i32,
    pub index: String,
    pub root_id: Option<i32>,
    pub depth: i32,
    pub retries_number: i32,
}

// Token models for external service response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Token {
    pub zrc20_contract_address: String,
    pub asset: String,
    pub foreign_chain_id: String,
    pub decimals: i32,
    pub name: String,
    pub symbol: String,
    pub coin_type: CoinType,
    pub gas_limit: String,
    pub paused: bool,
    pub liquidity_cap: String,
    pub icon_url: Option<String>,
}

impl TryFrom<Token> for zetachain_cctx_entity::token::ActiveModel {
    type Error = anyhow::Error;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        use sea_orm::ActiveValue;
        use zetachain_cctx_entity::token;

        let coin_type: DbCoinType =
            DbCoinType::try_from(token.coin_type).map_err(|e| anyhow::anyhow!(e))?;
        Ok(token::ActiveModel {
            id: ActiveValue::NotSet,
            zrc20_contract_address: ActiveValue::Set(token.zrc20_contract_address),
            asset: ActiveValue::Set(token.asset),
            foreign_chain_id: ActiveValue::Set(token.foreign_chain_id.parse::<i32>()?),
            decimals: ActiveValue::Set(token.decimals),
            name: ActiveValue::Set(token.name),
            symbol: ActiveValue::Set(token.symbol),
            coin_type: ActiveValue::Set(coin_type),
            gas_limit: ActiveValue::Set(token.gas_limit),
            paused: ActiveValue::Set(token.paused),
            liquidity_cap: ActiveValue::Set(token.liquidity_cap),
            icon_url: ActiveValue::Set(token.icon_url),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedTokenResponse {
    #[serde(rename = "foreignCoins")]
    pub foreign_coins: Vec<Token>,
    pub pagination: Pagination,
}
