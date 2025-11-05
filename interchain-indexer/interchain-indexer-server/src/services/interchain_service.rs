use crate::proto::{interchain_service_server::*, *};
use interchain_indexer_logic::InterchainDatabase;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct InterchainServiceImpl {
    pub db: Arc<InterchainDatabase>,
}

impl InterchainServiceImpl {
    pub fn new(db: Arc<InterchainDatabase>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl InterchainService for InterchainServiceImpl {
    async fn get_messages(
        &self,
        _request: Request<GetMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let response = GetMessagesResponse {
            items: vec![],
            next_page_params: None,
            prev_page_params: None,
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
