// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cluster entry points of the interchain Read API. Each one resolves the
//! cluster first (unknown ⇒ NotFound, before any other validation), then
//! mirrors the legacy entry point with the cluster's `ReadScope` in place of
//! the cluster query params, and calls the same core.

use super::{InterchainServiceImpl, parse_hex_path};
use crate::{
    proto::{interchain_cluster_service_server::InterchainClusterService, *},
    services::utils::{
        build_chain_bridge_filter, checked_bridge_id, list_page_params, non_empty,
        parse_chain_ids_csv,
    },
};
use interchain_indexer_logic::pagination::{MessagesPaginationLogic, TransfersPaginationLogic};
use tonic::{Request, Response, Status};

#[async_trait::async_trait]
impl InterchainClusterService for InterchainServiceImpl {
    async fn get_chains(
        &self,
        request: Request<GetClusterChainsRequest>,
    ) -> Result<Response<GetChainsResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let inner = request.into_inner();
        let chain_ids = parse_chain_ids_csv("chain_ids", inner.chain_ids.as_deref())?;
        self.chains_list(scope, non_empty(chain_ids))
            .await
            .map(Response::new)
    }

    async fn get_bridges(
        &self,
        request: Request<GetClusterBridgesRequest>,
    ) -> Result<Response<GetBridgesResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        self.bridges_list(scope.bridge_ids.as_deref())
            .await
            .map(Response::new)
    }

    async fn get_messages(
        &self,
        request: Request<GetClusterMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, MessagesPaginationLogic)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.messages_page(
            None,
            None,
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }

    async fn get_message_details(
        &self,
        request: Request<GetClusterMessageDetailsRequest>,
    ) -> Result<Response<InterchainMessage>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let inner = request.into_inner();
        let message_id = parse_hex_path("message_id", &inner.message_id)?;
        let bridge_id = checked_bridge_id(inner.bridge_id)?;
        // Home, counterparty and bridges of the cluster, plus the indexed-only
        // restriction when `include_unindexed_chains = false`.
        let filter = build_chain_bridge_filter(scope, None, None, self.indexed_chains.as_ref())?;
        self.message_details(message_id, bridge_id, Some(&filter))
            .await
            .map(Response::new)
    }

    async fn get_messages_by_transaction(
        &self,
        request: Request<GetClusterMessagesByTransactionRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, MessagesPaginationLogic)?;
        let tx_hash = parse_hex_path("tx_hash", &inner.tx_hash)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.messages_page(
            Some(tx_hash),
            None,
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }

    async fn get_messages_by_address(
        &self,
        request: Request<GetClusterMessagesByAddressRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, MessagesPaginationLogic)?;
        let address = parse_hex_path("address", &inner.address)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.messages_page(
            None,
            Some(address),
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }

    async fn get_transfers(
        &self,
        request: Request<GetClusterTransfersRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, TransfersPaginationLogic)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.transfers_page(
            None,
            None,
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }

    async fn get_transfers_by_transaction(
        &self,
        request: Request<GetClusterTransfersByTransactionRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, TransfersPaginationLogic)?;
        let tx_hash = parse_hex_path("tx_hash", &inner.tx_hash)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.transfers_page(
            Some(tx_hash),
            None,
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }

    async fn get_transfers_by_address(
        &self,
        request: Request<GetClusterTransfersByAddressRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, TransfersPaginationLogic)?;
        let address = parse_hex_path("address", &inner.address)?;
        let filter = build_chain_bridge_filter(
            scope,
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
            self.indexed_chains.as_ref(),
        )?;
        self.transfers_page(
            None,
            Some(address),
            filter,
            page_size,
            is_last_page,
            input_pagination,
        )
        .await
        .map(Response::new)
    }
}
