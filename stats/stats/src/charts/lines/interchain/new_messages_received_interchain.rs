// SPDX-License-Identifier: LicenseRef-Blockscout

//! New interchain messages received per day, within the configured interchain
//! slice.
//!
//! Counts messages admitted by the shared read filter whose destination event
//! was indexed (`dst_tx_hash IS NOT NULL`). When
//! `STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is set, "received" additionally
//! means `dst_chain_id = home`; with no home chain configured the chart degrades
//! to "destination-side observed", which the server warns about at startup.
//!
//! The time axis is the message's **initiation** date, not the date its
//! destination event was observed — a pre-existing property, unchanged here,
//! and the reason a "received" point can predate the reception itself.

use std::ops::Range;

use interchain_indexer_entity::crosschain_messages;

use crate::{
    chart_prelude::*,
    charts::db_interaction::filters::interchain::{
        InterchainFilter, InterchainFilterTarget, InterchainFiltered,
    },
};

pub struct NewMessagesReceivedInterchainStatement;
impl_db_choice!(NewMessagesReceivedInterchainStatement, UsePrimaryDB);

impl NewMessagesReceivedInterchainStatement {
    /// Split out from `get_statement_with_context` so tests can render it with an
    /// explicit filter and no `UpdateContext` (hence no database connections).
    fn build(filter: &InterchainFilter, range: Option<Range<DateTime<Utc>>>) -> Statement {
        use crosschain_messages::Column as C;
        const DATE: &str = "date";
        let query = filter
            .messages_query()
            .select_only()
            .expr_as(C::InitTimestamp.into_expr().cast_as(DATE), DATE)
            .expr_as(
                Func::count(Asterisk.into_column_ref()).cast_as("TEXT"),
                "value",
            )
            .filter(C::DstTxHash.is_not_null())
            .apply_if(filter.home_chain_id(), |query, home| {
                query.filter(C::DstChainId.eq(home))
            });
        let query = match &range {
            Some(range) => datetime_range_filter(query, C::InitTimestamp, range),
            None => query,
        };
        query
            .group_by(Expr::col(Alias::new(DATE)))
            .build(DbBackend::Postgres)
    }
}

impl StatementFromRange for NewMessagesReceivedInterchainStatement {
    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        range: Option<Range<DateTime<Utc>>>,
    ) -> Statement {
        Self::build(&cx.interchain_filter, range)
    }
}

impl InterchainFiltered for NewMessagesReceivedInterchainStatement {
    const TARGET: InterchainFilterTarget = InterchainFilterTarget::Messages;
    const CHART_NAME: &'static str = "newMessagesReceivedInterchain";

    fn render(filter: &InterchainFilter) -> Statement {
        Self::build(filter, None)
    }
}

pub type NewMessagesReceivedInterchainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewMessagesReceivedInterchainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newMessagesReceivedInterchain".into()
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

pub type NewMessagesReceivedInterchain =
    DirectVecLocalDbChartSource<NewMessagesReceivedInterchainRemote, Batch30Days, Properties>;
pub type NewMessagesReceivedInterchainInt =
    MapParseTo<StripExt<NewMessagesReceivedInterchain>, i64>;
pub type NewMessagesReceivedInterchainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewMessagesReceivedInterchainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewMessagesReceivedInterchainMonthlyInt =
    MapParseTo<StripExt<NewMessagesReceivedInterchainMonthly>, i64>;
pub type NewMessagesReceivedInterchainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewMessagesReceivedInterchainMonthlyInt, Year>>,
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
    async fn update_new_messages_received_interchain() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchain>(
            "update_new_messages_received_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "1"),
                ("2022-12-23", "1"),
                ("2022-12-27", "2"),
                ("2023-01-01", "1"),
                ("2023-01-02", "1"),
                ("2023-01-04", "1"),
                ("2023-01-11", "1"),
                ("2023-01-21", "2"),
                ("2023-02-01", "1"),
                ("2023-02-05", "1"),
                ("2023-02-06", "1"),
                ("2023-02-08", "1"),
                ("2023-02-09", "1"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<NewMessagesReceivedInterchain>(
            "update_new_messages_received_interchain_primary_1",
            vec![
                ("2022-12-23", "1"),
                ("2022-12-27", "1"),
                ("2023-01-02", "1"),
                ("2023-01-21", "2"),
                ("2023-02-05", "1"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_weekly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainWeekly>(
            "update_new_messages_received_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "3"),
                ("2023-01-02", "2"),
                ("2023-01-09", "1"),
                ("2023-01-16", "2"),
                ("2023-01-30", "2"),
                ("2023-02-06", "3"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_monthly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainMonthly>(
            "update_new_messages_received_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "6"),
                ("2023-02-01", "5"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_messages_received_interchain_yearly() {
        simple_test_chart_interchain::<NewMessagesReceivedInterchainYearly>(
            "update_new_messages_received_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "11")],
            InterchainFilter::default(),
        )
        .await;
    }
}
