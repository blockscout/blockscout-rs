use crate::{
    proto::{interchain_service_server::*, *},
    settings::ApiSettings,
};
use anyhow::anyhow;
use interchain_indexer_entity::{
    crosschain_messages::Model as CrosschainMessageModel,
    crosschain_transfers::Model as CrosschainTransferModel, sea_orm_active_enums::MessageStatus,
};
use interchain_indexer_logic::{
    InterchainDatabase, TokenInfoService, pagination::{ListMarker, MessagePaginationLogic, OutputPagination, PaginationDirection}, utils::{hex_string_opt, to_hex_prefixed, vec_from_hex_prefixed}
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tonic::{Request, Response, Status};

use super::utils::{db_datetime_to_string, map_db_error};

macro_rules! pagination_params {
    ($service:expr, $request:expr) => {{
        let inner = $request.into_inner();
        let api_settings = &$service.api_settings;

        let input_pagination = if api_settings.use_pagination_token {
            if let Some(pagination_token) = &inner.page_token {
                Some(
                    MessagePaginationLogic::from_token(pagination_token)
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
                        MessagePaginationLogic::new(
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

pub struct InterchainServiceImpl {
    pub db: Arc<InterchainDatabase>,
    pub token_info_service: Arc<TokenInfoService>,
    pub bridges_names: HashMap<i32, String>,
    pub api_settings: ApiSettings,
}

impl InterchainServiceImpl {
    pub fn new(
        db: Arc<InterchainDatabase>,
        token_info_service: Arc<TokenInfoService>,
        bridges_names: HashMap<i32, String>,
        api_settings: ApiSettings,
    ) -> Self {
        Self {
            db,
            token_info_service,
            bridges_names,
            api_settings,
        }
    }

    fn produce_next_pagination(
        &self,
        pagination: &OutputPagination<MessagePaginationLogic>,
    ) -> Option<Pagination> {
        if self.api_settings.use_pagination_token {
            pagination.next_marker.map(|marker| Pagination {
                page_token: marker.token().ok(),
                ..Default::default()
            })
        } else {
            pagination.next_marker.map(|marker| Pagination {
                timestamp: Some(marker.get_timestamp_ns().unwrap() as u64),
                message_id: Some(marker.get_message_id()),
                bridge_id: Some(marker.bridge_id),
                direction: Some(marker.direction.to_string()),
                ..Default::default()
            })
        }
    }

    fn produce_prev_pagination(
        &self,
        pagination: &OutputPagination<MessagePaginationLogic>,
    ) -> Option<Pagination> {
        if self.api_settings.use_pagination_token {
            pagination.prev_marker.map(|marker| Pagination {
                page_token: marker.token().ok(),
                ..Default::default()
            })
        } else {
            pagination.prev_marker.map(|marker| Pagination {
                timestamp: Some(marker.get_timestamp_ns().unwrap() as u64),
                message_id: Some(marker.get_message_id()),
                bridge_id: Some(marker.bridge_id),
                direction: Some(marker.direction.to_string()),
                ..Default::default()
            })
        }
    }

    fn message_model_to_proto(
        &self,
        message: CrosschainMessageModel,
        transfers: Vec<CrosschainTransferModel>,
    ) -> InterchainMessage {
        let message_id = message
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| message.id.to_string());
        let bridge_name = self
            .bridges_names
            .get(&message.bridge_id)
            .cloned()
            .unwrap_or("Unknown".to_string());
        let payload = message
            .payload
            .as_ref()
            .map(|payload| to_hex_prefixed(payload.as_slice()));
        let source_transaction_hash = hex_string_opt(message.src_tx_hash);
        let destination_transaction_hash = hex_string_opt(message.dst_tx_hash);

        let transfers = transfers
            .into_iter()
            .map(|transfer| InterchainTransfer {
                bridge_name: bridge_name.clone(),
                message_id: message_id.clone(),
                status: message_status_to_str(&message.status).to_string(),
                source_token: Some(TokenInfo {
                    chain_id: transfer.token_src_chain_id.to_string(),
                    address: to_hex_prefixed(transfer.token_src_address.as_slice()),
                    name: None,
                    symbol: None,
                    decimals: None,
                    icon_url: None,
                }),
                source_amount: Some(transfer.src_amount.to_string()),
                source_transaction_hash: source_transaction_hash.clone(),
                sender: hex_string_opt(transfer.sender_address),
                destination_token: Some(TokenInfo {
                    chain_id: transfer.token_dst_chain_id.to_string(),
                    address: to_hex_prefixed(transfer.token_dst_address.as_slice()),
                    name: None,
                    symbol: None,
                    decimals: None,
                    icon_url: None,
                }),
                destination_amount: Some(transfer.dst_amount.to_string()),
                destination_transaction_hash: destination_transaction_hash.clone(),
                recipient: hex_string_opt(transfer.recipient_address),
            })
            .collect();

        InterchainMessage {
            message_id,
            status: message_status_to_str(&message.status).to_string(),
            source_chain_id: message.src_chain_id.to_string(),
            sender: hex_string_opt(message.sender_address),
            send_timestamp: db_datetime_to_string(message.init_timestamp),
            source_transaction_hash,
            bridge_name,
            destination_chain_id: message
                .dst_chain_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            recipient: hex_string_opt(message.recipient_address),
            receive_timestamp: message
                .last_update_timestamp
                .map(|ts| db_datetime_to_string(ts)),
            destination_transaction_hash,
            payload,
            extra: BTreeMap::new(),
            transfers,
        }
    }

    fn messages_logic_to_proto(
        &self,
        messages: Vec<(CrosschainMessageModel, Vec<CrosschainTransferModel>)>,
    ) -> Vec<InterchainMessage> {
        messages
            .into_iter()
            .map(|(message, transfers)| self.message_model_to_proto(message, transfers))
            .collect()
    }
}

#[async_trait::async_trait]
impl InterchainService for InterchainServiceImpl {
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let (_inner, input_pagination, page_size, is_last_page) =
            pagination_params!(self, request)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(None, page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items);

        let response = GetMessagesResponse {
            items,
            next_page_params: self.produce_next_pagination(&output_pagination),
            prev_page_params: self.produce_prev_pagination(&output_pagination),
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
                let message = self.message_model_to_proto(message, transfers);
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
        let (inner, input_pagination, page_size, is_last_page) = pagination_params!(self, request)?;

        let tx_hash = vec_from_hex_prefixed(&inner.tx_hash).map_err(map_db_error)?;

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(Some(tx_hash), page_size, is_last_page, input_pagination)
            .await
            .map_err(map_db_error)?;

        let items = self.messages_logic_to_proto(items);

        let response = GetMessagesResponse {
            items,
            next_page_params: self.produce_next_pagination(&output_pagination),
            prev_page_params: self.produce_prev_pagination(&output_pagination),
        };
        Ok(Response::new(response))
    }

    async fn get_transfers(
        &self,
        _request: Request<GetTransfersRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let response = GetTransfersResponse {
            items: vec![],
            next_page_params: None,
            prev_page_params: None,
        };
        Ok(Response::new(response))
    }

    async fn get_transfers_by_message(
        &self,
        _request: Request<GetTransfersByMessageRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let response = GetTransfersResponse {
            items: vec![],
            next_page_params: None,
            prev_page_params: None,
        };
        Ok(Response::new(response))
    }

    async fn get_transfers_by_transaction(
        &self,
        _request: Request<GetTransfersByTransactionRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let response = GetTransfersResponse {
            items: vec![],
            next_page_params: None,
            prev_page_params: None,
        };
        Ok(Response::new(response))
    }
}

fn message_status_to_str(status: &MessageStatus) -> &'static str {
    match status {
        MessageStatus::Initiated => "initiated",
        MessageStatus::Completed => "completed",
        MessageStatus::Failed => "failed",
    }
}
