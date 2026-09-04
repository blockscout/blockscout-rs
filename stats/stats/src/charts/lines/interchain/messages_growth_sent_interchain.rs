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
    use std::fmt::Debug;

    use chrono::TimeDelta;
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
    use stats_proto::blockscout::stats::v1::Point;

    use super::*;
    use crate::{
        charts::db_interaction::{filters::interchain::InterchainFilter, read::find_chart},
        data_source::source::DataSource,
        query_dispatch::QuerySerialized,
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

    /// The lower-resolution analogue of
    /// `messages_growth_sent_interchain_picks_up_backwards_backfill`, and the
    /// test that catches trigger 2's blind spot: both sides of
    /// `interchain_history_gap_has_data`'s comparison go through
    /// `ChartProps::Resolution::from_date`, and a resolution's stored `date` is
    /// already its bucket's first day, so once a resolution chart's own
    /// comparison has fired once, a *further* floor movement that stays inside
    /// the same bucket is invisible to it. Three stages are used specifically
    /// so that every one of weekly/monthly/yearly hits that blind spot at some
    /// stage transition (verified against `Week::WEEK_START = Weekday::Mon`
    /// putting `2022-12-26` and `2023-01-01` in the same ISO week):
    ///
    /// - stage 1 → stage 2 (floor `2023-01-01` → `2022-12-26`): the **week**
    ///   bucket does not change (both fall in the week starting `2022-12-26`) —
    ///   weekly's own comparison is blind here, so this is the case that fails
    ///   without the propagation fix. Month and year *do* cross a bucket here
    ///   (`2023-01` → `2022-12`, `2023` → `2022`), so their own comparison alone
    ///   is (coincidentally) enough at this stage.
    /// - stage 2 → stage 3 (floor `2022-12-26` → `2022-12-20`): the **month**
    ///   and **year** buckets do not change (both `2022-12`, both `2022`) —
    ///   their own comparison is blind here. Week *does* cross a bucket
    ///   (`2022-12-26..` → `2022-12-19..`), so it is not exercising its blind
    ///   spot again at this stage — stage 1→2 already proved it.
    ///
    /// Each resolution is driven through its own independent DB pair via a
    /// single top-level `update_and_query_interchain_chart` call per stage, so
    /// that chart and its `Day`-resolution dependency always share one
    /// `UpdateContext`/cache per call — exactly the ordering the propagation
    /// fix relies on, and exactly how one shared update-group cycle would
    /// drive them in production.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn messages_growth_sent_interchain_picks_up_backwards_backfill_at_lower_resolutions() {
        async fn assert_converges_through_staged_backfill<C>(test_name_prefix: &str)
        where
            C: DataSource + ChartProperties + QuerySerialized<Output = Vec<Point>>,
            C::Resolution: Ord + Clone + Debug,
        {
            let stage_2_cutoff = d("2022-12-26");
            let stage_1_cutoff = d("2023-01-01");

            let (ref_time, ref_db, ref_indexer) =
                prepare_interchain_chart_test::<C>(&format!("{test_name_prefix}_reference")).await;
            let reference = update_and_query_interchain_chart_with_force::<C>(
                &ref_db,
                &ref_indexer,
                InterchainFilter::default(),
                ref_time,
                true,
            )
            .await;

            let (t1, db, indexer) =
                prepare_interchain_chart_test_unfilled::<C>(&format!("{test_name_prefix}_main"))
                    .await;
            fill_mock_interchain_reference_data(&indexer).await;

            // stage 1: only the tail (>= 2023-01-01)
            fill_mock_interchain_messages_in_range(&indexer, stage_1_cutoff..).await;
            let stage_1 = update_and_query_interchain_chart::<C>(
                &db,
                &indexer,
                InterchainFilter::default(),
                t1,
            )
            .await;
            assert_ne!(
                stage_1, reference,
                "{test_name_prefix}: stage 1 must be incomplete"
            );

            // stage 2: add [2022-12-26, 2023-01-01) — crosses month/year buckets,
            // but NOT the week bucket (both land in the week starting 2022-12-26)
            fill_mock_interchain_messages_in_range(&indexer, stage_2_cutoff..stage_1_cutoff).await;
            let t2 = t1 + TimeDelta::seconds(1);
            let stage_2 = update_and_query_interchain_chart::<C>(
                &db,
                &indexer,
                InterchainFilter::default(),
                t2,
            )
            .await;
            assert_ne!(
                stage_2, reference,
                "{test_name_prefix}: stage 2 must still be incomplete"
            );

            // stage 3: add everything before 2022-12-26 — crosses the week
            // bucket, but NOT the month/year buckets (both stay in 2022-12 / 2022)
            fill_mock_interchain_messages_in_range(&indexer, ..stage_2_cutoff).await;
            let t3 = t2 + TimeDelta::seconds(1);
            let stage_3 = update_and_query_interchain_chart::<C>(
                &db,
                &indexer,
                InterchainFilter::default(),
                t3,
            )
            .await;
            assert_eq!(
                stage_3, reference,
                "{test_name_prefix}: must converge to the from-scratch series after the full \
                 backfill"
            );
        }

        assert_converges_through_staged_backfill::<MessagesGrowthSentInterchainWeekly>(
            "messages_growth_sent_weekly_backfill",
        )
        .await;
        assert_converges_through_staged_backfill::<MessagesGrowthSentInterchainMonthly>(
            "messages_growth_sent_monthly_backfill",
        )
        .await;
        assert_converges_through_staged_backfill::<MessagesGrowthSentInterchainYearly>(
            "messages_growth_sent_yearly_backfill",
        )
        .await;
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

    /// The interior-fill case trigger 2 structurally cannot see, and the reason
    /// FIX 2 (forcing one rebuild on the verdict's `false → true` transition)
    /// exists: with per-chain decoupled catch-up, one chain can write its
    /// entire remaining history *above* another chain's already-lower floor —
    /// so `get_min_date` (the floor) never moves — and only then mark itself
    /// complete. `interchain_history_gap_has_data` (trigger 2) is blind to
    /// this by construction: it compares only the floor, never the interior.
    ///
    /// This test isolates that floor-blindness at the chart level: an interior
    /// date (`2023-01-10`, in the middle of the fixture's range) is withheld
    /// from an otherwise-complete fill, so the stored floor and the indexer's
    /// floor are identical before and after the fill — no floor movement
    /// anywhere in this test. It demonstrates both directions in one test, the
    /// same way `interchain_steady_state_does_not_rebuild_history` does for
    /// trigger 2: a plain incremental cycle (`force_full: false`, what an
    /// unfixed implementation computes once the verdict reports `true` with an
    /// unmoved floor) leaves the interior gap unfilled — proving the bug is
    /// real — and a forced cycle (`force_full: true`, what
    /// `resolve_interchain_verdict_transition`'s `false → true` branch
    /// actually produces for this exact cycle) picks it up and converges.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn interchain_verdict_transition_picks_up_an_interior_fill() {
        let interior_date = d("2023-01-10");

        let (ref_time, ref_db, ref_indexer) =
            prepare_interchain_chart_test::<MessagesGrowthSentInterchain>(
                "messages_growth_sent_interior_fill_reference",
            )
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

        let (t1, db, indexer) = prepare_interchain_chart_test_unfilled::<
            MessagesGrowthSentInterchain,
        >("messages_growth_sent_interior_fill_main")
        .await;
        fill_mock_interchain_reference_data(&indexer).await;
        // everything except the interior date — the earliest fixture date
        // (2022-12-20) is included, so the floor is already at its final value
        fill_mock_interchain_messages_in_range(&indexer, ..interior_date).await;
        fill_mock_interchain_messages_in_range(&indexer, interior_date.succ_opt().unwrap()..).await;
        let almost_complete = update_and_query_interchain_chart_with_force::<
            MessagesGrowthSentInterchain,
        >(&db, &indexer, InterchainFilter::default(), t1, true)
        .await;
        assert_ne!(
            almost_complete, reference,
            "withholding the interior date must leave the series incomplete"
        );

        // the indexer fills the interior date. The floor does not move: it was
        // already 2022-12-20 and stays 2022-12-20.
        fill_mock_interchain_messages_in_range(&indexer, interior_date..=interior_date).await;

        // an unfixed cycle: verdict reports complete, floor unchanged, so
        // trigger 1 and trigger 2 both stay silent — this is what
        // `resolve_interchain_verdict_transition` would produce *without* the
        // `false → true` branch (i.e. `verdict.complete` passed straight
        // through).
        let t2 = t1 + TimeDelta::seconds(1);
        let plain_increment = update_and_query_interchain_chart_with_force::<
            MessagesGrowthSentInterchain,
        >(&db, &indexer, InterchainFilter::default(), t2, false)
        .await;
        assert_eq!(
            plain_increment, almost_complete,
            "without forcing, an interior fill under an unmoved floor must be silently lost \
             — this is the bug FIX 2 exists to close"
        );

        // the forced cycle: what `resolve_interchain_verdict_transition`
        // actually returns for the cycle where the verdict is first observed
        // complete after being incomplete.
        let t3 = t2 + TimeDelta::seconds(1);
        let forced = update_and_query_interchain_chart_with_force::<MessagesGrowthSentInterchain>(
            &db,
            &indexer,
            InterchainFilter::default(),
            t3,
            true,
        )
        .await;
        assert_eq!(
            forced, reference,
            "the forced trailing rebuild must pick up the interior fill and converge"
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
