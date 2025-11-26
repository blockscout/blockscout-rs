use crate::{
    proto::{interchain_service_server::*, *},
    settings::ApiSettings,
};
use anyhow::anyhow;
use chrono::NaiveDateTime;
use interchain_indexer_entity::{
    crosschain_messages::Model as CrosschainMessageModel,
    crosschain_transfers::Model as CrosschainTransferModel, sea_orm_active_enums::MessageStatus,
};
use interchain_indexer_logic::{
    InterchainDatabase,
    pagination::{ListMarker, MessagePaginationLogic, OutputPagination, PaginationDirection},
    utils::{hex_string_opt, to_hex_prefixed},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tonic::{Request, Response, Status};

pub struct InterchainServiceImpl {
    pub db: Arc<InterchainDatabase>,
    pub bridges_names: HashMap<i32, String>,
    pub api_settings: ApiSettings,
}

impl InterchainServiceImpl {
    pub fn new(
        db: Arc<InterchainDatabase>,
        bridges_names: HashMap<i32, String>,
        api_settings: ApiSettings,
    ) -> Self {
        Self {
            db,
            bridges_names,
            api_settings,
        }
    }

    /// Input pagination can be either a token or a set of
    /// timestamp, message_id, bridge_id and direction
    /// (depending on the API settings)
    fn extract_input_pagination(
        &self,
        request: &GetMessagesRequest,
    ) -> anyhow::Result<Option<MessagePaginationLogic>> {
        if self.api_settings.use_pagination_token {
            if let Some(pagination_token) = &request.page_token {
                Ok(Some(MessagePaginationLogic::from_token(pagination_token)?))
            } else {
                Ok(None)
            }
        } else {
            // let timestamp = request.timestamp;
            // let message_id = request.message_id.clone();
            // let bridge_id = request.bridge_id;
            // let direction = request.direction.clone();

            match (
                request.timestamp,
                request.message_id.clone(),
                request.bridge_id,
                request.direction.clone(),
            ) {
                // all input pagination parameters are provided
                (Some(timestamp), Some(message_id), Some(bridge_id), Some(direction)) => {
                    Ok(Some(MessagePaginationLogic::new(
                        timestamp as i64,
                        message_id,
                        bridge_id,
                        PaginationDirection::from_string(&direction)?,
                    )?))
                }

                // no input pagination povided
                (None, None, None, None) => Ok(None),

                // some input pagination parameters are missing
                _ => Err(anyhow!(
                    "Pagination error: timestamp, message_id, bridge_id and direction must be provided together"
                )),
            }
        }
    }

    fn extract_page_size(&self, request: &GetMessagesRequest) -> usize {
        request
            .page_size
            .unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size) as usize
    }

    fn extract_is_last_page(&self, request: &GetMessagesRequest) -> bool {
        request.last_page.unwrap_or(false)
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
        let inner = request.into_inner();
        let input_pagination = self
            .extract_input_pagination(&inner)
            .map_err(map_db_error)?;

        let page_size = self.extract_page_size(&inner);
        let is_last_page = self.extract_is_last_page(&inner);

        let (items, output_pagination) = self
            .db
            .get_crosschain_messages(page_size, is_last_page, input_pagination)
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
        _request: Request<GetMessageDetailsRequest>,
    ) -> Result<Response<InterchainMessage>, Status> {
        let response = InterchainMessage {
            ..Default::default()
        };
        Ok(Response::new(response))
    }

    async fn get_messages_by_transaction(
        &self,
        _request: Request<GetMessagesByTransactionRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let response = GetMessagesResponse {
            items: vec![],
            next_page_params: None,
            prev_page_params: None,
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

fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tonic::Status::internal(err.to_string())
}

fn message_status_to_str(status: &MessageStatus) -> &'static str {
    match status {
        MessageStatus::Initiated => "initiated",
        MessageStatus::Completed => "completed",
        MessageStatus::Failed => "failed",
    }
}
