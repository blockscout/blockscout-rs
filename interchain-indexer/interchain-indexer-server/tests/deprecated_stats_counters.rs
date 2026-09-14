// SPDX-License-Identifier: LicenseRef-Blockscout

//! HTTP contract test for the two deprecated counter endpoints,
//! `GET /api/v1/stats/common` and `GET /api/v1/stats/daily`.
//!
//! Deprecation kept the routes and dropped the numbers: both used to run an
//! unbounded `COUNT(*)` over `crosschain_messages` plus a joined `COUNT(*)`
//! over `crosschain_transfers` on every request, and the stats service in
//! interchain mode now precomputes the same values. Three things therefore need
//! pinning, and only a running server pins them: the routes still answer `200`
//! with their full response shape, the counters stay `0` **even when the
//! canonical tables hold matching rows** — which is what fails if a future
//! change wires the queries back in — and a malformed filter still fails with
//! `400` rather than silently answering zeros.
//!
//! **One server, one database, deliberately.** All three checks are read-only
//! against the same seeded row, so splitting them across tests would boot three
//! servers to observe one state. That is not free: every server test boots the
//! whole service through `run()`, `test_server::init_server` never shuts one
//! down, and the port each one gets comes from a `get_free_port()` that binds
//! `:0`, reads the port and *drops* the listener before `run()` binds it for
//! real. Two servers racing for one port end with the loser panicking
//! `AddrInUse` inside its spawned task — swallowed — while its health check
//! passes against the *winner's* server, so the victim test silently talks to
//! someone else's database and fails as an unexplained `500`. Fewer concurrent
//! servers, less of that; see also the `ulimit` note in the `justfile` for the
//! file-descriptor half of the same problem.
//!
//! The cost is granularity: the first failing assertion hides the rest. That is
//! the right trade here, since a regression in any of the three has the same
//! single cause — someone reconnecting the removed queries.
//!
//! TODO(next API iteration): delete this file together with the endpoints.

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::DateTime;
use interchain_indexer_entity::{crosschain_messages, sea_orm_active_enums::MessageStatus};
use sea_orm::{ActiveValue::Set, EntityTrait};

/// Seeded on bridge 1 (`config/omnibridge/bridges.json`) between chains
/// `{1, 100}`, so it satisfies the default read filter.
const SEEDED_MESSAGE_ID: i64 = 7001;
/// `2026-01-02T00:30:00Z` — the seeded message's `init_timestamp`.
const SEEDED_INIT_TIMESTAMP: i64 = 1767313800;
/// `2026-01-02T02:54:05Z` — the timestamp both requests ask about. Strictly
/// after the seeded message and on the same UTC day, so the message satisfies
/// the total (`init_timestamp < timestamp`) *and* the daily
/// (`[day_start, next_day_start)`) predicate. Both endpoints would therefore
/// have answered `1` before deprecation, which is what makes the zero
/// assertions below meaningful rather than vacuous.
const REQUEST_TIMESTAMP: i64 = 1767322445;
const REQUEST_DAY: &str = "2026-01-02";

#[tokio::test]
#[ignore = "Needs database to run"]
async fn deprecated_counter_endpoints_answer_zeros_and_still_validate_filters() {
    let db = helpers::init_db(
        "test",
        "deprecated_counter_endpoints_answer_zeros_and_still_validate_filters",
    )
    .await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(SEEDED_MESSAGE_ID),
        bridge_id: Set(1),
        status: Set(MessageStatus::Initiated),
        init_timestamp: Set(DateTime::from_timestamp(SEEDED_INIT_TIMESTAMP, 0)
            .expect("a valid seeded timestamp")
            .naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(Some(100)),
        ..Default::default()
    })
    .exec(db.client().as_ref())
    .await
    .unwrap();

    // `/stats/common` — an explicit timestamp, so the seeded message is inside
    // the window the endpoint was asked about.
    let route = format!("/api/v1/stats/common?timestamp={REQUEST_TIMESTAMP}");
    let common: serde_json::Value = test_server::send_get_request(&base, &route).await;
    assert_eq!(
        common["total_messages"],
        serde_json::json!("0"),
        "the deprecated endpoint must report zero messages even with an indexed message \
         present; got {common}"
    );
    assert_eq!(
        common["total_transfers"],
        serde_json::json!("0"),
        "the deprecated endpoint must report zero transfers; got {common}"
    );
    assert!(
        common["timestamp"].is_string(),
        "the request's own timestamp must still be echoed back, so the response shape is \
         unchanged for existing clients; got {common}"
    );

    // `/stats/daily` — same timestamp, so `date` is asserted against a known
    // day rather than against "today".
    let route = format!("/api/v1/stats/daily?timestamp={REQUEST_TIMESTAMP}");
    let daily: serde_json::Value = test_server::send_get_request(&base, &route).await;
    assert_eq!(
        daily["daily_messages"],
        serde_json::json!("0"),
        "the deprecated endpoint must report zero messages even with an indexed message \
         present; got {daily}"
    );
    assert_eq!(
        daily["daily_transfers"],
        serde_json::json!("0"),
        "the deprecated endpoint must report zero transfers; got {daily}"
    );
    assert_eq!(
        daily["date"],
        serde_json::json!(REQUEST_DAY),
        "`date` is the UTC day of the request's own timestamp, computed without a query; \
         got {daily}"
    );

    // Request validation outlives the queries.
    for route in [
        "/api/v1/stats/common?bridge_ids=not-a-number",
        "/api/v1/stats/daily?bridge_ids=not-a-number",
    ] {
        let (status, body) = helpers::get_raw(&base, route).await;
        assert_eq!(
            status,
            reqwest::StatusCode::BAD_REQUEST,
            "{route} must still validate its filters; got {body}",
        );
    }
}
