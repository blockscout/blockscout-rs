use crate::{
    proto::{interchain_service_server::*, *},
    settings::ApiSettings,
};
use interchain_indexer_entity::{
    crosschain_messages::Model as CrosschainMessageModel,
    crosschain_transfers::Model as CrosschainTransferModel, sea_orm_active_enums::MessageStatus,
};
use interchain_indexer_logic::{
    InterchainDatabase,
    pagination::{ListMarker, MessagePaginationLogic, OutputPagination},
};
use std::{collections::BTreeMap, sync::Arc};
use tonic::{Request, Response, Status};

pub struct InterchainServiceImpl {
    pub db: Arc<InterchainDatabase>,
    pub api_settings: ApiSettings,
}

impl InterchainServiceImpl {
    pub fn new(db: Arc<InterchainDatabase>, api_settings: ApiSettings) -> Self {
        Self { db, api_settings }
    }

    fn extract_input_pagination(
        &self,
        request: &GetMessagesRequest,
    ) -> anyhow::Result<Option<MessagePaginationLogic>> {
        if let Some(pagination_token) = &request.page_token {
            Ok(Some(MessagePaginationLogic::from_token(pagination_token)?))
        } else {
            Ok(None)
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
        pagination.next_marker.map(|marker| Pagination {
            page_token: marker.token().ok(),
        })
    }

    fn produce_prev_pagination(
        &self,
        pagination: &OutputPagination<MessagePaginationLogic>,
    ) -> Option<Pagination> {
        pagination.prev_marker.map(|marker| Pagination {
            page_token: marker.token().ok(),
        })
    }

    fn message_model_to_proto(
        &self,
        message: CrosschainMessageModel,
        transfers: Vec<CrosschainTransferModel>,
    ) -> InterchainMessage {
        let status = message_status_to_str(&message.status).to_string();
        let message_id = message
            .native_id
            .as_ref()
            .map(|id| to_hex_prefixed(id.as_slice()))
            .unwrap_or_else(|| message.id.to_string());
        let bridge_name = message.bridge_id.to_string();
        let source_chain_id = message.src_chain_id.to_string();
        let destination_chain_id = message
            .dst_chain_id
            .map(|id| id.to_string())
            .unwrap_or_default();

        let sender = hex_string(message.sender_address.as_ref());
        let recipient = hex_string(message.recipient_address.as_ref());
        let source_transaction_hash = hex_string(message.src_tx_hash.as_ref());
        let destination_transaction_hash = hex_string(message.dst_tx_hash.as_ref());

        let send_timestamp = message.init_timestamp.and_utc().timestamp().to_string();
        let receive_timestamp = message
            .last_update_timestamp
            .map(|ts| ts.and_utc().timestamp().to_string());

        let payload = message
            .payload
            .as_ref()
            .map(|payload| to_hex_prefixed(payload.as_slice()));

        let transfers = transfers
            .into_iter()
            .map(|transfer| {
                let sender = with_fallback(hex_string(transfer.sender_address.as_ref()), &sender);
                let recipient =
                    with_fallback(hex_string(transfer.recipient_address.as_ref()), &recipient);

                InterchainTransfer {
                    bridge_name: bridge_name.clone(),
                    message_id: message_id.clone(),
                    status: status.clone(),
                    source_token: Some(TokenInfo {
                        chain_id: transfer.token_src_chain_id.to_string(),
                        address: to_hex_prefixed(transfer.token_src_address.as_slice()),
                        name: String::new(),
                        symbol: String::new(),
                        decimals: transfer.src_decimals.to_string(),
                        icon_url: String::new(),
                    }),
                    source_amount: Some(transfer.src_amount.to_string()),
                    source_transaction_hash: option_string(source_transaction_hash.clone()),
                    sender,
                    destination_token: Some(TokenInfo {
                        chain_id: transfer.token_dst_chain_id.to_string(),
                        address: to_hex_prefixed(transfer.token_dst_address.as_slice()),
                        name: String::new(),
                        symbol: String::new(),
                        decimals: transfer.dst_decimals.to_string(),
                        icon_url: String::new(),
                    }),
                    destination_amount: Some(transfer.dst_amount.to_string()),
                    destination_transaction_hash: option_string(
                        destination_transaction_hash.clone(),
                    ),
                    recipient,
                }
            })
            .collect();

        InterchainMessage {
            message_id,
            status,
            source_chain_id,
            sender,
            source_transaction_hash,
            send_timestamp,
            bridge_name,
            destination_chain_id,
            recipient,
            destination_transaction_hash,
            receive_timestamp: receive_timestamp.unwrap_or_default(),
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

fn to_hex_prefixed(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        String::new()
    } else {
        format!("0x{}", hex::encode(bytes))
    }
}

fn hex_string(data: Option<&Vec<u8>>) -> String {
    data.map(|bytes| to_hex_prefixed(bytes.as_slice()))
        .unwrap_or_default()
}

fn option_string(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
fn with_fallback(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}
