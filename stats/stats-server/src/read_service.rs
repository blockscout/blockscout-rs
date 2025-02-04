use std::{clone::Clone, collections::BTreeMap, fmt::Debug, str::FromStr, sync::Arc};

use crate::{
    config::{
        layout::placed_items_according_to_layout,
        types::{self, EnabledChartSettings},
    },
    runtime_setup::{EnabledChartEntry, RuntimeSetup},
    settings::LimitsSettings,
};

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use futures::{stream::FuturesOrdered, StreamExt};
use proto_v1::stats_service_server::StatsService;
use sea_orm::{DatabaseConnection, DbErr};
use stats::{
    counters::{
        AverageBlockTime, AverageTxnFee24h, NewContracts24h, NewOperationalTxns24h, NewTxns24h,
        NewVerifiedContracts24h, PendingTxns30m, TotalAddresses, TotalBlocks, TotalContracts,
        TotalOperationalTxns, TotalTxns, TotalVerifiedContracts, TxnsFee24h,
        YesterdayOperationalTxns, YesterdayTxns,
    },
    data_source::{types::BlockscoutMigrations, UpdateContext, UpdateParameters},
    lines::{NewOperationalTxnsWindow, NewTxnsWindow, NEW_TXNS_WINDOW_RANGE},
    query_dispatch::{CounterHandle, LineHandle, QuerySerializedDyn},
    range::UniversalRange,
    types::{Timespan, TimespanDuration},
    utils::day_start,
    ChartError, Named, RequestedPointsLimit, ResolutionKind,
};
use stats_proto::blockscout::stats::v1 as proto_v1;
use tokio::join;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ReadService {
    db: Arc<DatabaseConnection>,
    blockscout: Arc<DatabaseConnection>,
    charts: Arc<RuntimeSetup>,
    limits: ReadLimits,
}

