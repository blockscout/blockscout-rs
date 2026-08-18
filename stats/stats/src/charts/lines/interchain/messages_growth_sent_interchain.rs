// SPDX-License-Identifier: LicenseRef-Blockscout

//! Cumulative interchain messages sent, within the configured interchain slice.
//!
//! Has no SQL of its own: it is the running total of
//! [`super::new_messages_sent_interchain`]'s stored rows, and therefore inherits
//! that chart's read filter transitively. `interchain_filter_coverage` lists it
//! in `DERIVED_WITHOUT_OWN_STATEMENT` on exactly that basis.

use super::new_messages_sent_interchain::NewMessagesSentInterchainInt;
use crate::chart_prelude::*;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "messagesGrowthSentInterchain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
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

pub type MessagesGrowthSentInterchain =
    DailyCumulativeLocalDbChartSource<NewMessagesSentInterchainInt, Properties>;
type MessagesGrowthSentInterchainS = StripExt<MessagesGrowthSentInterchain>;

pub type MessagesGrowthSentInterchainWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type MessagesGrowthSentInterchainMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type MessagesGrowthSentInterchainMonthlyS = StripExt<MessagesGrowthSentInterchainMonthly>;
pub type MessagesGrowthSentInterchainYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<MessagesGrowthSentInterchainMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        charts::db_interaction::filters::interchain::InterchainFilter,
        tests::{
            mock_interchain::test_interchain_home_chain_filter,
            simple_test::{
                map_str_tuple_to_owned, prepare_interchain_chart_test,
                simple_test_chart_interchain, update_and_query_interchain_chart,
            },
        },
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchain>(
            "update_messages_growth_sent_interchain",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "3"),
                ("2022-12-26", "4"),
                ("2022-12-27", "5"),
                ("2023-01-01", "7"),
                ("2023-01-04", "8"),
                ("2023-01-10", "10"),
                ("2023-01-20", "11"),
                ("2023-01-21", "12"),
                ("2023-02-01", "14"),
                ("2023-02-06", "16"),
                ("2023-02-07", "17"),
                ("2023-02-08", "18"),
                ("2023-02-09", "19"),
                ("2023-02-10", "20"),
            ],
            InterchainFilter::default(),
        )
        .await;

        simple_test_chart_interchain::<MessagesGrowthSentInterchain>(
            "update_messages_growth_sent_interchain_primary_1",
            vec![
                ("2022-12-20", "1"),
                ("2022-12-21", "2"),
                ("2022-12-26", "3"),
                ("2023-01-01", "5"),
                ("2023-01-04", "6"),
                ("2023-01-10", "7"),
                ("2023-01-20", "8"),
                ("2023-02-01", "10"),
                ("2023-02-06", "11"),
                ("2023-02-07", "12"),
                ("2023-02-08", "13"),
                ("2023-02-09", "14"),
                ("2023-02-10", "15"),
            ],
            test_interchain_home_chain_filter(1),
        )
        .await;
    }

    /// Changing the interchain filter must invalidate the stored history rather
    /// than merge two filter regimes into one series.
    ///
    /// This chart is the exposed case, on both counts the mechanism exists for:
    /// it is a [`DailyCumulativeLocalDbChartSource`], so a stale prefix
    /// propagates through every later point, and `insert_data_many` is an upsert
    /// with no delete — a *narrowed* filter simply writes nothing on days that
    /// used to have rows, leaving the old values in place.
    ///
    /// The narrowing is `home_chain_id = 2`, chosen because it drops **both**
    /// ends of the wide series: its first day (2022-12-20) and its whole tail
    /// (2023-02-07..10). Without the clear-on-fingerprint-change those days
    /// survive with their wide values, so the comparison against a from-scratch
    /// computation fails at the head *and* at the tail.
    ///
    /// Step 3 (widening back) proves the clear is symmetric: a widening is not
    /// silently limited by the narrowed history.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn messages_growth_sent_interchain_filter_change_clears_stale_data() {
        let narrow = test_interchain_home_chain_filter(2);
        let wide_expected = map_str_tuple_to_owned(vec![
            ("2022-12-20", "1"),
            ("2022-12-21", "3"),
            ("2022-12-26", "4"),
            ("2022-12-27", "5"),
            ("2023-01-01", "7"),
            ("2023-01-04", "8"),
            ("2023-01-10", "10"),
            ("2023-01-20", "11"),
            ("2023-01-21", "12"),
            ("2023-02-01", "14"),
            ("2023-02-06", "16"),
            ("2023-02-07", "17"),
            ("2023-02-08", "18"),
            ("2023-02-09", "19"),
            ("2023-02-10", "20"),
        ]);

        // The reference: the same narrow filter, on databases that never saw the
        // wide one. Asserting against this rather than only against a literal is
        // the point — "what the series would be if it had never been computed
        // under another filter" is exactly the property under test.
        let (reference_time, reference_db, reference_indexer) =
            prepare_interchain_chart_test::<MessagesGrowthSentInterchain>(
                "growth_sent_interchain_filter_change_reference",
            )
            .await;
        let narrow_from_scratch =
            update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
                &reference_db,
                &reference_indexer,
                narrow.clone(),
                reference_time,
            )
            .await;
        assert_eq!(
            narrow_from_scratch,
            map_str_tuple_to_owned(vec![
                ("2022-12-21", "1"),
                ("2022-12-27", "2"),
                ("2023-01-10", "3"),
                ("2023-01-21", "4"),
                ("2023-02-06", "5"),
            ])
        );

        let (init_time, db, indexer) =
            prepare_interchain_chart_test::<MessagesGrowthSentInterchain>(
                "growth_sent_interchain_filter_change",
            )
            .await;

        // 1. wide
        let wide = update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            init_time,
        )
        .await;
        assert_eq!(wide, wide_expected);

        // 2. narrow, at a later update time so the "already handled within
        //    ongoing update" short-circuit does not skip the update entirely
        let narrowed = update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            narrow,
            init_time + TimeDelta::seconds(1),
        )
        .await;
        assert_eq!(narrowed, narrow_from_scratch);

        // 3. wide again
        let widened = update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            init_time + TimeDelta::seconds(2),
        )
        .await;
        assert_eq!(widened, wide_expected);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_weekly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainWeekly>(
            "update_messages_growth_sent_interchain_weekly",
            vec![
                ("2022-12-19", "3"),
                ("2022-12-26", "7"),
                ("2023-01-02", "8"),
                ("2023-01-09", "10"),
                ("2023-01-16", "12"),
                ("2023-01-30", "14"),
                ("2023-02-06", "20"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_monthly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainMonthly>(
            "update_messages_growth_sent_interchain_monthly",
            vec![
                ("2022-12-01", "5"),
                ("2023-01-01", "12"),
                ("2023-02-01", "20"),
            ],
            InterchainFilter::default(),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_messages_growth_sent_interchain_yearly() {
        simple_test_chart_interchain::<MessagesGrowthSentInterchainYearly>(
            "update_messages_growth_sent_interchain_yearly",
            vec![("2022-01-01", "5"), ("2023-01-01", "20")],
            InterchainFilter::default(),
        )
        .await;
    }
}
