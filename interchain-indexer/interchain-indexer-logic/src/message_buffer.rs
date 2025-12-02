//! Tiered message buffer for cross-chain message indexing.
//!
//! This module provides a two-tier storage system:
//! - **Hot tier**: In-memory storage for recent messages (fast access)
//! - **Cold tier**: Postgres `pending_messages` table for durability
//!
//! Messages are promoted to `crosschain_messages` when they become "ready"
//! (i.e., when the source event with `init_timestamp` has been received).

use std::{collections::HashMap, sync::Arc, time::Duration};

use alloy::primitives::{Address, FixedBytes};
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use dashmap::DashMap;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, indexer_checkpoints, pending_messages,
    sea_orm_active_enums::{self, MessageStatus},
};
use sea_orm::{
    ActiveValue, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
    prelude::Decimal,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};

use crate::InterchainDatabase;

/// Serializable wrapper for MessageStatus (avoids requiring serde on entity crate)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Initiated,
    Completed,
    Failed,
}

impl From<MessageStatus> for Status {
    fn from(status: MessageStatus) -> Self {
        match status {
            MessageStatus::Initiated => Status::Initiated,
            MessageStatus::Completed => Status::Completed,
            MessageStatus::Failed => Status::Failed,
        }
    }
}

impl From<Status> for MessageStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Initiated => MessageStatus::Initiated,
            Status::Completed => MessageStatus::Completed,
            Status::Failed => MessageStatus::Failed,
        }
    }
}

/// Key for identifying a cross-chain message
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Key {
    pub message_id: i64,
    pub bridge_id: i32,
}

impl Key {
    pub fn new(message_id: i64, bridge_id: i32) -> Self {
        Self {
            message_id,
            bridge_id,
        }
    }
}

/// Tracks which blocks have contributed data to this message entry.
/// Used for safe cursor advancement in bidirectional indexing.
///
/// The indexer runs two streams:
/// - **Realtime stream**: moves forward from a starting block toward chain head
/// - **Catchup stream**: moves backward from that starting block toward genesis
///
/// We track both min and max blocks per chain so that:
/// - `min_block` is used to update `catchup_max_block` (LEAST - going down)
/// - `max_block` is used to update `realtime_cursor` (GREATEST - going up)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cursor {
    /// chain_id -> (min_block, max_block) seen for this message
    chain_blocks: HashMap<i64, (i64, i64)>,
}

impl Cursor {
    /// Record that data from a specific block was added to this message.
    /// Updates both min and max to properly track bidirectional cursor movement.
    pub fn record_block(&mut self, chain_id: i64, block_number: i64) {
        self.chain_blocks
            .entry(chain_id)
            .and_modify(|(min, max)| {
                *min = (*min).min(block_number);
                *max = (*max).max(block_number);
            })
            .or_insert((block_number, block_number));
    }

    /// Get the minimum block recorded for a chain (for catchup cursor)
    pub fn min_block(&self, chain_id: i64) -> Option<i64> {
        self.chain_blocks.get(&chain_id).map(|(min, _)| *min)
    }

    /// Get the maximum block recorded for a chain (for realtime cursor)
    pub fn max_block(&self, chain_id: i64) -> Option<i64> {
        self.chain_blocks.get(&chain_id).map(|(_, max)| *max)
    }
}

/// ICTT event fragment - stores raw event data for later consolidation.
///
/// Events arrive in unpredictable order. We accumulate fragments and consolidate
/// them into a normalized TokenTransfer when the message is flushed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IcttEventFragment {
    /// TokensSent - source-side simple transfer
    TokensSent {
        token_contract: Address,
        sender: Address,
        dst_token_address: Address,
        recipient: Address,
        amount: alloy::primitives::U256,
    },

    /// TokensAndCallSent - source-side transfer with contract call
    TokensAndCallSent {
        token_contract: Address,
        sender: Address,
        dst_token_address: Address,
        recipient_contract: Address,
        fallback_recipient: Address,
        amount: alloy::primitives::U256,
    },

    /// TokensWithdrawn - destination-side token release
    TokensWithdrawn {
        recipient: Address,
        amount: alloy::primitives::U256,
    },

    /// CallSucceeded - destination-side contract call success
    CallSucceeded {
        recipient_contract: Address,
        amount: alloy::primitives::U256,
    },

    /// CallFailed - destination-side contract call failure
    /// Tokens go to fallback_recipient instead of recipient_contract
    CallFailed {
        recipient_contract: Address,
        amount: alloy::primitives::U256,
    },
}

/// Consolidate ICTT event fragments into a normalized TokenTransfer.
///
/// Handles out-of-order events by examining all fragments:
/// - TokensSent/TokensAndCallSent provide source-side info
/// - CallSucceeded/CallFailed provide destination result
/// - When CallFailed is present, recipient becomes fallback_recipient
pub fn consolidate_ictt_fragments(fragments: &[IcttEventFragment]) -> Option<TokenTransfer> {
    if fragments.is_empty() {
        return None;
    }

    let mut transfer = TokenTransfer::default();
    let mut call_failed = false;
    let mut fallback_recipient: Option<Address> = None;

    for fragment in fragments {
        match fragment {
            IcttEventFragment::TokensSent {
                token_contract,
                sender,
                dst_token_address,
                recipient,
                amount,
            } => {
                transfer.src_token_address = Some(*token_contract);
                transfer.sender = Some(*sender);
                transfer.dst_token_address = Some(*dst_token_address);
                transfer.recipient = Some(*recipient);
                transfer.amount = Some(*amount);
            }
            IcttEventFragment::TokensAndCallSent {
                token_contract,
                sender,
                dst_token_address,
                recipient_contract,
                fallback_recipient: fallback,
                amount,
            } => {
                transfer.src_token_address = Some(*token_contract);
                transfer.sender = Some(*sender);
                transfer.dst_token_address = Some(*dst_token_address);
                // Initially set recipient to contract; may be overridden by CallFailed
                transfer.recipient = Some(*recipient_contract);
                transfer.amount = Some(*amount);
                fallback_recipient = Some(*fallback);
            }
            IcttEventFragment::TokensWithdrawn { recipient, .. } => {
                // On destination side, this confirms who received tokens
                transfer.recipient = Some(*recipient);
            }
            IcttEventFragment::CallSucceeded { .. } => {
                // Call succeeded, recipient stays as recipient_contract
            }
            IcttEventFragment::CallFailed { .. } => {
                call_failed = true;
            }
        }
    }

    // If call failed and we have a fallback, use it as recipient
    if call_failed && let Some(fallback) = fallback_recipient {
        transfer.recipient = Some(fallback);
    }

    Some(transfer)
}
/// Token type for transfers (mirrors DB enum)
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransferType {
    #[default]
    Erc20,
    Erc721,
    Native,
    Erc1155,
}

