use crate::{
    BridgeConfig,
    proto::{interchain_service_server::*, *},
    settings::ApiSettings,
};
use anyhow::anyhow;
use interchain_indexer_entity::{
    chains::Model as ChainModel, crosschain_messages::Model as CrosschainMessageModel,
    crosschain_transfers::Model as CrosschainTransferModel,
    sea_orm_active_enums::MessageStatus as DbMessageStatus, tokens::Model as TokenInfoModel,
};
use interchain_indexer_logic::{
    ChainInfoService, InterchainDatabase, JoinedTransfer, TokenInfoService,
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

use super::utils::{db_datetime_to_string, map_db_error};

macro_rules! messages_pagination_params {
    ($service:expr, $request:expr) => {{
        let inner = $request.into_inner();
        let api_settings = &$service.api_settings;

        let input_pagination = if api_settings.use_pagination_token {
            if let Some(pagination_token) = &inner.page_token {
                Some(
                    MessagesPaginationLogic::from_token(pagination_token)
                        .map_err(map_db_error)?,
                )
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
                (Some(timestamp), Some(message_id), Some(bridge_id), Some(direction)) => {
                    Some(
                        MessagesPaginationLogic::new(
                            timestamp as i64,
                            message_id,
                            bridge_id,
                            PaginationDirection::from_string(&direction)
                                .map_err(map_db_error)?,
                        )
                        .map_err(map_db_error)?,
                    )
                }

                (None, None, None, None) => None,

                _ => {
                    return Err(map_db_error(anyhow!(
                        "Pagination error: timestamp, message_id, bridge_id and direction must be provided together"
                    )))
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
                Some(
                    TransfersPaginationLogic::from_token(pagination_token)
                        .map_err(map_db_error)?,
                )
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
                (Some(timestamp), Some(message_id), Some(bridge_id), Some(index), Some(direction)) => {
                    Some(
                        TransfersPaginationLogic::new(
                            timestamp as i64,
                            message_id,
                            bridge_id,
                            index,
                            PaginationDirection::from_string(&direction)
                                .map_err(map_db_error)?,
                        )
                        .map_err(map_db_error)?,
                    )
                }

                (None, None, None, None, None) => None,

                _ => {
                    return Err(map_db_error(anyhow!(
                        "Pagination error: timestamp, message_id, bridge_id, index and direction must be provided together"
                    )))
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
}

impl InterchainServiceImpl {
    pub fn new(
        db: Arc<InterchainDatabase>,
        token_info_service: Arc<TokenInfoService>,
        chain_info_service: Arc<ChainInfoService>,
        bridges: Vec<BridgeConfig>,
        api_settings: ApiSettings,
    ) -> Self {
        Self {
            db,
            token_info_service,
            chain_info_service,
            bridges_map: bridges
                .into_iter()
                .map(|b| {
                    (
                        b.bridge_id,
                        BridgeInfo {
                            name: b.name,
                            ui_url: b.ui_url,
                            docs_url: b.docs_url,
                        },
                    )
                })
                .collect(),
            api_settings,
        }
    }

    async fn message_model_to_proto(
        &self,
        message: CrosschainMessageModel,
        transfers: Vec<CrosschainTransferModel>,
    ) -> InterchainMessage {
        let payload = message
            .payload
            .as_ref()
            .map(|payload| to_hex_prefixed(payload.as_slice()));

        let transfers = futures::future::join_all(transfers.into_iter().map(|transfer| {
            let message = message.clone();
            async move { self.transfer_logic_to_proto(&transfer, &message).await }
        }))
        .await;

        let source_chain = self
            .get_chain_info(message.src_chain_id as u64)
            .await
            .into();
        let destination_chain = match message.dst_chain_id {
            Some(id) => Some(self.get_chain_info(id as u64).await),
            None => None,
        };

        InterchainMessage {
            bridge: self.get_bridge_info(message.bridge_id).into(),
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
        }
    }

    async fn messages_logic_to_proto(
        &self,
        messages: Vec<(CrosschainMessageModel, Vec<CrosschainTransferModel>)>,
    ) -> Vec<InterchainMessage> {
        let futures = messages.into_iter().map(|(message, transfers)| async move {
            self.message_model_to_proto(message, transfers).await
        });

        futures::future::join_all(futures).await
    }

    async fn joined_transfers_logic_to_proto(
        &self,
        transfers: Vec<JoinedTransfer>,
    ) -> Vec<InterchainTransfer> {
        let futures = transfers
            .into_iter()
            .map(|t| async move { self.joined_transfer_logic_to_proto(&t).await });

        futures::future::join_all(futures).await
    }

    async fn transfer_logic_to_proto(
        &self,
        transfer: &CrosschainTransferModel,
        message: &CrosschainMessageModel,
    ) -> InterchainTransfer {
        InterchainTransfer {
            bridge: self.get_bridge_info(message.bridge_id).into(),
            message_id: self.get_message_id_from_message(message),
            status: message_status_to_proto(&message.status) as i32,
            source_chain: Some(
                self.get_chain_info(transfer.token_src_chain_id as u64)
                    .await,
            ),
            destination_chain: Some(
                self.get_chain_info(transfer.token_dst_chain_id as u64)
                    .await,
            ),
            source_token: self
                .get_token_info(
                    transfer.token_src_chain_id as u64,
                    transfer.token_src_address.clone(),
                )
                .await,
            source_amount: Some(transfer.src_amount.to_plain_string()),
            source_transaction_hash: hex_string_opt(message.src_tx_hash.clone()),
            sender: self.get_address_info_opt(transfer.sender_address.clone()),
            send_timestamp: db_datetime_to_string(message.init_timestamp),
            destination_token: self
                .get_token_info(
                    transfer.token_dst_chain_id as u64,
                    transfer.token_dst_address.clone(),
                )
                .await,
            destination_amount: Some(transfer.dst_amount.to_plain_string()),
            destination_transaction_hash: hex_string_opt(message.dst_tx_hash.clone()),
            recipient: self.get_address_info_opt(transfer.recipient_address.clone()),
            receive_timestamp: message.last_update_timestamp.map(db_datetime_to_string),
        }
    }

    async fn joined_transfer_logic_to_proto(
        &self,
        transfer: &JoinedTransfer,
    ) -> InterchainTransfer {
        InterchainTransfer {
            bridge: self.get_bridge_info(transfer.bridge_id).into(),
            message_id: self.get_message_id_from_joined_transfer(transfer),
            status: message_status_to_proto(&transfer.status) as i32,
            source_chain: Some(
                self.get_chain_info(transfer.token_src_chain_id as u64)
                    .await,
            ),
            destination_chain: Some(
                self.get_chain_info(transfer.token_dst_chain_id as u64)
                    .await,
            ),
            source_token: self
                .get_token_info(
                    transfer.token_src_chain_id as u64,
                    transfer.token_src_address.clone(),
                )
                .await,
            source_amount: Some(transfer.src_amount.to_plain_string()),
            source_transaction_hash: hex_string_opt(transfer.src_tx_hash.clone()),
            sender: self.get_address_info_opt(transfer.sender_address.clone()),
            send_timestamp: db_datetime_to_string(transfer.init_timestamp),
            destination_token: self
                .get_token_info(
                    transfer.token_dst_chain_id as u64,
                    transfer.token_dst_address.clone(),
                )
                .await,
            destination_amount: Some(transfer.dst_amount.to_plain_string()),
            destination_transaction_hash: hex_string_opt(transfer.dst_tx_hash.clone()),
            recipient: self.get_address_info_opt(transfer.recipient_address.clone()),
            receive_timestamp: transfer.last_update_timestamp.map(db_datetime_to_string),
        }
    }

    fn get_bridge_info(&self, bridge_id: i32) -> BridgeInfo {
        self.bridges_map
            .get(&bridge_id)
            .cloned()
            .unwrap_or(BridgeInfo {
                name: "Unknown".to_string(),
                ui_url: None,
                docs_url: None,
            })
    }

    fn get_message_id_from_message(&self, message: &CrosschainMessageModel) -> String {
        message
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| message.id.to_string())
    }

    fn get_message_id_from_joined_transfer(&self, joined: &JoinedTransfer) -> String {
        joined
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| joined.message_id.to_string())
    }

    async fn get_token_info(&self, chain_id: u64, address: Vec<u8>) -> Option<TokenInfo> {
        let address_hex = to_hex_prefixed(address.as_slice());
        self
            .token_info_service
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

    async fn get_chain_info(&self, chain_id: u64) -> ChainInfo {
        chain_info_logic_to_proto(self.chain_info_service.get_chain_info(chain_id).await)
    }
}

#[async_trait::async_trait]
impl InterchainService for InterchainServiceImpl {
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (_inner, input_pagination, page_size, is_last_page) =
            messages_pagination_params!(self, request)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(None, page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items).await;

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
        let response = match self
            .db
            .get_crosschain_message(vec_from_hex_prefixed(&inner.message_id).map_err(map_db_error)?)
            .await
        {
            Ok(Some((message, transfers))) => {
                let message = self.message_model_to_proto(message, transfers).await;
                Ok(message)
            }
            Ok(None) => Err(tonic::Status::not_found("Message not found")),
            Err(e) => Err(map_db_error(e)),
        }?;

        Ok(Response::new(response))
    }

    async fn get_messages_by_transaction(
        &self,
        request: Request<GetMessagesByTransactionRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (inner, input_pagination, page_size, is_last_page) =
            messages_pagination_params!(self, request)?;

        let tx_hash = vec_from_hex_prefixed(&inner.tx_hash).map_err(map_db_error)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(Some(tx_hash), page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items).await;

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
        let (_inner, input_pagination, page_size, is_last_page) =
            transfers_pagination_params!(self, request)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_transfers(None, page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.joined_transfers_logic_to_proto(items).await;

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

        let tx_hash = vec_from_hex_prefixed(&inner.tx_hash).map_err(map_db_error)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_transfers(Some(tx_hash), page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.joined_transfers_logic_to_proto(items).await;

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
}

fn message_status_to_proto(status: &DbMessageStatus) -> MessageStatus {
    match status {
        DbMessageStatus::Initiated => MessageStatus::MessageStatusInitiated,
        DbMessageStatus::Completed => MessageStatus::MessageStatusCompleted,
        DbMessageStatus::Failed => MessageStatus::MessageStatusFailed,
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

fn chain_info_logic_to_proto(model: ChainModel) -> ChainInfo {
    ChainInfo {
        id: model.id.to_string(),
        name: model.name,
        logo: model.icon,
        explorer_url: model.explorer,
        custom_tx_route: model
            .custom_routes
            .clone()
            .and_then(|routes| routes.get("tx").and_then(|v| v.as_str()).map(String::from)),
        custom_address_route: model.custom_routes.clone().and_then(|routes| {
            routes
                .get("address")
                .and_then(|v| v.as_str())
                .map(String::from)
        }),
        custom_token_route: model.custom_routes.and_then(|routes| {
            routes
                .get("token")
                .and_then(|v| v.as_str())
                .map(String::from)
        }),
    }
}
