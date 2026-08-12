// SPDX-License-Identifier: LicenseRef-Blockscout

//! Tests for the case
//! - blockscout is fully indexed with the Filecoin fixture layer applied
//! - stats server is initialized with `enable_all_filecoin = true`
//!
//! `filecoinChainFeesGrowth` must be served under its own id with all
//! resolutions, and the public `txnsFee` id must serve the
//! `filecoinNewChainFees` implementation (chain-wide fees); the
//! implementation id and the intermediate charts stay hidden from the API.
//!
//! The same remap shape applies to the 24-hour counter: the public
//! `txnsFee24h` id serves the `filecoinChainFees24h` implementation, and the
//! internal id is never exposed under its own name. Unlike the line-chart
//! remap, the counter's *number* is not checked here — its single value is
//! defined relative to *now*, and this suite runs at wall-clock time against
//! a fixture that ends `2023-03-01`, so the 24-hour window is empty and the
//! counter reads `0` regardless of which implementation serves it. The
//! arithmetic is proven with the clock frozen by the DB tests in
//! `filecoin_chain_fees_24h.rs` (stats crate).

use std::collections::HashMap;

use blockscout_service_launcher::{
    launcher::GracefulShutdownHandler,
    test_server::{init_server, send_get_request},
};
use pretty_assertions::assert_eq;
use stats::{ResolutionKind, tests::init_db::init_db};
use stats_proto::blockscout::stats::v1::Counters;
use stats_server::stats;
use url::Url;

use super::common_tests::{test_counters_ok, test_lines_ok};
use crate::{
    common::{
        ChartSubset, assert_lines_not_served, get_test_stats_settings, sorted_vec,
        wait_for_subset_to_update,
    },
    it::mock_blockscout_simple::get_mock_blockscout_filecoin,
};

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
pub async fn run_tests_with_filecoin_charts_enabled() {
    let test_name = "run_tests_with_filecoin_charts_enabled";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout_filecoin().await;
    let blockscout_api = stats::tests::mock_blockscout::default_mock_blockscout_api().await;
    let (mut settings, base) =
        get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api, None);
    settings.enable_all_filecoin = true;
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_for_subset_to_update(&base, ChartSubset::AllCharts).await;

    // the shared exhaustive list check also pins that enabling the filecoin
    // flag adds exactly one new public id (`filecoinChainFeesGrowth`) and
    // remaps the existing `txnsFee` instead of adding a second one
    // (blockscout & user ops indexed, zetachain off, filecoin on)
    test_lines_ok(base.clone(), true, true, false, true).await;
    // counters remain the normal set under the flag: fee counters stay, and
    // the filecoin implementation is served under the existing `txnsFee24h`
    // id, so no new counter id appears (same indexing booleans as above)
    test_counters_ok(base.clone(), true, true, false).await;
    test_filecoin_charts_are_listed(&base).await;
    test_txns_fee_serves_filecoin_chain_fees_data(&base).await;
    test_filecoin_chain_fees_growth_data(&base).await;
    test_filecoin_intermediates_are_hidden(&base).await;
    test_txns_fee_24h_serves_filecoin_chain_fees_24h_implementation(&base).await;

    stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

async fn test_filecoin_charts_are_listed(base: &Url) {
    let line_charts: stats_proto::blockscout::stats::v1::LineCharts =
        send_get_request(base, "/api/v1/lines").await;
    let transactions_section = line_charts
        .sections
        .iter()
        .find(|sec| sec.id == "transactions")
        .expect("transactions section must be present");
    // `txnsFee` is the public id of the remapped chain-fees chart; its
    // metadata comes from the `txns_fee` config entry
    for chart_name in ["txnsFee", "filecoinChainFeesGrowth"] {
        let chart = transactions_section
            .charts
            .iter()
            .find(|chart| chart.id == chart_name)
            .unwrap_or_else(|| panic!("{chart_name} must be in the transactions section"));
        assert!(!chart.title.is_empty());
        assert!(!chart.description.is_empty());
        assert!(!chart.units().is_empty());
        let expected_resolutions: Vec<String> = [
            ResolutionKind::Day,
            ResolutionKind::Week,
            ResolutionKind::Month,
            ResolutionKind::Year,
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            sorted_vec(chart.resolutions.clone()),
            sorted_vec(expected_resolutions),
            "wrong resolutions for {chart_name}"
        );
    }
    // the implementation id must not be listed anywhere
    let listed_charts: Vec<&str> = line_charts
        .sections
        .iter()
        .flat_map(|sec| sec.charts.iter().map(|chart| chart.id.as_str()))
        .collect();
    assert!(
        !listed_charts.contains(&"filecoinNewChainFees"),
        "filecoinNewChainFees must not be listed as a public id"
    );
}