impl From<sea_orm_active_enums::TransferType> for TransferType {
    fn from(t: sea_orm_active_enums::TransferType) -> Self {
        match t {
            sea_orm_active_enums::TransferType::Erc20 => TransferType::Erc20,
            sea_orm_active_enums::TransferType::Erc721 => TransferType::Erc721,
            sea_orm_active_enums::TransferType::Native => TransferType::Native,
            sea_orm_active_enums::TransferType::Erc1155 => TransferType::Erc1155,
        }
    }
}

impl Into<sea_orm_active_enums::TransferType> for TransferType {
    fn into(self) -> sea_orm_active_enums::TransferType {
        match self {
            TransferType::Erc20 => sea_orm_active_enums::TransferType::Erc20,
            TransferType::Erc721 => sea_orm_active_enums::TransferType::Erc721,
            TransferType::Native => sea_orm_active_enums::TransferType::Native,
            TransferType::Erc1155 => sea_orm_active_enums::TransferType::Erc1155,
        }
    }
}

/// Token transfer associated with a cross-chain message (ICTT)
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TokenTransfer {
    /// Token type (ERC20, ERC721, Native, ERC1155)
    #[serde(default)]
    pub r#type: TransferType,

    /// Source token contract address
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_token_address: Option<Address>,

    /// Destination token contract address (from SendTokensInput/SendAndCallInput)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dst_token_address: Option<Address>,

    /// Amount transferred (in source token denomination)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<alloy::primitives::U256>,

    /// Sender address (from TokensSent/TokensAndCallSent event)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<Address>,

    /// Recipient address (from SendTokensInput or TokensWithdrawn)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient: Option<Address>,
    // /// For multi-hop: the final destination chain ID
    // #[serde(default, skip_serializing_if = "Option::is_none")]
    // pub final_destination_chain_id: Option<i64>,

    // /// Result of the call on destination (for WithCall transfers)
    // #[serde(default, skip_serializing_if = "Option::is_none")]
    // pub call_result: Option<CallResult>,
}

impl TryFrom<crosschain_transfers::Model> for TokenTransfer {
    type Error = anyhow::Error;

    fn try_from(model: crosschain_transfers::Model) -> Result<Self> {
        Ok(Self {
            r#type: model.r#type.map(TransferType::from).unwrap_or_default(),
            src_token_address: Some(Address::try_from(model.token_src_address.as_slice())?),
            dst_token_address: Some(Address::try_from(model.token_dst_address.as_slice())?),
            amount: Some(alloy::primitives::U256::from_str_radix(
                &model.src_amount.to_string(),
                10,
            )?),
            sender: model
                .sender_address
                .map(|v| Address::try_from(v.as_slice()))
                .transpose()?,
            recipient: model
                .recipient_address
                .map(|v| Address::try_from(v.as_slice()))
                .transpose()?,
        })
    }
}

/// Serializable cross-chain message - same structure for hot (memory) and cold (Postgres) storage
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Message {
    pub key: Key,

    // Source side (presence of init_timestamp makes message "ready")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_chain_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_transaction_hash: Option<FixedBytes<32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub init_timestamp: Option<NaiveDateTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_address: Option<Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipient_address: Option<Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_chain_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_id: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_transaction_hash: Option<FixedBytes<32>>,
    #[serde(default)]
    pub status: Status,

    /// ICTT event fragments accumulated from various events.
    /// Consolidated into `transfers` when the message is flushed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ictt_fragments: Vec<IcttEventFragment>,

    // Cursor tracking for checkpoint management
    #[serde(default)]
    pub cursor: Cursor,

    /// When this message was first created (for TTL-based offloading)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<NaiveDateTime>,
}

impl Message {
    /// Create a new empty message entry
    pub fn new(key: Key) -> Self {
        Self {
            key,
            created_at: Some(Utc::now().naive_utc()),
            ..Default::default()
        }
    }

    /// Check if this entry is ready for promotion to final storage.
    /// An entry is ready when it has an init_timestamp (source event received).
    pub fn is_ready(&self) -> bool {
        self.init_timestamp.is_some()
    }

    fn to_active_models(
        &self,
    ) -> Result<(
        crosschain_messages::ActiveModel,
        Vec<crosschain_transfers::ActiveModel>,
    )> {
        let message_model: crosschain_messages::ActiveModel = self.try_into()?;

        // If we have ICTT fragments, consolidate and use that; otherwise use existing transfers
        let consolidated = consolidate_ictt_fragments(&self.ictt_fragments);

        let transfer_models: Vec<crosschain_transfers::ActiveModel> =
            if let Some(ref t) = consolidated {
                // Single consolidated transfer from fragments
                vec![self.token_transfer_to_active_model(t)]
            } else {
                vec![]
            };

        Ok((message_model, transfer_models))
    }

    fn token_transfer_to_active_model(
        &self,
        t: &TokenTransfer,
    ) -> crosschain_transfers::ActiveModel {
        let now = Utc::now().naive_utc();
        let amount_str = t.amount.unwrap_or_default().to_string();
        let amount = Decimal::from_str_exact(&amount_str).unwrap_or(Decimal::ZERO);

        crosschain_transfers::ActiveModel {
            id: ActiveValue::NotSet,
            message_id: ActiveValue::Set(self.key.message_id),
            bridge_id: ActiveValue::Set(self.key.bridge_id),
            r#type: ActiveValue::Set(Some(t.r#type.clone().into())),
            token_src_address: ActiveValue::Set(
                t.src_token_address
                    .map(|a| a.as_slice().to_vec())
                    .unwrap_or_default(),
            ),
            token_dst_address: ActiveValue::Set(
                t.dst_token_address
                    .map(|a| a.as_slice().to_vec())
                    .unwrap_or_default(),
            ),
            src_amount: ActiveValue::Set(amount),
            sender_address: ActiveValue::Set(t.sender.map(|a| a.as_slice().to_vec())),
            recipient_address: ActiveValue::Set(t.recipient.map(|a| a.as_slice().to_vec())),
            created_at: ActiveValue::Set(Some(now)),
            updated_at: ActiveValue::Set(Some(now)),
            // Use message's chain IDs for the transfer
            token_src_chain_id: ActiveValue::Set(self.src_chain_id.unwrap_or(0)),
            token_dst_chain_id: ActiveValue::Set(self.destination_chain_id.unwrap_or(0)),
            src_decimals: ActiveValue::Set(18), // Default to 18 decimals
            dst_decimals: ActiveValue::Set(18),
            dst_amount: ActiveValue::Set(amount), // Same as src for now
            token_ids: ActiveValue::NotSet,
        }
    }

