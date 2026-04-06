use super::chain_info_proto::chain_model_to_proto;
use crate::{
    proto::{interchain_statistics_service_server::*, *},
    settings::ApiSettings,
};
use chrono::{DateTime, NaiveDate, Utc};
use interchain_indexer_logic::{
    BridgedTokenListRow, BridgedTokensPaginationLogic, BridgedTokensSortField, ChainInfoService,
    StatsChainListRow, StatsChainsPaginationLogic, StatsService, StatsSortOrder,
    utils::to_hex_prefixed,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct InterchainStatisticsServiceImpl {
    pub stats: Arc<StatsService>,
    pub api_settings: ApiSettings,
    pub chain_info: Arc<ChainInfoService>,
}

impl InterchainStatisticsServiceImpl {
    pub fn new(
        stats: Arc<StatsService>,
        api_settings: ApiSettings,
        chain_info: Arc<ChainInfoService>,
    ) -> Self {
        Self {
            stats,
            api_settings,
            chain_info,
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
        let order = StatsSortOrder::from_proto_order(inner.order)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let input_pagination = if self.api_settings.use_pagination_token {
            if let Some(t) = inner.page_token.as_deref() {
                let m = BridgedTokensPaginationLogic::from_token(t)
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
            BridgedTokensPaginationLogic::try_from_list_pagination_proto(&lp)
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

    async fn get_chains_stats(
        &self,
        request: Request<GetChainsStatsRequest>,
    ) -> Result<Response<GetChainsStatsResponse>, Status> {
        let inner = request.into_inner();
        let chain_ids = parse_chain_ids_csv(inner.chain_ids.as_deref())?;
        let order = StatsSortOrder::from_proto_order(inner.order)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let input_pagination = if self.api_settings.use_pagination_token {
            if let Some(t) = inner.page_token.as_deref() {
                let m = StatsChainsPaginationLogic::from_token(t)
                    .map_err(|e| Status::invalid_argument(e.to_string()))?;
                Some(m)
            } else {
                None
            }
        } else {
            let lp = StatsChainsListPagination {
                page_token: inner.page_token.clone(),
                direction: inner.direction.clone(),
                count: inner.count,
                chain_id: inner.chain_id,
            };
            StatsChainsPaginationLogic::try_from_list_pagination_proto(&lp)
                .map_err(|e| Status::invalid_argument(e.to_string()))?
        };

        let page_size = inner
            .page_size
            .unwrap_or(self.api_settings.default_page_size)
            .clamp(1, self.api_settings.max_page_size) as usize;
        let last_page = inner.last_page.unwrap_or(false);

        let (rows, pagination) = self
            .stats
            .get_stats_chains(chain_ids, order, page_size, last_page, input_pagination)
            .await
            .map_err(map_stats_error)?;

        let use_tok = self.api_settings.use_pagination_token;
        let response = GetChainsStatsResponse {
            items: rows.into_iter().map(stats_chain_row_to_proto).collect(),
            next_page_params: pagination
                .next_marker
                .map(|m| m.to_list_pagination_proto(use_tok)),
            prev_page_params: pagination
                .prev_marker
                .map(|m| m.to_list_pagination_proto(use_tok)),
        };
        Ok(Response::new(response))
    }

    async fn get_sent_message_paths(
        &self,
        request: Request<GetMessagePathsRequest>,
    ) -> Result<Response<GetMessagePathsResponse>, Status> {
        self.message_paths_response(request.into_inner(), true)
            .await
    }

    async fn get_received_message_paths(
        &self,
        request: Request<GetMessagePathsRequest>,
    ) -> Result<Response<GetMessagePathsResponse>, Status> {
        self.message_paths_response(request.into_inner(), false)
            .await
    }
}

impl InterchainStatisticsServiceImpl {
    async fn message_paths_response(
        &self,
        inner: GetMessagePathsRequest,
        outgoing: bool,
    ) -> Result<Response<GetMessagePathsResponse>, Status> {
        let from_date = parse_optional_utc_date(inner.from_date.as_deref())?;
        let to_date = parse_optional_utc_date(inner.to_date.as_deref())?;
        let counterparty_ids = parse_chain_ids_csv(inner.counterparty_chain_ids.as_deref())?;
        let counterparty = (!counterparty_ids.is_empty()).then_some(counterparty_ids.as_slice());

        let rows = if outgoing {
            self.stats
                .interchain_db()
                .get_outgoing_message_paths(inner.chain_id, from_date, to_date, counterparty)
                .await
        } else {
            self.stats
                .interchain_db()
                .get_incoming_message_paths(inner.chain_id, from_date, to_date, counterparty)
                .await
        }
        .map_err(map_stats_error)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let source =
                chain_model_to_proto(self.chain_info.get_chain_info(row.src_chain_id).await);
            let destination =
                chain_model_to_proto(self.chain_info.get_chain_info(row.dst_chain_id).await);
            items.push(MessagePathRow {
                source_chain: Some(source),
                destination_chain: Some(destination),
                messages_count: i64_to_u64_nonneg(row.messages_count),
            });
        }

        Ok(Response::new(GetMessagePathsResponse { items }))
    }
}

fn parse_optional_utc_date(s: Option<&str>) -> Result<Option<NaiveDate>, Status> {
    let Some(s) = s.map(str::trim) else {
        return Ok(None);
    };
    if s.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| {
            Status::invalid_argument(format!(
                "invalid date `{s}`: expected YYYY-MM-DD (UTC calendar date)"
            ))
        })
}

fn parse_chain_ids_csv(input: Option<&str>) -> Result<Vec<i64>, Status> {
    let Some(input) = input.map(str::trim) else {
        return Ok(Vec::new());
    };
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i64>().map_err(|_| {
                Status::invalid_argument(format!(
                    "invalid chain_ids value `{part}`: expected comma-separated int64 ids"
                ))
            })
        })
        .collect()
}

fn stats_chain_row_to_proto(row: StatsChainListRow) -> StatsChainRow {
    const UNKNOWN: &str = "Unknown";
    let name = if row.name.is_empty() {
        UNKNOWN.to_string()
    } else {
        row.name
    };
    let explorer_url = row
        .explorer_url
        .map(|url| url.trim_end_matches('/').to_string());
    StatsChainRow {
        chain_id: row.chain_id,
        name,
        icon_url: row.icon_url,
        explorer_url,
        unique_transfer_users_count: i64_to_u64_nonneg(row.unique_transfer_users_count),
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
                token_address: to_hex_prefixed(t.token_address.as_slice()),
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

#[cfg(test)]
mod tests {
    use super::{parse_chain_ids_csv, parse_optional_utc_date};

    #[test]
    fn parse_chain_ids_csv_accepts_missing_and_empty() {
        assert_eq!(parse_chain_ids_csv(None).unwrap(), Vec::<i64>::new());
        assert_eq!(parse_chain_ids_csv(Some("")).unwrap(), Vec::<i64>::new());
        assert_eq!(parse_chain_ids_csv(Some("   ")).unwrap(), Vec::<i64>::new());
    }

    #[test]
    fn parse_chain_ids_csv_parses_comma_separated_ids() {
        assert_eq!(
            parse_chain_ids_csv(Some("123,456, 789")).unwrap(),
            vec![123, 456, 789]
        );
    }

    #[test]
    fn parse_chain_ids_csv_rejects_invalid_values() {
        let err = parse_chain_ids_csv(Some("123,abc")).expect_err("must reject invalid id");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid chain_ids value `abc`"));
    }

    #[test]
    fn parse_optional_utc_date_accepts_valid_yyyy_mm_dd() {
        assert_eq!(
            parse_optional_utc_date(Some("2026-03-24")).unwrap(),
            Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 24).expect("valid date"))
        );
    }

    #[test]
    fn parse_optional_utc_date_accepts_none_and_blank() {
        assert_eq!(parse_optional_utc_date(None).unwrap(), None);
        assert_eq!(parse_optional_utc_date(Some("")).unwrap(), None);
        assert_eq!(parse_optional_utc_date(Some("   ")).unwrap(), None);
    }

    #[test]
    fn parse_optional_utc_date_rejects_malformed() {
        let err = parse_optional_utc_date(Some("24-03-2026")).expect_err("wrong format must fail");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid date"));
    }
}