/// The HTTP API reads with `fill_missing_dates = true`, so gap days
/// materialize according to the missing date policy — including the genuine
/// no-data day of the fixture (`2022-12-15`: no f099 row, no block).
async fn get_filled_daily_data(base: &Url, chart_name: &str) -> HashMap<String, String> {
    let chart: stats_proto::blockscout::stats::v1::LineChart =
        send_get_request(base, &format!("/api/v1/lines/{chart_name}")).await;
    chart
        .chart
        .into_iter()
        .map(|point| (point.date, point.value))
        .collect()
}

/// The remap is proven end-to-end by observing the filecoin-computed numbers
/// under the public `txnsFee` id: the fixture DB also contains regular
/// transactions, so the replaced fee computation would yield *different*
/// numbers — matching the filecoin ones shows the implementation chart was
/// both served and updated through its own update group.
async fn test_txns_fee_serves_filecoin_chain_fees_data(base: &Url) {
    let chart: stats_proto::blockscout::stats::v1::LineChart =
        send_get_request(base, "/api/v1/lines/txnsFee").await;
    let info = chart.info.as_ref().expect("chart info must be present");
    assert_eq!(
        info.id, "txnsFee",
        "returned chart id (left) doesn't match requested (right)",
    );
    assert!(!info.title.is_empty());
    assert!(!info.description.is_empty());
    assert!(!info.units().is_empty());
    let data: HashMap<String, String> = chart
        .chart
        .into_iter()
        .map(|point| (point.date, point.value))
        .collect();
    // spot-check single points computed from the Filecoin fixture layer
    // (burn delta + fevm tips)
    let expected_points = [
        // first day: the whole starting f099 balance counts as burn
        ("2022-11-09", "30000000"),
        ("2022-11-10", "1000.0005840074068"),
        // tips-only day
        ("2022-11-11", "0.001193214813588"),
        // no-data day (no f099 row, no block): filled with 0 (FillZero)
        ("2022-12-15", "0"),
        // mixed day: understated tips-only value (see the `fevmFeeTips`
        // characterization test in the stats crate)
        ("2023-02-14", "0.0001"),
        // burn plus tips: the 24-hour counter's fixture blocks add two
        // priced transactions on this day (`COUNTER_END_ANCHOR_BLOCK` and
        // the window-edge boundary block, both included here since the
        // server updates at wall-clock time, well after the fixture's last
        // block — unlike the stats-crate chart test, which freezes
        // `update_time` exactly on the boundary block and therefore excludes
        // it; see `fevm_fee_tips.rs`'s test)
        ("2023-03-01", "3000.0002058"),
    ];
    for (date, value) in expected_points {
        assert_eq!(
            data.get(date).map(String::as_str),
            Some(value),
            "unexpected txnsFee value for {date}"
        );
    }
}