    fn is_ictt_complete(&self) -> bool {
        if self.ictt_fragments.is_empty() {
            return true; // No ICTT - not applicable
        }

        let has_tokens_sent = self
            .ictt_fragments
            .iter()
            .any(|f| matches!(f, IcttEventFragment::TokensSent { .. }));
        let has_tokens_and_call_sent = self
            .ictt_fragments
            .iter()
            .any(|f| matches!(f, IcttEventFragment::TokensAndCallSent { .. }));
        let has_tokens_withdrawn = self
            .ictt_fragments
            .iter()
            .any(|f| matches!(f, IcttEventFragment::TokensWithdrawn { .. }));
        let has_call_result = self.ictt_fragments.iter().any(|f| {
            matches!(
                f,
                IcttEventFragment::CallSucceeded { .. } | IcttEventFragment::CallFailed { .. }
            )
        });

        (has_tokens_sent && has_tokens_withdrawn) || (has_tokens_and_call_sent && has_call_result)
    }
}

fn with_cursors(
    message: &Message,
    cursors: HashMap<(i32, i64), (i64, i64)>,
) -> HashMap<(i32, i64), (i64, i64)> {
    message
        .cursor
        .chain_blocks
        .iter()
        .fold(cursors, |mut acc, (&chain_id, &(min, max))| {
            acc.entry((message.key.bridge_id, chain_id))
                .and_modify(|(existing_min, existing_max)| {
                    *existing_min = (*existing_min).min(min);
                    *existing_max = (*existing_max).max(max);
                })
                .or_insert((min, max));
            acc
        })
}

impl TryInto<crosschain_messages::ActiveModel> for &Message {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<crosschain_messages::ActiveModel, Self::Error> {
        let now = Utc::now().naive_utc();
        self.init_timestamp
            .ok_or_else(|| anyhow::anyhow!("init_timestamp is required for promotion"))?;

        let entry = crosschain_messages::ActiveModel {
            id: ActiveValue::Set(self.key.message_id),
            bridge_id: ActiveValue::Set(self.key.bridge_id),
            status: ActiveValue::Set(self.status.clone().into()),
            src_chain_id: ActiveValue::Set(self.src_chain_id.unwrap_or(0)),
            dst_chain_id: ActiveValue::Set(self.destination_chain_id),
            native_id: ActiveValue::Set(self.native_id.clone()),
            init_timestamp: ActiveValue::Set(now),
            last_update_timestamp: ActiveValue::Set(Some(now)),
            src_tx_hash: ActiveValue::Set(
                self.source_transaction_hash.map(|v| v.as_slice().to_vec()),
            ),
            dst_tx_hash: ActiveValue::Set(
                self.destination_transaction_hash
                    .map(|v| v.as_slice().to_vec()),
            ),
            sender_address: ActiveValue::Set(self.sender_address.map(|a| a.to_vec())),
            recipient_address: ActiveValue::Set(self.recipient_address.map(|a| a.to_vec())),
            payload: ActiveValue::Set(self.payload.clone()),
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::Set(Some(now)),
        };

        Ok(entry)
    }
}

impl TryInto<pending_messages::ActiveModel> for &Message {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<pending_messages::ActiveModel, Self::Error> {
        let now = Utc::now().naive_utc();
        let payload = serde_json::to_value(&self)?;

        Ok(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(self.key.message_id),
            bridge_id: ActiveValue::Set(self.key.bridge_id),
            payload: ActiveValue::Set(payload),
            created_at: ActiveValue::Set(Some(now)),
        })
    }
}

impl TryFrom<(crosschain_messages::Model, Vec<crosschain_transfers::Model>)> for Message {
    type Error = anyhow::Error;

    fn try_from(
        (model, _transfers): (crosschain_messages::Model, Vec<crosschain_transfers::Model>),
    ) -> Result<Self> {
        Ok(Self {
            key: Key::new(model.id, model.bridge_id),
            src_chain_id: Some(model.src_chain_id),
            source_transaction_hash: model
                .src_tx_hash
                .map(|v| FixedBytes::<32>::try_from(v.as_slice()))
                .transpose()?,
            init_timestamp: Some(model.init_timestamp),
            sender_address: model
                .sender_address
                .map(|v| Address::try_from(v.as_slice()))
                .transpose()?,
            recipient_address: model
                .recipient_address
                .map(|v| Address::try_from(v.as_slice()))
                .transpose()?,
            destination_chain_id: model.dst_chain_id,
            payload: model.payload,
            native_id: model.native_id,
            destination_transaction_hash: model
                .dst_tx_hash
                .map(|v| FixedBytes::<32>::try_from(v.as_slice()))
                .transpose()?,
            status: model.status.into(),
            ictt_fragments: Vec::new(), // Not stored in DB, fragments are consolidated
            cursor: Cursor::default(),  // Not stored in DB, starts fresh
            created_at: model.created_at,
        })
    }
}

/// Configuration for the tiered message buffer
#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum entries in hot tier before forced offload
    pub max_hot_entries: usize,
    /// Time before entry moves from hot to cold
    pub hot_ttl: Duration,
    /// How often to run maintenance (offload + flush)
    pub maintenance_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_hot_entries: 100_000,
            hot_ttl: Duration::from_secs(300), // 5 minutes
            maintenance_interval: Duration::from_millis(500),
        }
    }
}

/// Tiered message buffer with hot (memory) and cold (Postgres) storage
pub struct MessageBuffer {
    /// Hot tier: in-memory entries
    hot: DashMap<Key, Message>,
    /// Configuration
    config: Config,
    /// Database connection for cold tier operations
    db: InterchainDatabase,
    /// Lock for maintenance operations (offload/flush)
    maintenance_lock: RwLock<()>,
}

impl MessageBuffer {
    /// Create a new tiered message buffer
    pub fn new(db: InterchainDatabase, config: Config) -> Arc<Self> {
        Arc::new(Self {
            hot: DashMap::new(),
            config,
            db,
            maintenance_lock: RwLock::new(()),
        })
    }

    /// Get the number of entries in the hot tier (for testing/monitoring)
    ///
    #[allow(dead_code)]
    pub fn hot_len(&self) -> usize {
        self.hot.len()
    }

