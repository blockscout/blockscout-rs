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
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};

    use super::*;
    use crate::{
        charts::db_interaction::{filters::interchain::InterchainFilter, read::find_chart},
        tests::{
            mock_interchain::{
                fill_mock_interchain_messages_in_range, fill_mock_interchain_reference_data,
                test_interchain_home_chain_filter,
            },
            point_construction::d,
            simple_test::{
                map_str_tuple_to_owned, prepare_interchain_chart_test,
                prepare_interchain_chart_test_unfilled, simple_test_chart_interchain,
                update_and_query_interchain_chart, update_and_query_interchain_chart_with_force,
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

    /// The test that would have caught the original bug. The cumulative family
    /// is the right subject: a missing prefix offsets every later point, so any
    /// mistake in trigger 2 shows up in the whole tail, not just the backfilled
    /// dates.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn messages_growth_sent_interchain_picks_up_backwards_backfill() {
        let cutoff = d("2023-01-01");

        // The reference: a from-scratch, force_full run over the complete
        // fixture, on an independent DB pair.
        let (ref_time, ref_db, ref_indexer) = prepare_interchain_chart_test::<
            MessagesGrowthSentInterchain,
        >("messages_growth_sent_backfill_reference")
        .await;
        let reference =
            update_and_query_interchain_chart_with_force::<MessagesGrowthSentInterchain>(
                &ref_db,
                &ref_indexer,
                InterchainFilter::default(),
                ref_time,
                true,
            )
            .await;

        // The main pair: only the tail of the fixture at first.
        let (t1, db, indexer) = prepare_interchain_chart_test_unfilled::<
            MessagesGrowthSentInterchain,
        >("messages_growth_sent_backfill_main")
        .await;
        fill_mock_interchain_reference_data(&indexer).await;
        fill_mock_interchain_messages_in_range(&indexer, cutoff..).await;
        let tail_only = update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            t1,
        )
        .await;
        // sanity: the tail-only run must actually differ from the full fixture,
        // otherwise the test below would pass regardless of trigger 2
        assert_ne!(tail_only, reference);

        // The indexer backfills everything before the cutoff.
        fill_mock_interchain_messages_in_range(&indexer, ..cutoff).await;
        let t2 = t1 + TimeDelta::seconds(1);
        let backfilled = update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            t2,
        )
        .await;

        assert_eq!(backfilled, reference);
    }

    /// The "must not clear" requirement, asserted positively: a sentinel row on
    /// a genuine hole date (one the recompute writes nothing for) must survive
    /// a backfill-triggered rebuild. Had the trigger gone through
    /// `clear_chart_data_and_updated_at` instead of a from-the-floor recompute,
    /// the sentinel would be gone.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn interchain_backfill_rebuild_does_not_clear_stored_rows() {
        let cutoff = d("2023-01-01");
        // a documented hole in the fixture (see `mock_interchain`'s
        // `routed("2023-01-04T10:00:00", 11, ...)` comment: "hole 3rd") that
        // falls inside the eventual full range, so the phase-2 recompute passes
        // over it but writes nothing for it
        let hole_date = d("2023-01-03");

        let (t1, db, indexer) = prepare_interchain_chart_test_unfilled::<
            MessagesGrowthSentInterchain,
        >("interchain_backfill_no_clear")
        .await;
        fill_mock_interchain_reference_data(&indexer).await;
        fill_mock_interchain_messages_in_range(&indexer, cutoff..).await;
        update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            t1,
        )
        .await;

        let chart_id = find_chart(&db, &Properties::key())
            .await
            .unwrap()
            .expect("chart must exist after the first update");
        let sentinel = chart_data::ActiveModel {
            chart_id: Set(chart_id),
            date: Set(hole_date),
            value: Set("999999".to_owned()),
            min_blockscout_block: Set(Some(InterchainFilter::default().fingerprint)),
            ..Default::default()
        };
        chart_data::Entity::insert(sentinel)
            .exec(&*db)
            .await
            .unwrap();

        // the indexer backfills everything before the cutoff, triggering trigger
        // 2's from-the-floor recompute
        fill_mock_interchain_messages_in_range(&indexer, ..cutoff).await;
        let t2 = t1 + TimeDelta::seconds(1);
        update_and_query_interchain_chart::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            t2,
        )
        .await;

        let sentinel_row = chart_data::Entity::find()
            .filter(chart_data::Column::ChartId.eq(chart_id))
            .filter(chart_data::Column::Date.eq(hole_date))
            .one(&*db)
            .await
            .unwrap();
        assert_eq!(
            sentinel_row.map(|row| row.value),
            Some("999999".to_owned()),
            "the sentinel on the hole date must survive a backfill-triggered rebuild"
        );
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
