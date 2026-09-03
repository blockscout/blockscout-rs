// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    BridgeConfig,
    proto::{interchain_service_server::*, *},
    settings::ApiSettings,
};
use anyhow::{Context, anyhow};
use interchain_indexer_entity::{
    crosschain_messages::Model as CrosschainMessageModel,
    crosschain_transfers::Model as CrosschainTransferModel,
    sea_orm_active_enums::MessageStatus as DbMessageStatus, tokens::Model as TokenInfoModel,
};
use interchain_indexer_logic::{
    ChainInfoService, CrosschainMessageLookup, IndexedChains, InterchainDatabase, JoinedTransfer,
    TokenInfoService,
    pagination::{
        ListMarker, MessagesPaginationLogic, PaginationDirection, TransfersPaginationLogic,
    },
    utils::{hex_string_opt, to_hex_prefixed, vec_from_hex_prefixed},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tonic::{Request, Response, Status};

use super::{
    bridge_proto::bridge_model_to_proto,
    chain_info_proto::chain_model_to_proto,
    utils::{
        build_chain_bridge_filter, checked_bridge_id, db_datetime_to_string, map_db_error,
        parse_bridge_ids_csv, parse_chain_ids_csv,
    },
};

macro_rules! messages_pagination_params {
    ($service:expr, $request:expr) => {{
        let inner = $request.into_inner();
        let api_settings = &$service.api_settings;

        let input_pagination = if api_settings.use_pagination_token {
            if let Some(pagination_token) = &inner.page_token {
                Some(MessagesPaginationLogic::from_token(pagination_token).map_err(map_db_error)?)
            } else {
                None
            }
        } else {
            match (
                inner.timestamp,
                inner.message_id.clone(),
                inner.bridge_id,
                inner.direction.clone(),
            ) {
                (Some(timestamp), Some(message_id), Some(bridge_id), Some(direction)) => Some(
                    MessagesPaginationLogic::new(
                        timestamp as i64,
                        message_id,
                        bridge_id,
                        PaginationDirection::from_string(&direction).map_err(map_db_error)?,
                    )
                    .map_err(map_db_error)?,
                ),

                (None, None, None, None) => None,

                _ => {
                    return Err(map_db_error(anyhow!(
                        "Pagination error: timestamp, message_id, \
                         bridge_id and direction must be provided together"
                    )));
                }
            }
        };

        let page_size = inner
            .page_size
            .unwrap_or(api_settings.default_page_size)
            .clamp(1, api_settings.max_page_size) as usize;

        let is_last_page = inner.last_page.unwrap_or(false);

        Ok::<_, tonic::Status>((inner, input_pagination, page_size, is_last_page))
    }};
}

macro_rules! transfers_pagination_params {
    ($service:expr, $request:expr) => {{
        let inner = $request.into_inner();
        let api_settings = &$service.api_settings;

        let input_pagination = if api_settings.use_pagination_token {
            if let Some(pagination_token) = &inner.page_token {
                Some(TransfersPaginationLogic::from_token(pagination_token).map_err(map_db_error)?)
            } else {
                None
            }
        } else {
            match (
                inner.timestamp,
                inner.message_id.clone(),
                inner.bridge_id,
                inner.index,
                inner.direction.clone(),
            ) {
                (
                    Some(timestamp),
                    Some(message_id),
                    Some(bridge_id),
                    Some(index),
                    Some(direction),
                ) => Some(
                    TransfersPaginationLogic::new(
                        timestamp as i64,
                        message_id,
                        bridge_id,
                        index,
                        PaginationDirection::from_string(&direction).map_err(map_db_error)?,
                    )
                    .map_err(map_db_error)?,
                ),

                (None, None, None, None, None) => None,

                _ => {
                    return Err(map_db_error(anyhow!(
                        "Pagination error: timestamp, message_id, \
                         bridge_id, index and direction must be provided \
                         together"
                    )));
                }
            }
        };

        let page_size = inner
            .page_size
            .unwrap_or(api_settings.default_page_size)
            .clamp(1, api_settings.max_page_size) as usize;

        let is_last_page = inner.last_page.unwrap_or(false);

        Ok::<_, tonic::Status>((inner, input_pagination, page_size, is_last_page))
    }};
}

pub struct InterchainServiceImpl {
    pub db: Arc<InterchainDatabase>,
    pub token_info_service: Arc<TokenInfoService>,
    pub chain_info_service: Arc<ChainInfoService>,
    pub bridges_map: HashMap<i32, BridgeInfo>,
    pub api_settings: ApiSettings,
    pub indexed_chains: Arc<IndexedChains>,
}

impl InterchainServiceImpl {
    pub fn new(
        db: Arc<InterchainDatabase>,
        token_info_service: Arc<TokenInfoService>,
        chain_info_service: Arc<ChainInfoService>,
        bridges: Vec<BridgeConfig>,
        api_settings: ApiSettings,
        indexed_chains: Arc<IndexedChains>,
    ) -> anyhow::Result<Self> {
        let bridges_map = bridges
            .into_iter()
            .map(|b| {
                let id = u32::try_from(b.bridge_id)
                    .with_context(|| format!("bridge id {} out of u32 range", b.bridge_id))?;
                Ok((
                    b.bridge_id,
                    BridgeInfo {
                        name: b.name,
                        ui_url: b.ui_url,
                        docs_url: b.docs_url,
                        id,
                    },
                ))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Ok(Self {
            db,
            token_info_service,
            chain_info_service,
            bridges_map,
            api_settings,
            indexed_chains,
        })
    }

    async fn message_model_to_proto(
        &self,
        message: CrosschainMessageModel,
        transfers: Vec<CrosschainTransferModel>,
    ) -> Result<InterchainMessage, Status> {
        let payload = message
            .payload
            .as_ref()
            .map(|payload| to_hex_prefixed(payload.as_slice()));

        let transfers = futures::future::try_join_all(transfers.into_iter().map(|transfer| {
            let message = message.clone();
            async move { self.transfer_logic_to_proto(&transfer, &message).await }
        }))
        .await?;

        let source_chain = self.get_chain_info(message.src_chain_id).await.into();
        let destination_chain = match message.dst_chain_id {
            Some(id) => Some(self.get_chain_info(id).await),
            None => None,
        };

        let has_unindexed_chain = self.indexed_chains.message_has_unindexed(
            message.bridge_id,
            message.src_chain_id,
            message.dst_chain_id,
        );

        Ok(InterchainMessage {
            bridge: self.get_bridge_info(message.bridge_id)?.into(),
            message_id: self.get_message_id_from_message(&message),
            status: message_status_to_proto(&message.status) as i32,
            source_chain,
            sender: self.get_address_info_opt(message.sender_address),
            send_timestamp: db_datetime_to_string(message.init_timestamp),
            source_transaction_hash: hex_string_opt(message.src_tx_hash),
            destination_chain,
            recipient: self.get_address_info_opt(message.recipient_address),
            receive_timestamp: message.last_update_timestamp.map(db_datetime_to_string),
            destination_transaction_hash: hex_string_opt(message.dst_tx_hash),
            payload,
            extra: BTreeMap::new(),
            transfers,
            has_unindexed_chain,
        })
    }

    async fn messages_logic_to_proto(
        &self,
        messages: Vec<(CrosschainMessageModel, Vec<CrosschainTransferModel>)>,
    ) -> Result<Vec<InterchainMessage>, Status> {
        let futures = messages.into_iter().map(|(message, transfers)| async move {
            self.message_model_to_proto(message, transfers).await
        });

        futures::future::try_join_all(futures).await
    }

    async fn joined_transfers_logic_to_proto(
        &self,
        transfers: Vec<JoinedTransfer>,
    ) -> Result<Vec<InterchainTransfer>, Status> {
        let futures = transfers
            .into_iter()
            .map(|t| async move { self.joined_transfer_logic_to_proto(&t).await });

        futures::future::try_join_all(futures).await
    }

    async fn transfer_logic_to_proto(
        &self,
        transfer: &CrosschainTransferModel,
        message: &CrosschainMessageModel,
    ) -> Result<InterchainTransfer, Status> {
        let has_unindexed_chain = self.indexed_chains.transfer_has_unindexed(
            message.bridge_id,
            transfer.token_src_chain_id,
            transfer.token_dst_chain_id,
        );

        Ok(InterchainTransfer {
            bridge: self.get_bridge_info(message.bridge_id)?.into(),
            message_id: self.get_message_id_from_message(message),
            status: message_status_to_proto(&message.status) as i32,
            source_chain: Some(self.get_chain_info(transfer.token_src_chain_id).await),
            destination_chain: Some(self.get_chain_info(transfer.token_dst_chain_id).await),
            source_token: self
                .get_token_info_opt(
                    transfer.token_src_chain_id,
                    transfer.token_src_address.clone(),
                )
                .await,
            source_amount: transfer.src_amount.as_ref().map(|a| a.to_plain_string()),
            source_transaction_hash: hex_string_opt(message.src_tx_hash.clone()),
            sender: self.get_address_info_opt(transfer.sender_address.clone()),
            send_timestamp: db_datetime_to_string(message.init_timestamp),
            destination_token: self
                .get_token_info_opt(
                    transfer.token_dst_chain_id,
                    transfer.token_dst_address.clone(),
                )
                .await,
            destination_amount: transfer.dst_amount.as_ref().map(|a| a.to_plain_string()),
            destination_transaction_hash: hex_string_opt(message.dst_tx_hash.clone()),
            recipient: self.get_address_info_opt(transfer.recipient_address.clone()),
            receive_timestamp: message.last_update_timestamp.map(db_datetime_to_string),
            has_unindexed_chain,
        })
    }

    async fn joined_transfer_logic_to_proto(
        &self,
        transfer: &JoinedTransfer,
    ) -> Result<InterchainTransfer, Status> {
        let has_unindexed_chain = self.indexed_chains.transfer_has_unindexed(
            transfer.bridge_id,
            transfer.token_src_chain_id,
            transfer.token_dst_chain_id,
        );

        Ok(InterchainTransfer {
            bridge: self.get_bridge_info(transfer.bridge_id)?.into(),
            message_id: self.get_message_id_from_joined_transfer(transfer),
            status: message_status_to_proto(&transfer.status) as i32,
            source_chain: Some(self.get_chain_info(transfer.token_src_chain_id).await),
            destination_chain: Some(self.get_chain_info(transfer.token_dst_chain_id).await),
            source_token: self
                .get_token_info_opt(
                    transfer.token_src_chain_id,
                    transfer.token_src_address.clone(),
                )
                .await,
            source_amount: transfer.src_amount.as_ref().map(|a| a.to_plain_string()),
            source_transaction_hash: hex_string_opt(transfer.src_tx_hash.clone()),
            sender: self.get_address_info_opt(transfer.sender_address.clone()),
            send_timestamp: db_datetime_to_string(transfer.init_timestamp),
            destination_token: self
                .get_token_info_opt(
                    transfer.token_dst_chain_id,
                    transfer.token_dst_address.clone(),
                )
                .await,
            destination_amount: transfer.dst_amount.as_ref().map(|a| a.to_plain_string()),
            destination_transaction_hash: hex_string_opt(transfer.dst_tx_hash.clone()),
            recipient: self.get_address_info_opt(transfer.recipient_address.clone()),
            receive_timestamp: transfer.last_update_timestamp.map(db_datetime_to_string),
            has_unindexed_chain,
        })
    }

    fn get_bridge_info(&self, bridge_id: i32) -> Result<BridgeInfo, Status> {
        match self.bridges_map.get(&bridge_id).cloned() {
            Some(info) => Ok(info),
            None => {
                let id = u32::try_from(bridge_id)
                    .map_err(|_| map_db_error(anyhow!("bridge id out of range")))?;
                Ok(BridgeInfo {
                    name: "Unknown".to_string(),
                    ui_url: None,
                    docs_url: None,
                    id,
                })
            }
        }
    }

    fn get_message_id_from_message(&self, message: &CrosschainMessageModel) -> String {
        message
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| format!("0x{:x}", message.id))
    }

    fn get_message_id_from_joined_transfer(&self, joined: &JoinedTransfer) -> String {
        joined
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| format!("0x{:x}", joined.message_id))
    }

    /// Token info for an optional token address: `None` when the transfer side's
    /// token is unknown (the corresponding bridge event has not been observed).
    async fn get_token_info_opt(
        &self,
        chain_id: i64,
        address: Option<Vec<u8>>,
    ) -> Option<TokenInfo> {
        match address {
            Some(address) => self.get_token_info(chain_id, address).await,
            None => None,
        }
    }

    async fn get_token_info(&self, chain_id: i64, address: Vec<u8>) -> Option<TokenInfo> {
        let address_hex = to_hex_prefixed(address.as_slice());
        self.token_info_service
            .clone()
            .get_token_info(chain_id, address)
            .await
            .inspect_err(|e| tracing::error!(err = ?e, chain_id, address =? address_hex, "Failed to get token info"))
            .ok()
            .map(token_info_logic_to_proto)
            .unwrap_or_else(|| {
                // void TokenInfo (at least store address and chain id)
                TokenInfo {
                    address_hash: address_hex.clone(),
                    name: None,
                    symbol: None,
                    decimals: None,
                    icon_url: None,
                }
            })
            .into()
    }

    fn get_address_info_opt(&self, address: Option<Vec<u8>>) -> Option<AddressInfo> {
        address.map(|a| AddressInfo {
            hash: to_hex_prefixed(a.as_slice()),
            ens_domain_name: None,
        })
    }

    async fn get_chain_info(&self, chain_id: i64) -> ChainInfo {
        chain_model_to_proto(self.chain_info_service.get_chain_info(chain_id).await)
    }
}

#[async_trait::async_trait]
impl InterchainService for InterchainServiceImpl {
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            messages_pagination_params!(self, request)?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(
                None,
                None,
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items).await?;

        let response = GetMessagesResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_message_details(
        &self,
        request: Request<GetMessageDetailsRequest>,
    ) -> Result<Response<InterchainMessage>, Status> {
        let inner = request.into_inner();
        let message_id = vec_from_hex_prefixed(&inner.message_id).map_err(|_| {
            Status::invalid_argument("invalid message_id: expected a 0x-prefixed hex string")
        })?;
        let bridge_id = checked_bridge_id(inner.bridge_id)?;

        let response = match self
            .db
            .get_crosschain_message(message_id, bridge_id)
            .await
            .map_err(map_db_error)?
        {
            CrosschainMessageLookup::Found(message, transfers) => {
                self.message_model_to_proto(message, transfers).await?
            }
            CrosschainMessageLookup::NotFound => {
                return Err(Status::not_found("Message not found"));
            }
            CrosschainMessageLookup::Ambiguous => {
                return Err(Status::failed_precondition(
                    "Message ID matches multiple bridges; provide bridge_id",
                ));
            }
        };

        Ok(Response::new(response))
    }

    async fn get_messages_by_transaction(
        &self,
        request: Request<GetMessagesByTransactionRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            messages_pagination_params!(self, request)?;

        let tx_hash = vec_from_hex_prefixed(&inner.tx_hash).map_err(|_| {
            Status::invalid_argument("invalid tx_hash: expected a 0x-prefixed hex string")
        })?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(
                Some(tx_hash),
                None,
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items).await?;

        let response = GetMessagesResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_messages_by_address(
        &self,
        request: Request<GetMessagesByAddressRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            messages_pagination_params!(self, request)?;

        let address = vec_from_hex_prefixed(&inner.address).map_err(|_| {
            Status::invalid_argument("invalid address: expected a 0x-prefixed hex string")
        })?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(
                None,
                Some(address),
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items).await?;

        let response = GetMessagesResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_transfers(
        &self,
        request: Request<GetTransfersRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            transfers_pagination_params!(self, request)?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_transfers(
                None,
                None,
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.joined_transfers_logic_to_proto(items).await?;

        let response = GetTransfersResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_transfers_by_transaction(
        &self,
        request: Request<GetTransfersByTransactionRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            transfers_pagination_params!(self, request)?;

        let tx_hash = vec_from_hex_prefixed(&inner.tx_hash).map_err(|_| {
            Status::invalid_argument("invalid tx_hash: expected a 0x-prefixed hex string")
        })?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_transfers(
                Some(tx_hash),
                None,
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.joined_transfers_logic_to_proto(items).await?;

        let response = GetTransfersResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_transfers_by_address(
        &self,
        request: Request<GetTransfersByAddressRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            transfers_pagination_params!(self, request)?;

        let address = vec_from_hex_prefixed(&inner.address).map_err(|_| {
            Status::invalid_argument("invalid address: expected a 0x-prefixed hex string")
        })?;

        let filter = build_chain_bridge_filter(
            inner.home_chain_id,
            inner.counterparty_chain_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            inner.bridge_ids.as_deref(),
            self.indexed_chains.as_ref(),
            inner.include_unindexed_chains.unwrap_or(false),
        )?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_transfers(
                None,
                Some(address),
                filter,
                page_size,
                is_last_page,
                input_pagination,
            )
            .await
            .map_err(map_db_error)?;

        let items = self.joined_transfers_logic_to_proto(items).await?;

        let response = GetTransfersResponse {
            items,
            next_page_params: output_pagination
                .next_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
            prev_page_params: output_pagination
                .prev_marker
                .map(|p| p.to_proto(self.api_settings.use_pagination_token)),
        };
        Ok(Response::new(response))
    }

    async fn get_chains(
        &self,
        request: Request<GetChainsRequest>,
    ) -> Result<Response<GetChainsResponse>, Status> {
        let inner = request.into_inner();
        let chain_ids = parse_chain_ids_csv("chain_ids", inner.chain_ids.as_deref())?;
        let mut bridge_ids = parse_bridge_ids_csv(inner.bridge_ids.as_deref())?;
        bridge_ids.sort_unstable();
        bridge_ids.dedup();

        // Chains the selected bridges index today. Only meaningful (and only
        // computed) for a non-empty selection: `selected_configured_union`
        // returns a bare `Vec` where empty means "no candidates", the OPPOSITE
        // of `configured_union()`'s "restrict nothing". Gate on the parsed
        // request value, never on the union's emptiness, or `?bridge_ids=999`
        // would return the whole directory.
        let selected_configured_chain_ids = if bridge_ids.is_empty() {
            None
        } else {
            Some(self.indexed_chains.selected_configured_union(&bridge_ids))
        };

        let models = self
            .chain_info_service
            .get_all_chains_info()
            .await
            .map_err(map_db_error)?;

        let models: Vec<_> = models
            .into_iter()
            .filter(|m| chain_ids.is_empty() || chain_ids.contains(&m.id))
            .filter(|m| {
                selected_configured_chain_ids
                    .as_ref()
                    .is_none_or(|selected| selected.contains(&m.id))
            })
            .collect();

        // The unindexed gate stays last and global: it means "no configured
        // bridge indexes this chain", never "not indexed by the selected
        // bridges". Under a non-empty `bridge_ids` it is currently a no-op
        // (`selected_configured_union ⊆ configured_union`), but that
        // containment is a property of the current derivation, not a guarantee.
        let models = if inner.include_unindexed_chains.unwrap_or(false) {
            models
        } else {
            match self.indexed_chains.configured_union() {
                // An empty union can only mean no bridge is configured at all;
                // emptying the chain directory in that case would be exactly the
                // retroactive reinterpretation ADR-004 Decision 5 forbids.
                Some(union) if !union.is_empty() => models
                    .into_iter()
                    .filter(|m| union.contains(&m.id))
                    .collect(),
                _ => models,
            }
        };

        let items = models.into_iter().map(chain_model_to_proto).collect();
        Ok(Response::new(GetChainsResponse { items }))
    }

    async fn get_bridges(
        &self,
        _request: Request<GetBridgesRequest>,
    ) -> Result<Response<GetBridgesResponse>, Status> {
        let rows = self.db.get_all_bridges().await.map_err(map_db_error)?;
        let items = rows
            .into_iter()
            .map(|m| bridge_model_to_proto(m, &self.indexed_chains))
            .collect::<Result<Vec<_>, Status>>()?;
        Ok(Response::new(GetBridgesResponse { items }))
    }
}

fn message_status_to_proto(status: &DbMessageStatus) -> MessageStatus {
    match status {
        DbMessageStatus::Initiated => MessageStatus::MessageStatusInitiated,
        DbMessageStatus::Completed => MessageStatus::MessageStatusCompleted,
        DbMessageStatus::Failed => MessageStatus::MessageStatusFailed,
        DbMessageStatus::ReadyToClaim => MessageStatus::MessageStatusReadyToClaim,
    }
}

fn token_info_logic_to_proto(model: TokenInfoModel) -> TokenInfo {
    TokenInfo {
        address_hash: to_hex_prefixed(model.address.as_slice()),
        name: model.name,
        symbol: model.symbol,
        decimals: model.decimals.map(|d| d.to_string()),
        icon_url: model.token_icon,
    }
}
