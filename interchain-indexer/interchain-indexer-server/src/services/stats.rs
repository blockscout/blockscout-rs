use crate::{
    proto::{interchain_statistics_service_server::*, *},
    settings::ApiSettings,
};
use chrono::{DateTime, Utc};
use interchain_indexer_logic::{
    BridgedTokenListRow, BridgedTokensPaginationLogic, BridgedTokensSortField,
    BridgedTokensSortOrder, StatsService,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct InterchainStatisticsServiceImpl {
    pub stats: Arc<StatsService>,
    pub api_settings: ApiSettings,
}

impl InterchainStatisticsServiceImpl {
    pub fn new(stats: Arc<StatsService>, api_settings: ApiSettings) -> Self {
        Self {
            stats,
            api_settings,
        }
    }
}

#[async_trait::async_trait]
impl InterchainStatisticsService for InterchainStatisticsServiceImpl {
    async fn get_common_statistics(
        &self,
        request: Request<GetCommonStatisticsRequest>,
    ) -> Result<Response<GetCommonStatisticsResponse>, Status> {
        let inner = request.into_inner();
        let timestamp = inner
            .timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0).map(|dt| dt.naive_utc()))
            .unwrap_or_else(|| Utc::now().naive_utc());

        let counters = self
            .stats
            .interchain_db()
            .get_total_counters(timestamp, None, None)
            .await
            .map_err(map_stats_error)?;

        let response = GetCommonStatisticsResponse {
            timestamp: super::utils::db_datetime_to_string(counters.timestamp),
            total_messages: counters.total_messages,
            total_transfers: counters.total_transfers,
        };
        Ok(Response::new(response))
    }

    async fn get_daily_statistics(
        &self,
        request: Request<GetDailyStatisticsRequest>,
    ) -> Result<Response<GetDailyStatisticsResponse>, Status> {
        let inner = request.into_inner();
        let timestamp = inner
            .timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0).map(|dt| dt.naive_utc()))
            .unwrap_or_else(|| Utc::now().naive_utc());

        let counters = self
            .stats
            .interchain_db()
            .get_daily_counters(timestamp, None, None)
            .await
            .map_err(map_stats_error)?;

        let response = GetDailyStatisticsResponse {
            date: counters.date.to_string(),
            daily_messages: counters.daily_messages,
            daily_transfers: counters.daily_transfers,
        };
        Ok(Response::new(response))
    }

    async fn get_bridged_tokens(
        &self,
        request: Request<GetBridgedTokensRequest>,
    ) -> Result<Response<GetBridgedTokensResponse>, Status> {
        let inner = request.into_inner();
        let sort = BridgedTokensSortField::from_proto_sort(inner.sort);
        let order = BridgedTokensSortOrder::from_proto_order(inner.order);

        let input_pagination = if self.api_settings.use_pagination_token {
            if let Some(t) = inner.page_token.as_deref() {
                let m = BridgedTokensPaginationLogic::from_token(t)
                    .map_err(|e| Status::invalid_argument(e.to_string()))?;
                m.ensure_matches_request(inner.chain_id, sort, order)
                    .map_err(|e| Status::invalid_argument(e.to_string()))?;
                Some(m)
            } else {
                None
            }
        } else {
            let lp = BridgedTokensListPagination {
                page_token: inner.page_token.clone(),
                direction: inner.direction.clone(),
                asset_id: inner.asset_id,
                name: inner.name.clone(),
                name_blank: inner.name_blank,
                count: inner.count,
            };
            BridgedTokensPaginationLogic::try_from_list_pagination_proto(
                inner.chain_id,
                sort,
                order,
                &lp,
            )
            .map_err(|e| Status::invalid_argument(e.to_string()))?
        };

        let page_size = inner
            .page_size
            .unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size) as usize;
        let last_page = inner.last_page.unwrap_or(false);

        let (rows, pagination) = self
            .stats
            .get_bridged_tokens_for_chain(
                inner.chain_id,
                sort,
                order,
                page_size,
                last_page,
                input_pagination,
            )
            .await
            .map_err(map_stats_error)?;

        let use_tok = self.api_settings.use_pagination_token;
        let response = GetBridgedTokensResponse {
            items: rows.into_iter().map(bridged_row_to_proto).collect(),
            next_page_params: pagination
                .next_marker
                .map(|m| m.to_list_pagination_proto(use_tok)),
            prev_page_params: pagination
                .prev_marker
                .map(|m| m.to_list_pagination_proto(use_tok)),
        };
        Ok(Response::new(response))
    }
}

fn bridged_row_to_proto(row: BridgedTokenListRow) -> StatsBridgedTokenRow {
    let a = row.aggregate;
    StatsBridgedTokenRow {
        stats_asset_id: a.stats_asset_id,
        name: a.name,
        symbol: a.symbol,
        icon_url: a.icon_url,
        input_transfers_count: i64_to_u64_nonneg(a.input_transfers_count),
        output_transfers_count: i64_to_u64_nonneg(a.output_transfers_count),
        total_transfers_count: i64_to_u64_nonneg(a.total_transfers_count),
        tokens: row
            .tokens
            .into_iter()
            .map(|t| StatsBridgedTokenItem {
                chain_id: t.chain_id,
                token_address: t.token_address.into(),
                name: t.name,
                symbol: t.symbol,
                icon_url: t.icon_url,
                decimals: t.decimals.map(|d| d as u32),
            })
            .collect(),
    }
}

fn i64_to_u64_nonneg(v: i64) -> u64 {
    v.max(0) as u64
}

fn map_stats_error(err: anyhow::Error) -> Status {
    Status::internal(err.to_string())
}
