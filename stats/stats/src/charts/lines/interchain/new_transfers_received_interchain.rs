// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain transfers received per day, within the configured interchain
//! slice.
//!
//! Counts transfers admitted by the shared read filter whose parent message's
//! destination event was indexed (`crosschain_messages.dst_tx_hash IS NOT NULL`).
//!
//! The join to `crosschain_messages` exists only to reach that message's
//! `init_timestamp` (the time axis — a transfer has no timestamp of its own) and
//! `dst_tx_hash`. The filter and the directional term both stay on the
//! transfer's own token columns: with
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` set, "received" means
//! `token_dst_chain_id = home`. See
//! [`crate::counters::TotalInterchainTransfersSent`]'s module docs for why.
//! The join is composite (`(message_id, bridge_id)`) via the declared SeaORM
//! relation.

use std::ops::Range;

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct NewTransfersReceivedInterchainStatement;
impl_db_choice!(NewTransfersReceivedInterchainStatement, UsePrimaryDB);

impl NewTransfersReceivedInterchainStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        const DATE: &str = "date";
        let time_axis = crosschain_messages::Column::InitTimestamp;
        let query = filter
            .transfers_joined_query()
            .select_only()
            .expr_as(time_axis.into_expr().cast_as(DATE), DATE)
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            )
            .filter(crosschain_messages::Column::DstTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(crosschain_transfers::Column::TokenDstChainId.eq(home))
            });
        let query = match &range {
            Some(range) => datetime_range_filter(query, time_axis, range),
            None => query,
        };
        query
            .group_by(Expr::col(Alias::new(DATE)))
            .build(DbBackend::Postgres)
    }
}

impl StatementFromRange for NewTransfersReceivedInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewTransfersReceivedInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "newTransfersReceivedInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewTransfersReceivedInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTransfersReceivedInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTransfersReceivedInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp)
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewTransfersReceivedInterchain =
    DirectVecLocalDbChartSource<NewTransfersReceivedInterchainRemote, Batch30Days, Properties>;
pub type NewTransfersReceivedInterchainInt =
    MapParseTo<StripExt<NewTransfersReceivedInterchain>, i64>;
pub type NewTransfersReceivedInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTransfersReceivedInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTransfersReceivedInterchainMonthlyInt =
    MapParseTo<StripExt<NewTransfersReceivedInterchainMonthly>, i64>;
pub type NewTransfersReceivedInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersReceivedInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use interchain_indexer_filters::ChainBridgeFilter;

    use super::*;
    use crate::tests::{
        mock_interchain::{
            mock_interchain_horizon, test_interchain_filter_with_horizon,
            test_interchain_home_chain_filter,
        },
        simple_test::simple_test_chart_interchain,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchain>(
            "update_new_transfers_received_interchain",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-23", "1"),
                ("2022-12-27", "5"),
                ("2023-01-01", "2"),
                ("2023-01-04", "3"),
                ("2023-01-11", "1"),
                ("2023-01-21", "3"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
                ("2023-02-06", "2"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewTransfersReceivedInterchain>(
            "update_new_transfers_received_interchain_primary_1",
            vec![
                ("2022-12-23", "1"),
                ("2022-12-27", "1"),
                ("2023-01-21", "3"),
                ("2023-02-05", "1"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    /// The horizon drops message 23's transfer (`1 → 4`), message 24's (`3 → 4`)
    /// and all three bridge-2 transfers; message 22's survives, since
    /// `transfers_condition` carries no NULL guard.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn new_transfers_received_interchain_horizon() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchain>(
            "new_transfers_received_horizon",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-23", "1"),
                ("2022-12-27", "5"),
                ("2023-01-01", "2"),
                ("2023-01-04", "3"),
                ("2023-01-11", "1"),
                ("2023-01-21", "3"),
                ("2023-02-01", "2"),
                ("2023-02-05", "1"),
            ],
            test_interchain_filter_with_horizon(
                ChainBridgeFilter::default(),
                Some(mock_interchain_horizon()),
            ),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_weekly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainWeekly>(
            "update_new_transfers_received_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "7"),
                ("2023-01-02", "3"),
                ("2023-01-09", "1"),
                ("2023-01-16", "3"),
                ("2023-01-30", "3"),
                ("2023-02-06", "4"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_monthly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainMonthly>(
            "update_new_transfers_received_interchain_monthly",
            vec![
                ("2022-12-01", "8"),
                ("2023-01-01", "9"),
                ("2023-02-01", "7"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_received_interchain_yearly() {
        simple_test_chart_interchain::<NewTransfersReceivedInterchainYearly>(
            "update_new_transfers_received_interchain_yearly",
            vec![("2022-01-01", "8"), ("2023-01-01", "16")],
            InterchainFilter::default(),
        )
        .await;
    }
}