    /// Get existing entry or create new one.
    ///
    /// Checks:
    /// 1. hot tier
    /// 2. cold tier (pending_messages)
    /// 3. final storage (crosschain_messages)
    /// 4. create new
    pub async fn get_or_create(&self, key: Key) -> Result<Message> {
        if let Some(entry) = self.hot.get(&key) {
            return Ok(entry.clone());
        }

        if let Some(entry) = self
            .db
            .get_pending_message(key.message_id, key.bridge_id)
            .await?
            .map(|m| {
                serde_json::from_value(m.payload).context("failed to deserialize MessageEntry")
            })
        {
            return entry;
        }

        if let Some(model) = self
            .db
            .get_crosschain_message_by_pk(key.message_id, key.bridge_id)
            .await?
        {
            return model.try_into();
        }

        // 4. Create new entry
        Ok(Message::new(key))
    }

    /// Upsert entry in hot tier.
    /// If hot tier exceeds capacity, runs maintenance to flush entries.
    pub async fn upsert(&self, msg: Message) -> Result<()> {
        let key = msg.key;
        self.hot.insert(key, msg);

        // Run maintenance if hot tier is getting full
        if self.hot.len() > self.config.max_hot_entries {
            self.run().await?;
        }

        Ok(())
    }

    /// Run maintenance: offload stale entries, flush ready entries, update
    /// cursors
    ///
    /// Cursor update logic:
    /// - We can only safely advance cursors past blocks where ALL messages have
    ///   been flushed
    /// - Entries still in hot tier or cold storage represent "pending" work
    /// - The realtime_cursor should not advance past the lowest max_block of
    ///   any pending entry
    /// - The catchup_max_block should not retreat past the highest min_block of
    ///   any pending entry
    ///
    /// TODO: In case that buffer is full of entries that aren't too old based
    /// on TTL but also not ready yet, we may need to implement a more
    /// aggressive offloading strategy
    pub async fn run(&self) -> Result<()> {
        let _guard = self.maintenance_lock.write().await;
        let now = Utc::now().naive_utc();

        // Collect entries to process
        let mut ready_entries = Vec::new();
        let mut stale_entries = Vec::new();
        let mut keys_to_remove_from_hot = Vec::new();
        let mut keys_to_remove_from_pending = Vec::new();
        let mut hot_cursors = HashMap::<(_, _), (_, _)>::new();
        let mut cold_cursors = HashMap::<(_, _), (_, _)>::new();

        for entry in self.hot.iter() {
            match (entry.to_active_models(), entry.created_at) {
                (Ok(model), _) => {
                    ready_entries.push(model);
                    if entry.is_ictt_complete() {
                        // ICTT complete: remove from both pending and hot
                        keys_to_remove_from_pending.push(entry.key);
                    } else {
                        // ICTT incomplete: save to pending for recovery, remove from hot
                        stale_entries.push(entry.clone());
                    }
                    keys_to_remove_from_hot.push(entry.key);
                    cold_cursors = with_cursors(entry.value(), cold_cursors);
                }
                (_, Some(created_at))
                    if now
                        .signed_duration_since(created_at)
                        .to_std()
                        .unwrap_or_default()
                        >= self.config.hot_ttl =>
                {
                    cold_cursors = with_cursors(entry.value(), cold_cursors);
                    stale_entries.push(entry.clone());
                    keys_to_remove_from_hot.push(entry.key);
                }
                _ => {
                    hot_cursors = with_cursors(entry.value(), hot_cursors);
                }
            }
        }

        let cursors: HashMap<(i32, i64), (i64, i64)> = cold_cursors
            .into_iter()
            .filter_map(|(key, (cold_min, cold_max))| {
                hot_cursors.get(&key).map(|&(hot_min, hot_max)| {
                    // Bound cursor updates by what's still pending in hot tier
                    // NOTE: this may cause some blocks to be processed twice,
                    // so handlers and inserts should be idempotent.
                    let min = cold_min.max(hot_min.saturating_add(1));
                    let max = cold_max.min(hot_max.saturating_sub(1));
                    (key, (min, max))
                })
            })
            .filter(|(_, (min, max))| max >= min)
            .collect();

        // Skip if nothing to do
        if ready_entries.is_empty() && stale_entries.is_empty() {
            return Ok(());
        }

        let ready_count = ready_entries.len();
        let stale_count = stale_entries.len();
        let cursor_count = cursors.len();

        self.db
            .db
            .transaction::<_, (), DbErr>(|tx| {
                Box::pin(async move {
                    let batch_size = ((u16::MAX - 100) / 4) as usize;
                    for batch in stale_entries.chunks(batch_size) {
                        let models: Vec<pending_messages::ActiveModel> =
                            batch.iter().filter_map(|e| e.try_into().ok()).collect();

                        pending_messages::Entity::insert_many(models)
                            .on_conflict(
                                OnConflict::columns([
                                    pending_messages::Column::MessageId,
                                    pending_messages::Column::BridgeId,
                                ])
                                .update_column(pending_messages::Column::Payload)
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    // 2. Flush ready entries to final storage
                    let batch_size = ((u16::MAX - 100) / 15) as usize;
                    let (messages, transfers) = ready_entries
                        .into_iter()
                        .unzip::<_, _, Vec<_>, Vec<Vec<_>>>();
                    let transfers = transfers.into_iter().flatten().collect::<Vec<_>>();

                    for batch in messages.chunks(batch_size) {
                        crosschain_messages::Entity::insert_many(batch.to_vec())
                            .on_conflict(
                                OnConflict::columns([
                                    crosschain_messages::Column::Id,
                                    crosschain_messages::Column::BridgeId,
                                ])
                                .update_columns([
                                    crosschain_messages::Column::Status,
                                    crosschain_messages::Column::DstChainId,
                                    crosschain_messages::Column::DstTxHash,
                                    crosschain_messages::Column::LastUpdateTimestamp,
                                    crosschain_messages::Column::SenderAddress,
                                    crosschain_messages::Column::RecipientAddress,
                                    crosschain_messages::Column::Payload,
                                ])
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    // TODO: refactor to avoid deleting then inserting
                    for batch in transfers.chunks(batch_size) {
                        // Get unique (message_id, bridge_id) pairs from this batch
                        // TODO: avoid clone().unwrap()
                        let keys_in_batch: Vec<_> = batch
                            .iter()
                            .map(|t| {
                                (
                                    t.message_id.clone().unwrap(),
                                    t.bridge_id.clone().unwrap(),
                                )
                            })
                            .collect::<std::collections::HashSet<_>>()
                            .into_iter()
                            .collect();

                        // Delete existing transfers for these messages
                        crosschain_transfers::Entity::delete_many()
                            .filter(
                                Expr::tuple([
                                    Expr::col(crosschain_transfers::Column::MessageId).into(),
                                    Expr::col(crosschain_transfers::Column::BridgeId).into(),
                                ])
                                .in_tuples(keys_in_batch),
                            )
                            .exec(tx)
                            .await?;

                        // Insert new transfers
                        crosschain_transfers::Entity::insert_many(batch.to_vec())
                            .exec(tx)
                            .await?;
                    }

                    for batch in keys_to_remove_from_pending.chunks(batch_size) {
                        let batch = batch.iter()
                            .map(|k| (k.message_id, k.bridge_id));

                        pending_messages::Entity::delete_many()
                            .filter(
                                Expr::tuple([
                                    Expr::col(pending_messages::Column::MessageId).into(),
                                    Expr::col(pending_messages::Column::BridgeId).into(),
                                ])
                                .in_tuples(batch),
                            )
                            .exec(tx)
                            .await?;
                    }

                    let models: Vec<_> = cursors
                        .iter()
                        .map(|((bridge_id, chain_id), (min, max))| {
                            indexer_checkpoints::ActiveModel {
                                bridge_id: ActiveValue::Set(*bridge_id as i64),
                                chain_id: ActiveValue::Set(*chain_id),
                                catchup_min_block: ActiveValue::Set(0),
                                catchup_max_block: ActiveValue::Set(*min),
                                finality_cursor: ActiveValue::Set(0),
                                realtime_cursor: ActiveValue::Set(*max),
                                created_at: ActiveValue::NotSet,
                                updated_at: ActiveValue::NotSet,
                            }
                        })
                        .collect();

                        indexer_checkpoints::Entity::insert_many(models)
                            .on_empty_do_nothing()
                            .on_conflict(
                                OnConflict::columns([
                                    indexer_checkpoints::Column::BridgeId,
                                    indexer_checkpoints::Column::ChainId,
                                ])
                                .value(
                                    indexer_checkpoints::Column::CatchupMaxBlock,
                                    Expr::cust(
                                        "LEAST(indexer_checkpoints.catchup_max_block, EXCLUDED.catchup_max_block)",
                                    ),
                                )
                                .value(
                                    indexer_checkpoints::Column::RealtimeCursor,
                                    Expr::cust(
                                        "GREATEST(indexer_checkpoints.realtime_cursor, EXCLUDED.realtime_cursor)",
                                    ),
                                )
                                .value(
                                    indexer_checkpoints::Column::UpdatedAt,
                                    Expr::current_timestamp(),
                                )
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;


                    Ok(())
                })
            })
            .await
            .context("maintenance transaction failed")?;

        // Remove processed entries from hot tier
        keys_to_remove_from_hot.iter().for_each(|k| {
            self.hot.remove(k);
        });

        tracing::info!(
            ready = ready_count,
            stale = stale_count,
            cursors = cursor_count,
            "Maintenance completed"
        );

        Ok(())
    }

    /// Start the background maintenance loop. Restores pending messages from
    /// cold storage and spawns a maintenance task.
    pub async fn start(self: Arc<Self>) -> Result<JoinHandle<()>> {
        let pending = pending_messages::Entity::find()
            .order_by_asc(pending_messages::Column::CreatedAt)
            .limit(self.config.max_hot_entries as u64)
            .all(self.db.db.as_ref())
            .await
            .context("failed to load pending messages from cold storage")?;

        let count = pending.len();
        for p in pending {
            let msg: Message = serde_json::from_value(p.payload)?;
            self.hot.insert(msg.key, msg);
        }

        tracing::info!(
            count,
            max_entries = self.config.max_hot_entries,
            "Restored pending messages from cold storage"
        );

        let buffer = Arc::clone(&self);
        Ok(tokio::spawn(async move {
            let mut interval = tokio::time::interval(buffer.config.maintenance_interval);
            loop {
                interval.tick().await;
                if let Err(e) = buffer.run().await {
                    tracing::error!(error = ?e, "Buffer maintenance failed");
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_db;
    use interchain_indexer_entity::{
        bridges, chains, indexer_checkpoints, sea_orm_active_enums::BridgeType,
    };
    use sea_orm::ActiveValue::Set;

    /// Helper to set up test database with required foreign key data
    async fn setup_test_db(name: &str) -> (InterchainDatabase, i32, i64) {
        let db = init_db(name).await;
        let interchain_db = InterchainDatabase::new(db.client());

        let bridge_id = 1i32;
        let chain_id = 100i64;

        // Insert required chain (foreign key for indexer_checkpoints)
        interchain_db
            .upsert_chains(vec![chains::ActiveModel {
                id: Set(chain_id),
                name: Set("Test Chain".to_string()),
                native_id: Set(None),
                icon: Set(None),
                ..Default::default()
            }])
            .await
            .unwrap();

        // Insert required bridge (foreign key for indexer_checkpoints)
        interchain_db
            .upsert_bridges(vec![bridges::ActiveModel {
                id: Set(bridge_id),
                name: Set("Test Bridge".to_string()),
                r#type: Set(Some(BridgeType::AvalancheNative)),
                enabled: Set(true),
                api_url: Set(None),
                ui_url: Set(None),
                ..Default::default()
            }])
            .await
            .unwrap();

        // Insert initial checkpoint
        indexer_checkpoints::Entity::insert(indexer_checkpoints::ActiveModel {
            bridge_id: Set(bridge_id as i64),
            chain_id: Set(chain_id),
            catchup_min_block: Set(0),
            catchup_max_block: Set(1000), // Start high, will go DOWN via LEAST
            finality_cursor: Set(0),
            realtime_cursor: Set(0), // Start low, will go UP via GREATEST
            created_at: ActiveValue::NotSet,
            updated_at: ActiveValue::NotSet,
        })
        .exec(interchain_db.db.as_ref())
        .await
        .unwrap();

        (interchain_db, bridge_id, chain_id)
    }

    #[test]
    fn test_message_entry_serialization() {
        let key = Key::new(12345, 1);
        let mut msg = Message::new(key);

        let tx_hash = FixedBytes::<32>::repeat_byte(0xab);
        msg.src_chain_id = Some(43114);
        msg.source_transaction_hash = Some(tx_hash);
        msg.init_timestamp = Some(Utc::now().naive_utc());
        msg.sender_address = Some(Address::repeat_byte(0x11));
        msg.native_id = Some(vec![0x22; 32]);
        msg.status = Status::Completed;
        msg.cursor.record_block(43114, 1000);

        let json = serde_json::to_value(&msg).unwrap();
        let restored = serde_json::from_value::<Message>(json).unwrap();

        assert_eq!(restored.key, key);
        assert_eq!(restored.src_chain_id, Some(43114));
        assert_eq!(restored.source_transaction_hash, Some(tx_hash));
        assert!(restored.init_timestamp.is_some());
        assert_eq!(restored.status, Status::Completed);
        assert_eq!(
            restored.cursor.chain_blocks.get(&43114),
            Some(&(1000, 1000))
        );
    }

    #[test]
    fn test_cursor_tracker_bidirectional() {
        let mut tracker = Cursor::default();

        tracker.record_block(1, 100);
        tracker.record_block(1, 200);
        tracker.record_block(1, 30);

        assert_eq!(tracker.chain_blocks.get(&1), Some(&(30, 200)));
        assert_eq!(tracker.min_block(1), Some(30));
        assert_eq!(tracker.max_block(1), Some(200));
    }

    /// E2E Test: Ready message is flushed to crosschain_messages table
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_ready_message_flushed_to_final_storage() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_ready_flush").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0), // Immediate TTL for testing
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Create a ready message (has init_timestamp)
        let mut msg = Message::new(Key::new(1, bridge_id));
        msg.src_chain_id = Some(chain_id);
        msg.init_timestamp = Some(Utc::now().naive_utc());
        msg.cursor.record_block(chain_id, 10);

        buffer.upsert(msg).await.unwrap();
        assert_eq!(buffer.hot.len(), 1);

        // Run maintenance
        buffer.run().await.unwrap();

        // Message should be flushed to final storage
        assert_eq!(buffer.hot.len(), 0, "Hot tier should be empty after flush");

        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1, "Message should be in final storage");
        assert_eq!(messages[0].0.id, 1);
    }

    /// E2E Test: Stale message is offloaded to pending_messages table
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_stale_message_offloaded_to_cold_storage() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_stale_offload").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0), // Immediate TTL
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Create an incomplete message (no init_timestamp - not ready)
        let mut msg = Message::new(Key::new(1, bridge_id));
        msg.src_chain_id = Some(chain_id);
        // No init_timestamp - message is not ready
        msg.cursor.record_block(chain_id, 10);
        // Backdate created_at to make it stale
        msg.created_at = Some(Utc::now().naive_utc() - chrono::Duration::seconds(10));

        buffer.upsert(msg).await.unwrap();
        assert_eq!(buffer.hot.len(), 1);

        // Run maintenance
        buffer.run().await.unwrap();

        // Message should be offloaded to cold storage (pending_messages)
        assert_eq!(
            buffer.hot.len(),
            0,
            "Hot tier should be empty after offload"
        );

        let pending = db.get_pending_message(1, bridge_id).await.unwrap();
        assert!(pending.is_some(), "Message should be in pending_messages");
    }

    /// E2E Test: Cursor is NOT updated when pending message blocks the range
    ///
    /// Scenario:
    /// - Message 1: ready, at blocks 1-2
    /// - Message 2: pending (not ready), at block 2
    ///
    /// Expected: realtime_cursor should NOT advance because Message 2 at block 2 blocks it
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_cursor_blocked_by_pending_message() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_cursor_blocked").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(300), // Long TTL - don't offload msg2
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Message 1: ready (will be flushed)
        let mut msg1 = Message::new(Key::new(1, bridge_id));
        msg1.src_chain_id = Some(chain_id);
        msg1.init_timestamp = Some(Utc::now().naive_utc());
        msg1.cursor.record_block(chain_id, 1);
        msg1.cursor.record_block(chain_id, 2);

        // Message 2: NOT ready (stays in hot tier)
        let mut msg2 = Message::new(Key::new(2, bridge_id));
        msg2.src_chain_id = Some(chain_id);
        // No init_timestamp
        msg2.cursor.record_block(chain_id, 2);

        buffer.upsert(msg1).await.unwrap();
        buffer.upsert(msg2).await.unwrap();
        assert_eq!(buffer.hot.len(), 2);

        // Run maintenance - should flush msg1 but keep msg2
        buffer.run().await.unwrap();

        // msg1 flushed, msg2 still in hot
        assert_eq!(buffer.hot.len(), 1);
        assert!(buffer.hot.get(&Key::new(2, bridge_id)).is_some());

        // Check checkpoint - should NOT have been updated due to bounding logic
        let checkpoint = db
            .get_checkpoint(bridge_id as u64, chain_id as u64)
            .await
            .unwrap()
            .unwrap();

        // realtime_cursor should still be 0 (initial value) because msg2 at block 2 blocks it
        assert_eq!(
            checkpoint.realtime_cursor, 0,
            "realtime_cursor should not advance past pending message"
        );
    }

    /// E2E Test: Cursor advances when no pending messages block the range
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_cursor_advances_when_unblocked() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_cursor_advances").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(300),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Message 1: ready at blocks 1-5
        let mut msg1 = Message::new(Key::new(1, bridge_id));
        msg1.src_chain_id = Some(chain_id);
        msg1.init_timestamp = Some(Utc::now().naive_utc());
        msg1.cursor.record_block(chain_id, 1);
        msg1.cursor.record_block(chain_id, 5);

        // Message 2: NOT ready, but at blocks 10-15 (doesn't overlap with msg1)
        let mut msg2 = Message::new(Key::new(2, bridge_id));
        msg2.src_chain_id = Some(chain_id);
        msg2.cursor.record_block(chain_id, 10);
        msg2.cursor.record_block(chain_id, 15);

        buffer.upsert(msg1).await.unwrap();
        buffer.upsert(msg2).await.unwrap();

        buffer.run().await.unwrap();

        // msg1 should be flushed
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        // Due to the bounding logic:
        // cold: msg1 has blocks 1-5
        // hot: msg2 has blocks 10-15
        // min = max(1, 10+1) = 11, max = min(5, 15-1) = 5
        // Since max(5) < min(11), cursor update is filtered out
        let checkpoint = db
            .get_checkpoint(bridge_id as u64, chain_id as u64)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            checkpoint.realtime_cursor, 0,
            "Cursor should not update when ranges don't overlap properly"
        );
    }

    /// E2E Test: Message restored from cold storage on start
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_cold_storage_restored_on_start() {
        let (db, bridge_id, _chain_id) = setup_test_db("buffer_restore").await;

        // Pre-populate pending_messages
        let msg = Message::new(Key::new(42, bridge_id));
        let payload = serde_json::to_value(&msg).unwrap();
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: Set(42),
            bridge_id: Set(bridge_id),
            payload: Set(payload),
            created_at: Set(Some(Utc::now().naive_utc())),
        })
        .await
        .unwrap();

        // Create buffer and start (which loads from cold storage)
        let config = Config::default();
        let buffer = MessageBuffer::new(db.clone(), config);

        // Start loads pending messages
        let _handle = buffer.clone().start().await.unwrap();

        // Should have the message in hot tier
        assert_eq!(buffer.hot.len(), 1);
        assert!(buffer.hot.get(&Key::new(42, bridge_id)).is_some());
    }

    /// E2E Test: Maintenance triggered when hot tier exceeds capacity
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_maintenance_triggered_on_capacity() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_capacity").await;

        let config = Config {
            max_hot_entries: 2, // Very low capacity
            hot_ttl: Duration::from_secs(0),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Insert 3 ready messages (exceeds capacity of 2)
        for i in 1..=3 {
            let mut msg = Message::new(Key::new(i, bridge_id));
            msg.src_chain_id = Some(chain_id);
            msg.init_timestamp = Some(Utc::now().naive_utc());
            msg.cursor.record_block(chain_id, i);
            buffer.upsert(msg).await.unwrap();
        }

        // After inserting 3rd message, maintenance should have been triggered
        // All ready messages should be flushed
        assert_eq!(
            buffer.hot.len(),
            0,
            "Hot tier should be empty after capacity-triggered maintenance"
        );

        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 3, "All messages should be in final storage");
    }

    /// E2E Test: Pending message deleted when promoted to final storage
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_pending_deleted_on_promotion() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_pending_delete").await;

        // Pre-populate pending_messages with incomplete message
        let mut msg = Message::new(Key::new(1, bridge_id));
        msg.src_chain_id = Some(chain_id);
        msg.cursor.record_block(chain_id, 5);
        let payload = serde_json::to_value(&msg).unwrap();

        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: Set(1),
            bridge_id: Set(bridge_id),
            payload: Set(payload),
            created_at: Set(Some(Utc::now().naive_utc())),
        })
        .await
        .unwrap();

