// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cluster entry points of the statistics Read API (see
//! `interchain_service/cluster.rs` for the shared rules). Per-chain routes
//! derive their focal/peer scope from the cluster and reject a path chain
//! outside it with InvalidArgument. The `bridge_ids` param is a client
//! selection narrowing the cluster's bridges; a selection sharing none with
//! the cluster matches no rows (the response type's `Default`).

use super::{InterchainStatisticsServiceImpl, normalize_stats_q, parse_optional_utc_date};
use crate::{
    proto::{interchain_cluster_statistics_service_server::InterchainClusterStatisticsService, *},
    services::{
        scope::{ChainStatsScope, ReadScope},
        utils::{list_page_params, non_empty, parse_bridge_ids_csv, parse_chain_ids_csv},
    },
};
use interchain_indexer_logic::{
    BridgedTokensPaginationLogic, BridgedTokensSortField, StatsChainsPaginationLogic,
    StatsChainsSortField, StatsListQuery, StatsSortOrder,
};
use tonic::{Request, Response, Status};

/// The per-chain scope of `chain_id` under a cluster, or InvalidArgument when
/// the chain is outside the cluster's chain set.
fn cluster_chain_scope(
    scope: &ReadScope,
    cluster_id: &str,
    chain_id: i64,
) -> Result<ChainStatsScope, Status> {
    scope.chain_stats_scope(chain_id).ok_or_else(|| {
        Status::invalid_argument(format!(
            "chain_id {chain_id} is not in cluster {cluster_id}"
        ))
    })
}

impl InterchainStatisticsServiceImpl {
    /// Shared body of the sent/received cluster message-path routes. The
    /// cluster is resolved first, then the path chain, then the dates.
    async fn cluster_message_paths(
        &self,
        inner: GetClusterMessagePathsRequest,
        outgoing: bool,
    ) -> Result<GetMessagePathsResponse, Status> {
        let scope = cluster_chain_scope(
            self.clusters.scope(&inner.cluster_id)?,
            &inner.cluster_id,
            inner.chain_id,
        )?;
        let from_date = parse_optional_utc_date(inner.from_date.as_deref())?;
        let to_date = parse_optional_utc_date(inner.to_date.as_deref())?;
        let selection = non_empty(parse_bridge_ids_csv(inner.bridge_ids.as_deref())?);
        match scope.narrow_bridges(selection) {
            Some(scope) => {
                self.message_paths(&scope, from_date, to_date, outgoing)
                    .await
            }
            None => Ok(GetMessagePathsResponse::default()),
        }
    }
}

#[async_trait::async_trait]
impl InterchainClusterStatisticsService for InterchainStatisticsServiceImpl {
    async fn get_bridged_tokens(
        &self,
        request: Request<GetClusterBridgedTokensRequest>,
    ) -> Result<Response<GetBridgedTokensResponse>, Status> {
        let scope = {
            let req = request.get_ref();
            cluster_chain_scope(
                self.clusters.scope(&req.cluster_id)?,
                &req.cluster_id,
                req.chain_id,
            )?
        };
        let (inner, input_pagination, page_size, last_page) =
            list_page_params!(self, request, BridgedTokensPaginationLogic)?;
        let sort = BridgedTokensSortField::from_proto_sort(inner.sort);
        let order = StatsSortOrder::from_proto_order(inner.order)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let q = normalize_stats_q(inner.q.as_deref());
        let selection = non_empty(parse_bridge_ids_csv(inner.bridge_ids.as_deref())?);
        match scope.narrow_bridges(selection) {
            Some(scope) => {
                self.bridged_tokens_page(
                    &scope,
                    StatsListQuery {
                        sort,
                        order,
                        page_size,
                        last_page,
                        input_pagination,
                        q,
                    },
                )
                .await
            }
            None => Ok(GetBridgedTokensResponse::default()),
        }
        .map(Response::new)
    }

    async fn get_chains_stats(
        &self,
        request: Request<GetClusterChainsStatsRequest>,
    ) -> Result<Response<GetChainsStatsResponse>, Status> {
        let scope = self.clusters.scope(&request.get_ref().cluster_id)?;
        let (inner, input_pagination, page_size, last_page) =
            list_page_params!(self, request, StatsChainsPaginationLogic)?;
        let chain_ids = parse_chain_ids_csv("chain_ids", inner.chain_ids.as_deref())?;
        let sort = StatsChainsSortField::from_proto_sort(inner.sort);
        let order = StatsSortOrder::from_proto_order(inner.order)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let q = normalize_stats_q(inner.q.as_deref());
        let selection = non_empty(parse_bridge_ids_csv(inner.bridge_ids.as_deref())?);
        match scope.narrow_bridges(selection) {
            Some(scope) => {
                self.chains_stats_page(
                    &scope,
                    non_empty(chain_ids),
                    StatsListQuery {
                        sort,
                        order,
                        page_size,
                        last_page,
                        input_pagination,
                        q,
                    },
                )
                .await
            }
            None => Ok(GetChainsStatsResponse::default()),
        }
        .map(Response::new)
    }

    async fn get_sent_message_paths(
        &self,
        request: Request<GetClusterMessagePathsRequest>,
    ) -> Result<Response<GetMessagePathsResponse>, Status> {
        self.cluster_message_paths(request.into_inner(), true)
            .await
            .map(Response::new)
    }

    async fn get_received_message_paths(
        &self,
        request: Request<GetClusterMessagePathsRequest>,
    ) -> Result<Response<GetMessagePathsResponse>, Status> {
        self.cluster_message_paths(request.into_inner(), false)
            .await
            .map(Response::new)
    }
}