impl ReadService {
    pub async fn new(
        db: Arc<DatabaseConnection>,
        blockscout: Arc<DatabaseConnection>,
        charts: Arc<RuntimeSetup>,
        limits: ReadLimits,
    ) -> Result<Self, DbErr> {
        Ok(Self {
            db,
            blockscout,
            charts,
            limits,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadLimits {
    /// See [`LimitsSettings::requested_points_limit`]
    pub requested_points_limit: RequestedPointsLimit,
}

impl From<LimitsSettings> for ReadLimits {
    fn from(value: LimitsSettings) -> Self {
        Self {
            requested_points_limit: RequestedPointsLimit::from_points(value.requested_points_limit),
        }
    }
}

fn map_update_error(err: ChartError) -> Status {
    match &err {
        ChartError::ChartNotFound(_) => Status::not_found(err.to_string()),
        ChartError::IntervalTooLarge { limit: _ } => Status::invalid_argument(err.to_string()),
        _ => {
            tracing::error!(err = ?err, "internal read error");
            Status::internal(err.to_string())
        }
    }
}

fn inclusive_date_range_to_query_range(
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> UniversalRange<DateTime<Utc>> {
    let from = from.map(|d| day_start(&d));
    let to_exclusive = to.map(|d| day_start(&d.saturating_next_timespan()));
    (from..to_exclusive).into()
}

/// Add chart information to each chart id in layout
///
/// Returns `None` if info were not found for some chart.
fn add_chart_info_to_layout(
    layout: Vec<types::LineChartCategory>,
    chart_info: &BTreeMap<String, EnabledChartEntry>,
) -> Vec<proto_v1::LineChartSection> {
    layout
        .into_iter()
        .map(|cat| cat.intersect_info(chart_info))
        .collect()
}

fn convert_resolution(input: proto_v1::Resolution) -> ResolutionKind {
    match input {
        proto_v1::Resolution::Unspecified | proto_v1::Resolution::Day => ResolutionKind::Day,
        proto_v1::Resolution::Week => ResolutionKind::Week,
        proto_v1::Resolution::Month => ResolutionKind::Month,
        proto_v1::Resolution::Year => ResolutionKind::Year,
    }
}

fn get_line_chart_query_handle(
    line_chart: &EnabledChartEntry,
    resolution: ResolutionKind,
) -> Option<LineHandle> {
    let enabled_resolution = line_chart.resolutions.get(&resolution)?;
    enabled_resolution.type_specifics.clone().into_line_handle()
}

fn get_counter_query_handle(name: &str, counter: &EnabledChartEntry) -> Option<CounterHandle> {
    // resolutions other than day are currently not supported
    // for counters
    let Some(enabled_resolution) = counter.resolutions.get(&ResolutionKind::Day) else {
        tracing::warn!(
            "No 'day' resolution enabled for counter {}, skipping its value",
            name
        );
        return None;
    };
    enabled_resolution
        .type_specifics
        .clone()
        .into_counter_handle()
}

impl ReadService {
    pub fn main_page_charts() -> Vec<String> {
        // ensure that changes to api are reflected here
        #[allow(clippy::no_effect)]
        proto_v1::MainPageStats {
            average_block_time: None,
            total_addresses: None,
            total_blocks: None,
            total_transactions: None,
            yesterday_transactions: None,
            total_operational_transactions: None,
            yesterday_operational_transactions: None,
            daily_new_transactions: None,
            daily_new_operational_transactions: None,
        };
        vec![
            AverageBlockTime::name(),
            TotalAddresses::name(),
            TotalBlocks::name(),
            TotalTxns::name(),
            YesterdayTxns::name(),
            TotalOperationalTxns::name(),
            YesterdayOperationalTxns::name(),
            NewTxnsWindow::name(),
            NewOperationalTxnsWindow::name(),
        ]
    }

    pub fn contracts_page_charts() -> Vec<String> {
        // ensure that changes to api are reflected here
        #[allow(clippy::no_effect)]
        proto_v1::ContractsPageStats {
            total_contracts: None,
            new_contracts_24h: None,
            total_verified_contracts: None,
            new_verified_contracts_24h: None,
        };
        vec![
            TotalContracts::name(),
            NewContracts24h::name(),
            TotalVerifiedContracts::name(),
            NewVerifiedContracts24h::name(),
        ]
    }

    pub fn transactions_page_charts() -> Vec<String> {
        // ensure that changes to api are reflected here
        #[allow(clippy::no_effect)]
        proto_v1::TransactionsPageStats {
            pending_transactions_30m: None,
            transactions_fee_24h: None,
            average_transactions_fee_24h: None,
            transactions_24h: None,
            operational_transactions_24h: None,
        };
        vec![
            PendingTxns30m::name(),
            TxnsFee24h::name(),
            AverageTxnFee24h::name(),
            NewTxns24h::name(),
            NewOperationalTxns24h::name(),
        ]
    }
}

impl ReadService {
    async fn query_with_handle<Data: Send>(
        &self,
        query_handle: QuerySerializedDyn<Data>,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        query_time: DateTime<Utc>,
    ) -> Result<Data, ChartError> {
        let migrations = BlockscoutMigrations::query_from_db(&self.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
        let context = UpdateContext::from_params_now_or_override(UpdateParameters {
            db: &self.db,
            blockscout: &self.blockscout,
            blockscout_applied_migrations: migrations,
            update_time_override: Some(query_time),
            force_full: false,
        });
        query_handle
            .query_data(&context, range, points_limit, true)
            .await
    }

    async fn query_counter_with_handle(
        &self,
        name: String,
        settings: EnabledChartSettings,
        query_handle: CounterHandle,
        query_time: DateTime<Utc>,
    ) -> Result<proto_v1::Counter, ChartError> {
        let point = self
            .query_with_handle(query_handle, UniversalRange::full(), None, query_time)
            .await?;
        Ok(proto_v1::Counter {
            id: name,
            value: point.value,
            title: settings.title,
            description: settings.description,
            units: settings.units,
        })
    }

    async fn query_line_chart_with_handle(
        &self,
        name: String,
        chart_entry: &EnabledChartEntry,
        query_handle: LineHandle,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        query_time: DateTime<Utc>,
    ) -> Result<proto_v1::LineChart, ChartError> {
        let data = self
            .query_with_handle(query_handle, range, points_limit, query_time)
            .await?;
        Ok(proto_v1::LineChart {
            chart: data,
            info: Some(chart_entry.build_proto_line_chart_info(name.to_string())),
        })
    }

    /// Logs errors, returning `None`
    async fn query_counter_with_entry(
        &self,
        name: String,
        chart_entry: &EnabledChartEntry,
        query_time: DateTime<Utc>,
    ) -> Option<proto_v1::Counter> {
        let query_handle = get_counter_query_handle(&name, chart_entry)?;
        let counter = self
            .query_counter_with_handle(name, chart_entry.settings.clone(), query_handle, query_time)
            .await
            .inspect_err(|e| tracing::error!("Failed to query counter: {:?}", e))
            .ok()?;
        Some(counter)
    }

    async fn query_counter(
        &self,
        name: String,
        query_time: DateTime<Utc>,
    ) -> Option<proto_v1::Counter> {
        let chart_entry = self.charts.charts_info.get(&name)?;
        self.query_counter_with_entry(name, chart_entry, query_time)
            .await
    }

    async fn query_line_chart(
        &self,
        name: String,
        resolution: ResolutionKind,
        range: UniversalRange<DateTime<Utc>>,
        points_limit: Option<RequestedPointsLimit>,
        query_time: DateTime<Utc>,
    ) -> Result<proto_v1::LineChart, Status> {
        let chart_entry = self.charts.charts_info.get(&name).ok_or_else(|| {
            Status::not_found(format!("chart with name '{}' was not found", name))
        })?;
        let query_handle =
            get_line_chart_query_handle(chart_entry, resolution).ok_or_else(|| {
                Status::not_found(format!(
                    "resolution '{}' for line chart '{}' was not found",
                    String::from(resolution),
                    &name,
                ))
            })?;

        let chart_data = self
            .query_line_chart_with_handle(
                name,
                chart_entry,
                query_handle,
                range,
                points_limit,
                query_time,
            )
            .await
            .map_err(map_update_error)?;
        Ok(chart_data)
    }

    async fn query_window_chart(
        &self,
        name: String,
        window_range: u64,
        query_time: DateTime<Utc>,
    ) -> Option<proto_v1::LineChart> {
        // `query_line_chart` will result in warn here even when querying a disabled chart.
        if !self.charts.charts_info.contains_key(&name) {
            return None;
        }

        // All `window_range` should be returned,
        // therefore we need to set exact query range to fill
        // zeroes (if any)

        let query_day = query_time.date_naive();
        // overshoot by two to account for
        // - last point being approximate
        // - chart last updated yesterday
        let range_start = query_day.saturating_sub(TimespanDuration::from_days(window_range + 1));
        let request_range = inclusive_date_range_to_query_range(Some(range_start), Some(query_day));
        let mut transactions = self
            .query_line_chart(
                name.clone(),
                ResolutionKind::Day,
                request_range,
                None,
                query_time,
            )
            .await
            .inspect_err(|e| tracing::warn!("Couldn't get {} for the main page: {}", name, e))
            .ok()?;
        // return exactly `NEW_TXNS_WINDOW_RANGE` accurate points
        let data = transactions
            .chart
            .into_iter()
            // 1 should be filtered
            .filter(|p| !p.is_approximate);
        // take last `NEW_TXNS_WINDOW_RANGE` to ensure the resulting number
        let mut data_reversed: Vec<_> = data.rev().take(NEW_TXNS_WINDOW_RANGE as usize).collect();
        data_reversed.reverse();
        transactions.chart = data_reversed;
        Some(transactions)
    }
}

#[async_trait]
impl StatsService for ReadService {
    async fn get_counters(
        &self,
        _request: Request<proto_v1::GetCountersRequest>,
    ) -> Result<Response<proto_v1::Counters>, Status> {
        let now = Utc::now();
        let counters_futures: FuturesOrdered<_> = self
            .charts
            .charts_info
            .iter()
            .map(|(name, counter)| self.query_counter_with_entry(name.to_string(), counter, now))
            .collect();
        let counters: Vec<proto_v1::Counter> = counters_futures
            .filter_map(|result| async move { result })
            .collect()
            .await;
        let counters_sorted =
            placed_items_according_to_layout(counters, &self.charts.counters_layout, |c| &c.id);
        let counters = proto_v1::Counters {
            counters: counters_sorted,
        };
        Ok(Response::new(counters))
    }

    async fn get_line_chart(
        &self,
        request: Request<proto_v1::GetLineChartRequest>,
    ) -> Result<Response<proto_v1::LineChart>, Status> {
        let request = request.into_inner();
        let resolution = convert_resolution(request.resolution());
        let chart_name = request.name;

        let request_range = inclusive_date_range_to_query_range(
            request
                .from
                .and_then(|date| NaiveDate::from_str(&date).ok()),
            request.to.and_then(|date| NaiveDate::from_str(&date).ok()),
        );
        let points_limit = Some(self.limits.requested_points_limit);

        let chart_data = self
            .query_line_chart(
                chart_name,
                resolution,
                request_range,
                points_limit,
                Utc::now(),
            )
            .await?;

        Ok(Response::new(chart_data))
    }

    async fn get_line_charts(
        &self,
        _request: Request<proto_v1::GetLineChartsRequest>,
    ) -> Result<Response<proto_v1::LineCharts>, Status> {
        let layout = self.charts.lines_layout.clone();
        let sections = add_chart_info_to_layout(layout, &self.charts.charts_info);

        Ok(Response::new(proto_v1::LineCharts { sections }))
    }

    async fn get_main_page_stats(
        &self,
        _request: Request<proto_v1::GetMainPageStatsRequest>,
    ) -> Result<Response<proto_v1::MainPageStats>, Status> {
        let now = Utc::now();

        let (
            average_block_time,
            total_addresses,
            total_blocks,
            total_transactions,
            yesterday_transactions,
            total_operational_transactions,
            yesterday_operational_transactions,
            daily_new_transactions,
            daily_new_operational_transactions,
        ) = join!(
            self.query_counter(AverageBlockTime::name(), now),
            self.query_counter(TotalAddresses::name(), now),
            self.query_counter(TotalBlocks::name(), now),
            self.query_counter(TotalTxns::name(), now),
            self.query_counter(YesterdayTxns::name(), now),
            self.query_counter(TotalOperationalTxns::name(), now),
            self.query_counter(YesterdayOperationalTxns::name(), now),
            self.query_window_chart(NewTxnsWindow::name(), NEW_TXNS_WINDOW_RANGE, now),
            self.query_window_chart(NewOperationalTxnsWindow::name(), NEW_TXNS_WINDOW_RANGE, now),
        );

        Ok(Response::new(proto_v1::MainPageStats {
            average_block_time,
            total_addresses,
            total_blocks,
            total_transactions,
            yesterday_transactions,
            total_operational_transactions,
            yesterday_operational_transactions,
            daily_new_transactions,
            daily_new_operational_transactions,
        }))
    }

    async fn get_transactions_page_stats(
        &self,
        _request: Request<proto_v1::GetTransactionsPageStatsRequest>,
    ) -> Result<Response<proto_v1::TransactionsPageStats>, Status> {
        let now = Utc::now();
        let (
            pending_transactions_30m,
            transactions_fee_24h,
            average_transactions_fee_24h,
            transactions_24h,
            operational_transactions_24h,
        ) = join!(
            self.query_counter(PendingTxns30m::name(), now),
            self.query_counter(TxnsFee24h::name(), now),
            self.query_counter(AverageTxnFee24h::name(), now),
            self.query_counter(NewTxns24h::name(), now),
            self.query_counter(NewOperationalTxns24h::name(), now),
        );
        Ok(Response::new(proto_v1::TransactionsPageStats {
            pending_transactions_30m,
            transactions_fee_24h,
            average_transactions_fee_24h,
            transactions_24h,
            operational_transactions_24h,
        }))
    }

    async fn get_contracts_page_stats(
        &self,
        _request: Request<proto_v1::GetContractsPageStatsRequest>,
    ) -> Result<Response<proto_v1::ContractsPageStats>, Status> {
        let now = Utc::now();
        let (
            total_contracts,
            new_contracts_24h,
            total_verified_contracts,
            new_verified_contracts_24h,
        ) = join!(
            self.query_counter(TotalContracts::name(), now),
            self.query_counter(NewContracts24h::name(), now),
            self.query_counter(TotalVerifiedContracts::name(), now),
            self.query_counter(NewVerifiedContracts24h::name(), now),
        );
        Ok(Response::new(proto_v1::ContractsPageStats {
            total_contracts,
            new_contracts_24h,
            total_verified_contracts,
            new_verified_contracts_24h,
        }))
    }
}
