// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cluster entry points of the interchain Read API. Each one resolves the
//! cluster first (unknown ⇒ NotFound, before any other validation the handler
//! does), then mirrors the legacy entry point with the cluster's `ReadScope` in
//! place of the cluster query params (except `bridge_ids`, a client selection
//! that narrows the scope's bridges), and calls the same core. Params the HTTP
//! layer cannot parse into their declared type (a non-integer `chain_id` or
//! `page_size`) are rejected with 400 by the extractors before any handler runs.

use super::{InterchainServiceImpl, parse_hex_path};
use crate::{
    proto::{interchain_cluster_service_server::InterchainClusterService, *},
    services::{
        scope::ReadScope,
        utils::{
            build_chain_bridge_filter, checked_bridge_id, list_page_params, non_empty,
            parse_bridge_ids_csv, parse_chain_ids_csv,
        },
    },
};
use interchain_indexer_logic::{
    ChainBridgeFilter,
    pagination::{MessagesPaginationLogic, TransfersPaginationLogic},
};
use tonic::{Request, Response, Status};

impl InterchainServiceImpl {
    /// The list filter of a cluster request, or `None` when the client's
    /// `bridge_ids` share no bridge with the cluster. Every param is validated
    /// in both cases.
    fn cluster_list_filter(
        &self,
        scope: &ReadScope,
        bridge_ids: Option<&str>,
        src_chain_ids: Option<&str>,
        dst_chain_ids: Option<&str>,
    ) -> Result<Option<ChainBridgeFilter>, Status> {
        let selection = non_empty(parse_bridge_ids_csv(bridge_ids)?);
        let narrowed = scope.narrow_bridges(selection);
        // A disjoint selection still validates src/dst against the cluster scope.
        let filter = build_chain_bridge_filter(
            narrowed.as_ref().unwrap_or(scope),
            src_chain_ids,
            dst_chain_ids,
            self.indexed_chains.as_ref(),
        )?;
        Ok(narrowed.map(|_| filter))
    }
}

#[async_trait::async_trait]
impl InterchainClusterService for InterchainServiceImpl {
    async fn get_chains(
        &self,
        request: Request<GetClusterChainsRequest>,
    ) -> Result<Response<GetChainsResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let inner = request.into_inner();
        let chain_ids = parse_chain_ids_csv("chain_ids", inner.chain_ids.as_deref())?;
        let selection = non_empty(parse_bridge_ids_csv(inner.bridge_ids.as_deref())?);
        match scope.narrow_bridges(selection) {
            Some(scope) => self.chains_list(&scope, non_empty(chain_ids)).await,
            None => Ok(GetChainsResponse::default()),
        }
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
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.messages_page(
                    None,
                    None,
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetMessagesResponse::default()),
        }
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
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.messages_page(
                    Some(tx_hash),
                    None,
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetMessagesResponse::default()),
        }
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
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.messages_page(
                    None,
                    Some(address),
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetMessagesResponse::default()),
        }
        .map(Response::new)
    }

    async fn get_transfers(
        &self,
        request: Request<GetClusterTransfersRequest>,
    ) -> Result<Response<GetTransfersResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, is_last_page) =
            list_page_params!(self, request, TransfersPaginationLogic)?;
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.transfers_page(
                    None,
                    None,
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetTransfersResponse::default()),
        }
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
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.transfers_page(
                    Some(tx_hash),
                    None,
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetTransfersResponse::default()),
        }
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
        let filter = self.cluster_list_filter(
            scope,
            inner.bridge_ids.as_deref(),
            inner.src_chain_ids.as_deref(),
            inner.dst_chain_ids.as_deref(),
        )?;
        match filter {
            Some(filter) => {
                self.transfers_page(
                    None,
                    Some(address),
                    filter,
                    page_size,
                    is_last_page,
                    input_pagination,
                )
                .await
            }
            None => Ok(GetTransfersResponse::default()),
        }
        .map(Response::new)
    }
}