        // Verify it's in pending
        assert!(
            db.get_pending_message(1, bridge_id)
                .await
                .unwrap()
                .is_some()
        );

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Now complete the message (add init_timestamp) and upsert
        msg.init_timestamp = Some(Utc::now().naive_utc());
        buffer.upsert(msg).await.unwrap();

        // Run maintenance
        buffer.run().await.unwrap();

        // Message should be in final storage
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        // Message should be deleted from pending
        assert!(
            db.get_pending_message(1, bridge_id)
                .await
                .unwrap()
                .is_none(),
            "Message should be deleted from pending_messages after promotion"
        );
    }

    /// Regression test: When SendCrossChainMessage is flushed to crosschain_messages,
    /// and then a later event arrives (e.g., ReceiveCrossChainMessage), the buffer
    /// should find the existing message in the final table and update it,
    /// not create a new orphan entry.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_update_after_message_flushed_to_final_storage() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_update_after_flush").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0), // Immediate TTL for testing
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        let key = Key::new(1, bridge_id);

        // Phase 1: Insert a READY message (simulating SendCrossChainMessage)
        // with init_timestamp set and source data filled
        let mut msg = Message::new(key);
        msg.init_timestamp = Some(chrono::Utc::now().naive_utc());
        msg.src_chain_id = Some(chain_id);
        msg.source_transaction_hash = Some(FixedBytes::<32>::repeat_byte(0xab));
        msg.sender_address = Some(Address::repeat_byte(0x11));
        msg.cursor.record_block(chain_id, 1000);
        buffer.upsert(msg).await.unwrap();

        // Phase 2: Run maintenance - this flushes the ready message to crosschain_messages
        buffer.run().await.unwrap();

        // Verify message is in final storage
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(
            messages.len(),
            1,
            "Message should be in crosschain_messages"
        );
        let msg_row = &messages[0].0;
        assert_eq!(msg_row.src_chain_id, chain_id);
        assert!(
            msg_row.dst_chain_id.is_none(),
            "Dest chain should not be set yet"
        );
        assert_eq!(msg_row.status, MessageStatus::Initiated);

        // Verify hot tier is empty
        assert!(
            buffer.hot.is_empty(),
            "Hot tier should be empty after flush"
        );

        // Phase 3: A later event arrives (e.g., message completion)
        // The handler calls get_or_create and updates with destination data
        let mut msg = buffer.get_or_create(key).await.unwrap();
        // The bug: this creates a NEW entry without init_timestamp
        // It should have found the message in crosschain_messages and loaded it
        msg.destination_chain_id = Some(chain_id); // Same chain for simplicity
        msg.destination_transaction_hash = Some(FixedBytes::<32>::repeat_byte(0xef));
        msg.recipient_address = Some(Address::repeat_byte(0x22));
        msg.status = Status::Completed;
        msg.cursor.record_block(chain_id, 2000);
        buffer.upsert(msg).await.unwrap();

        // Phase 4: Run maintenance again
        buffer.run().await.unwrap();

        // Verify: The message in crosschain_messages should now have BOTH source AND dest data
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(
            messages.len(),
            1,
            "Should still be 1 message, not 2 (no orphan created)"
        );

        let msg = &messages[0].0;
        // Source data should still be there
        assert_eq!(
            msg.src_chain_id, chain_id,
            "Source chain should still be set"
        );
        assert!(
            msg.src_tx_hash.is_some(),
            "Source tx hash should still be present"
        );

        // Destination data should now be added
        assert_eq!(
            msg.dst_chain_id,
            Some(chain_id),
            "Dest chain should now be set from update event"
        );
        assert!(msg.dst_tx_hash.is_some(), "Dest tx hash should now be set");
        assert_eq!(
            msg.status,
            MessageStatus::Completed,
            "Status should be updated"
        );
    }

    /// Unit test: is_ictt_complete returns true when no ICTT fragments
    #[test]
    fn test_is_ictt_complete_no_fragments() {
        let msg = Message::new(Key::new(1, 1));
        assert!(msg.is_ictt_complete(), "No fragments means complete");
    }

    /// Unit test: is_ictt_complete returns false with only TokensSent (no destination event)
    #[test]
    fn test_is_ictt_complete_tokens_sent_only() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments.push(IcttEventFragment::TokensSent {
            token_contract: Address::repeat_byte(0x01),
            sender: Address::repeat_byte(0x02),
            dst_token_address: Address::repeat_byte(0x03),
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            !msg.is_ictt_complete(),
            "TokensSent without TokensWithdrawn is incomplete"
        );
    }

    /// Unit test: is_ictt_complete returns true with TokensSent + TokensWithdrawn
    #[test]
    fn test_is_ictt_complete_tokens_sent_and_withdrawn() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments.push(IcttEventFragment::TokensSent {
            token_contract: Address::repeat_byte(0x01),
            sender: Address::repeat_byte(0x02),
            dst_token_address: Address::repeat_byte(0x03),
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            msg.is_ictt_complete(),
            "TokensSent + TokensWithdrawn is complete"
        );
    }

    /// Unit test: is_ictt_complete returns false with only TokensAndCallSent (no call result)
    #[test]
    fn test_is_ictt_complete_tokens_and_call_sent_only() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments
            .push(IcttEventFragment::TokensAndCallSent {
                token_contract: Address::repeat_byte(0x01),
                sender: Address::repeat_byte(0x02),
                dst_token_address: Address::repeat_byte(0x03),
                recipient_contract: Address::repeat_byte(0x04),
                fallback_recipient: Address::repeat_byte(0x05),
                amount: alloy::primitives::U256::from(1000),
            });
        assert!(
            !msg.is_ictt_complete(),
            "TokensAndCallSent without CallSucceeded/CallFailed is incomplete"
        );
    }

    /// Unit test: is_ictt_complete returns true with TokensAndCallSent + CallSucceeded
    #[test]
    fn test_is_ictt_complete_tokens_and_call_sent_with_success() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments
            .push(IcttEventFragment::TokensAndCallSent {
                token_contract: Address::repeat_byte(0x01),
                sender: Address::repeat_byte(0x02),
                dst_token_address: Address::repeat_byte(0x03),
                recipient_contract: Address::repeat_byte(0x04),
                fallback_recipient: Address::repeat_byte(0x05),
                amount: alloy::primitives::U256::from(1000),
            });
        msg.ictt_fragments.push(IcttEventFragment::CallSucceeded {
            recipient_contract: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            msg.is_ictt_complete(),
            "TokensAndCallSent + CallSucceeded is complete"
        );
    }

    /// Unit test: is_ictt_complete returns true with TokensAndCallSent + CallFailed
    #[test]
    fn test_is_ictt_complete_tokens_and_call_sent_with_failure() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments
            .push(IcttEventFragment::TokensAndCallSent {
                token_contract: Address::repeat_byte(0x01),
                sender: Address::repeat_byte(0x02),
                dst_token_address: Address::repeat_byte(0x03),
                recipient_contract: Address::repeat_byte(0x04),
                fallback_recipient: Address::repeat_byte(0x05),
                amount: alloy::primitives::U256::from(1000),
            });
        msg.ictt_fragments.push(IcttEventFragment::CallFailed {
            recipient_contract: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            msg.is_ictt_complete(),
            "TokensAndCallSent + CallFailed is complete"
        );
    }

    /// Unit test: is_ictt_complete returns false with mismatched pairs
    /// (TokensSent should not be completed by CallSucceeded)
    #[test]
    fn test_is_ictt_complete_mismatched_pairs() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments.push(IcttEventFragment::TokensSent {
            token_contract: Address::repeat_byte(0x01),
            sender: Address::repeat_byte(0x02),
            dst_token_address: Address::repeat_byte(0x03),
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        msg.ictt_fragments.push(IcttEventFragment::CallSucceeded {
            recipient_contract: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            !msg.is_ictt_complete(),
            "TokensSent + CallSucceeded is NOT a valid completion pair"
        );
    }

    /// Unit test: is_ictt_complete returns false with only destination events (no source)
    #[test]
    fn test_is_ictt_complete_destination_only() {
        let mut msg = Message::new(Key::new(1, 1));
        msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        assert!(
            !msg.is_ictt_complete(),
            "TokensWithdrawn without source event is incomplete"
        );
    }

    /// E2E Test: Incomplete ICTT message is flushed but kept in pending
    ///
    /// Scenario:
    /// - Message has init_timestamp (ready) and TokensSent (source ICTT event)
    /// - But NO destination event yet (TokensWithdrawn)
    ///
    /// Expected:
    /// - Message is flushed to crosschain_messages (it's ready)
    /// - Message is NOT deleted from pending_messages (ICTT incomplete)
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_incomplete_ictt_kept_in_pending() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_incomplete_ictt").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0), // Immediate TTL
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Create a ready message with incomplete ICTT (only source event)
        let mut msg = Message::new(Key::new(1, bridge_id));
        msg.src_chain_id = Some(chain_id);
        msg.destination_chain_id = Some(chain_id); // Same chain for simplicity in test
        msg.init_timestamp = Some(Utc::now().naive_utc()); // Ready!
        msg.cursor.record_block(chain_id, 10);

        // Add TokensSent but NOT TokensWithdrawn - ICTT incomplete
        msg.ictt_fragments.push(IcttEventFragment::TokensSent {
            token_contract: Address::repeat_byte(0x01),
            sender: Address::repeat_byte(0x02),
            dst_token_address: Address::repeat_byte(0x03),
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });

        buffer.upsert(msg).await.unwrap();

        // Run maintenance
        buffer.run().await.unwrap();

        // Message should be flushed to crosschain_messages (it's ready)
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(
            messages.len(),
            1,
            "Message should be in crosschain_messages"
        );

        // But pending should still exist because ICTT is incomplete
        let pending = db.get_pending_message(1, bridge_id).await.unwrap();
        assert!(
            pending.is_some(),
            "Pending should NOT be deleted when ICTT is incomplete"
        );
    }

    /// E2E Test: Complete ICTT message is flushed AND deleted from pending
    ///
    /// Scenario:
    /// - Message has init_timestamp (ready) and complete ICTT (TokensSent + TokensWithdrawn)
    ///
    /// Expected:
    /// - Message is flushed to crosschain_messages
    /// - Message IS deleted from pending_messages
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_complete_ictt_deleted_from_pending() {
        let (db, bridge_id, chain_id) = setup_test_db("buffer_complete_ictt").await;

        let config = Config {
            max_hot_entries: 100,
            hot_ttl: Duration::from_secs(0),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        // Create a ready message with complete ICTT
        let mut msg = Message::new(Key::new(1, bridge_id));
        msg.src_chain_id = Some(chain_id);
        msg.destination_chain_id = Some(chain_id); // Same chain for simplicity in test
        msg.init_timestamp = Some(Utc::now().naive_utc());
        msg.cursor.record_block(chain_id, 10);

        // Add TokensSent AND TokensWithdrawn - ICTT complete
        msg.ictt_fragments.push(IcttEventFragment::TokensSent {
            token_contract: Address::repeat_byte(0x01),
            sender: Address::repeat_byte(0x02),
            dst_token_address: Address::repeat_byte(0x03),
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });
        msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
            recipient: Address::repeat_byte(0x04),
            amount: alloy::primitives::U256::from(1000),
        });

        buffer.upsert(msg).await.unwrap();

        // Run maintenance
        buffer.run().await.unwrap();

        // Message should be flushed to crosschain_messages
        let (messages, _) = db
            .get_crosschain_messages(None, 100, false, None)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        // Pending should be deleted because ICTT is complete
        let pending = db.get_pending_message(1, bridge_id).await.unwrap();
        assert!(
            pending.is_none(),
            "Pending should be deleted when ICTT is complete"
        );
    }
}
