// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain transfers sent per day, within the configured interchain
//! slice.
//!
//! Counts transfers admitted by the shared read filter whose parent message's
//! source event was indexed (`crosschain_messages.src_tx_hash IS NOT NULL`).
//!
//! The join to `crosschain_messages` exists only to reach that message's
//! `init_timestamp` (the time axis — a transfer has no timestamp of its own) and
//! `src_tx_hash`. The filter and the directional term both stay on the
//! transfer's own token columns: with
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` set, "sent" means
//! `token_src_chain_id = home`. See
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

pub struct NewTransfersSentInterchainStatement;
impl_db_choice!(NewTransfersSentInterchainStatement, UsePrimaryDB);

impl NewTransfersSentInterchainStatement {
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
            .filter(crosschain_messages::Column::SrcTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(crosschain_transfers::Column::TokenSrcChainId.eq(home))
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

impl StatementFromRange for NewTransfersSentInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewTransfersSentInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Transfers;
    const CHART_NAME: &'static str = "newTransfersSentInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewTransfersSentInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTransfersSentInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTransfersSentInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
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

pub type NewTransfersSentInterchain =
    DirectVecLocalDbChartSource<NewTransfersSentInterchainRemote, Batch30Days, Properties>;
pub type NewTransfersSentInterchainInt = MapParseTo<StripExt<NewTransfersSentInterchain>, i64>;
pub type NewTransfersSentInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersSentInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTransfersSentInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersSentInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTransfersSentInterchainMonthlyInt =
    MapParseTo<StripExt<NewTransfersSentInterchainMonthly>, i64>;
pub type NewTransfersSentInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTransfersSentInterchainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        mock_interchain::test_interchain_home_chain_filter,
        simple_test::simple_test_chart_interchain,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_sent_interchain() {
        simple_test_chart_interchain::<NewTransfersSentInterchain>(
            "update_new_transfers_sent_interchain",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-21", "3"),
                ("2022-12-26", "5"),
                ("2022-12-27", "4"),
                ("2023-01-01", "3"),
                ("2023-01-04", "3"),
                ("2023-01-10", "6"),
                ("2023-01-20", "1"),
                ("2023-01-21", "3"),
                ("2023-02-01", "7"),
                ("2023-02-06", "3"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;

        // 2023-02-09 is absent here and only here: message 24's transfer has
        // `token_src_chain_id = 3` on a `1 → 2` route, so the directional term on
        // the transfer's own column excludes it where the old
        // `m.src_chain_id = 1` shape would have kept it.
        simple_test_chart_interchain::<NewTransfersSentInterchain>(
            "update_new_transfers_sent_interchain_primary_1",
            vec![
                ("2022-12-20", "2"),
                ("2022-12-26", "5"),
                ("2023-01-01", "3"),
                ("2023-01-04", "3"),
                ("2023-01-10", "2"),
                ("2023-01-20", "1"),
                ("2023-02-01", "7"),
                ("2023-02-06", "2"),
                ("2023-02-07", "1"),
                ("2023-02-08", "1"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_sent_interchain_weekly() {
        simple_test_chart_interchain::<NewTransfersSentInterchainWeekly>(
            "update_new_transfers_sent_interchain_weekly",
            vec![
                ("2022-12-19", "5"),
                ("2022-12-26", "12"),
                ("2023-01-02", "3"),
                ("2023-01-09", "6"),
                ("2023-01-16", "4"),
                ("2023-01-30", "7"),
                ("2023-02-06", "6"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_sent_interchain_monthly() {
        simple_test_chart_interchain::<NewTransfersSentInterchainMonthly>(
            "update_new_transfers_sent_interchain_monthly",
            vec![
                ("2022-12-01", "14"),
                ("2023-01-01", "16"),
                ("2023-02-01", "13"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_transfers_sent_interchain_yearly() {
        simple_test_chart_interchain::<NewTransfersSentInterchainYearly>(
            "update_new_transfers_sent_interchain_yearly",
            vec![("2022-01-01", "14"), ("2023-01-01", "29")],
            InterchainFilter::default(),
        )
        .await;
    }
}