async fn test_filecoin_chain_fees_growth_data(base: &Url) {
    let data = get_filled_daily_data(base, "filecoinChainFeesGrowth").await;
    // cumulative values never decrease (fixture mirrors the production
    // invariant: tips are non-negative, the burn-actor balance only grows)
    let mut sorted: Vec<_> = data.iter().collect();
    sorted.sort();
    let values: Vec<f64> = sorted.iter().map(|(_, v)| v.parse().unwrap()).collect();
    assert!(
        values.windows(2).all(|w| w[0] <= w[1]),
        "cumulative chain fees must be monotonically non-decreasing: {sorted:?}"
    );
    // the no-data day holds the previous day's cumulative (FillPrevious)
    assert_eq!(
        data.get("2022-12-15").map(String::as_str),
        Some("30010000.003450688"),
        "no-data day must carry the cumulative value of 2022-12-01"
    );
    // moves only by the tips added on `2023-03-01` by the 24-hour counter's
    // fixture blocks (two priced transactions here — the server updates at
    // wall-clock time, so the window-edge boundary block is included, unlike
    // the frozen-clock stats-crate chart test); the burn side is unaffected
    // by the fixture's *split* of the old burn-only value across
    // `2023-02-28`/`2023-03-01` because a cumulative sum telescopes over it.
    assert_eq!(
        data.get("2023-03-01").map(String::as_str),
        Some("30050000.004932046")
    );
}

async fn test_filecoin_intermediates_are_hidden(base: &Url) {
    // not served by the line chart endpoints...
    assert_lines_not_served(base, &["burnActorBalance", "fevmFeeTips"]).await;
    // ...and not listed in any section
    let line_charts: stats_proto::blockscout::stats::v1::LineCharts =
        send_get_request(base, "/api/v1/lines").await;
    let listed_charts: Vec<&str> = line_charts
        .sections
        .iter()
        .flat_map(|sec| sec.charts.iter().map(|chart| chart.id.as_str()))
        .collect();
    for chart_name in ["burnActorBalance", "fevmFeeTips"] {
        assert!(
            !listed_charts.contains(&chart_name),
            "intermediate chart {chart_name} must not be listed"
        );
    }
}

/// Proves the counter side of the remap: `txnsFee24h` stays served (its
/// metadata comes from the public `txns_fee_24h` config entry) and the
/// internal `filecoinChainFees24h` id never appears, either in the counters
/// list or as a line chart.
///
/// The counter's *number* is deliberately not proven here — unlike the
/// line-chart remap (`test_txns_fee_serves_filecoin_chain_fees_data`), a
/// 24-hour counter's single value is defined relative to *now*, and this
/// suite runs at wall-clock time (`update_time_override: None` on every
/// server update path) over a fixture that ends `2023-03-01`. So the
/// 24-hour window is empty for both the remapped and the non-remapped
/// implementation, and the value below is `"0"` regardless of which one
/// serves the id: both burn anchors resolve to the same f099 row and no
/// transaction falls in the interval. The non-zero arithmetic is proven with
/// the clock frozen by the DB tests in `filecoin_chain_fees_24h.rs` (stats
/// crate).
///
/// Expected log noise, not a defect: at wall-clock time both anchors
/// resolve to the same f099 row, so the counter's degenerate/missing-anchors
/// warning fires on every run of this test — the designed behaviour for an
/// empty window.
async fn test_txns_fee_24h_serves_filecoin_chain_fees_24h_implementation(base: &Url) {
    let counters: Counters = send_get_request(base, "/api/v1/counters").await;
    let txns_fee_24h = counters
        .counters
        .iter()
        .find(|c| c.id == "txnsFee24h")
        .expect("txnsFee24h must be in the counters response");
    assert!(!txns_fee_24h.title.is_empty());
    assert!(!txns_fee_24h.description.is_empty());
    assert!(!txns_fee_24h.units().is_empty());
    assert_eq!(
        txns_fee_24h.value, "0",
        "empty window at wall-clock time over a fixture ending 2023-03-01 \
         must yield 0 for either implementation"
    );

    assert!(
        !counters
            .counters
            .iter()
            .any(|c| c.id == "filecoinChainFees24h"),
        "filecoinChainFees24h must not appear in /api/v1/counters"
    );
    assert_lines_not_served(base, &["filecoinChainFees24h"]).await;
}
