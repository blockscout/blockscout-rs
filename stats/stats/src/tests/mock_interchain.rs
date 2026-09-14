// SPDX-License-Identifier: LicenseRef-Blockscout

#![cfg(any(feature = "test-utils", test))]

//! Mock data for the interchain indexer DB, whose schema is created by the
//! indexer's own migrator (`interchain_indexer_migration::Migrator`, see
//! [`crate::tests::init_db::init_db_interchain`]).
//!
//! Tables filled here:
//! - `chains` — reference rows for [`MOCK_CHAIN_IDS`]; every chain-id column in
//!   the tables below is a foreign key into it.
//! - `bridges` — [`MOCK_BRIDGE_ID`] and [`MOCK_SECOND_BRIDGE_ID`].
//! - `crosschain_messages` — `(id, bridge_id, init_timestamp, src_chain_id,
//!   dst_chain_id, src_tx_hash, dst_tx_hash)`; every other column keeps its
//!   default or stays NULL. The primary key is the **pair** `(id, bridge_id)`,
//!   which is why two bridges may carry the same numeric id.
//! - `crosschain_transfers` — `(id, message_id, bridge_id, index,
//!   token_src_chain_id, token_dst_chain_id, sender_address, recipient_address)`.
//!   A transfer has no timestamp of its own: its date comes from the parent
//!   message's `init_timestamp`. `index` is the transfer's 0-based position
//!   within its message, which `UNIQUE (message_id, bridge_id, index)` requires.
//! - `bridge_contracts` — contracts of [`MOCK_BRIDGE_ID`] on
//!   [`MOCK_BRIDGE_CONTRACT_CHAIN_IDS`], with one chain deliberately carrying two
//!   contract rows so the horizon resolver's de-duplication is exercised.
//!   [`MOCK_SECOND_BRIDGE_ID`] gets **no** contract rows at all: that is the
//!   "present but observes nothing" case, whose own horizon disjunct renders
//!   `1 = 2` and hides every one of its rows.
//!
//! The horizon these two tables resolve to is [`mock_interchain_horizon`].
//!
//! ## The five deliberately awkward cases
//!
//! Messages 1..=21 are the original fixture: all on [`MOCK_BRIDGE_ID`], all with
//! a non-NULL `dst_chain_id`, and every transfer's token chains equal to its
//! message's route. Everything after them exists to make a filter dimension
//! observable that otherwise could not be tested at all:
//!
//! - **message 22 — NULL destination.** Exercises the NULL semantics of
//!   `counterparty_chain_ids`/`dst_chain_ids` (a NULL never satisfies `IN`) and
//!   the horizon's explicit `dst IS NOT NULL` guard on the permissive arm. Its
//!   single transfer still has a known token destination, which is itself a case
//!   worth having: a transfer whose route is unknown but whose token pair is not.
//! - **message 23 — an endpoint outside its bridge's horizon** (`dst = 4`, and
//!   bridge 1 has no contract on chain 4). Included by every configured
//!   dimension, excluded by the observability horizon.
//! - **message 24 — token chains diverging from the message route** (route
//!   `1 → 2`, token chains `3 → 4`). This is the only row that can distinguish
//!   filtering a transfer on its own `token_src_chain_id`/`token_dst_chain_id`
//!   from filtering it on the joined message's `src_chain_id`/`dst_chain_id`.
//! - **`(id = 1, bridge_id = 2)` — a numeric message id reused across bridges.**
//!   Without it, joining transfers to messages on `message_id` alone would give
//!   the same answer as the composite `(message_id, bridge_id)` join, and the
//!   composite-join fix would be untested. With it, a non-composite join
//!   fans transfers out across bridges and the transfer counts grow.
//! - **`MOCK_SECOND_BRIDGE_ID` with zero `bridge_contracts` rows** — the
//!   "present but empty" horizon case.
//!
//! Covers at least two weeks, months and years with holes (gaps in dates).
//! Dates: late Dec 2022, Jan 2023, early Feb 2023.

use std::{ops::RangeBounds, str::FromStr};

use chrono::{NaiveDate, NaiveDateTime};
use interchain_indexer_filters::ChainBridgeFilter;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};

use crate::charts::db_interaction::filters::interchain::{
    InterchainFilter, InterchainFilterConfig,
};

/// Chain ids used by the fixture. Inserted into `chains` so that the
/// `crosschain_messages.{src,dst}_chain_id` and
/// `crosschain_transfers.token_{src,dst}_chain_id` foreign keys resolve.
///
/// Chain `4` exists only as an endpoint outside [`MOCK_BRIDGE_ID`]'s observed
/// set — see [`MOCK_BRIDGE_CONTRACT_CHAIN_IDS`].
pub const MOCK_CHAIN_IDS: [i64; 4] = [1, 2, 3, 4];

/// The bridge messages 1..=24 belong to.
pub const MOCK_BRIDGE_ID: i32 = 1;

/// A second bridge, whose messages deliberately reuse numeric ids of
/// [`MOCK_BRIDGE_ID`]'s messages, and which has no `bridge_contracts` rows.
pub const MOCK_SECOND_BRIDGE_ID: i32 = 2;

/// The chains [`MOCK_BRIDGE_ID`] has contracts on — i.e. the chains it could
/// have observed. Deliberately a strict subset of [`MOCK_CHAIN_IDS`].
pub const MOCK_BRIDGE_CONTRACT_CHAIN_IDS: [i64; 3] = [1, 2, 3];

/// The observability horizon the fixture's `bridges` / `bridge_contracts` rows
/// resolve to via `resolve_only_indexed_by_bridge`.
///
/// [`MOCK_SECOND_BRIDGE_ID`] is listed with an **empty** chain set, which is the
/// case that excludes all of its own rows.
pub fn mock_interchain_horizon() -> Vec<(i32, Vec<i64>)> {
    vec![
        (MOCK_BRIDGE_ID, MOCK_BRIDGE_CONTRACT_CHAIN_IDS.to_vec()),
        (MOCK_SECOND_BRIDGE_ID, vec![]),
    ]
}

/// Build a filter from the configured dimensions, with the observability horizon
/// **disabled**. Horizon cases must pass their pairs explicitly via
/// [`test_interchain_filter_with_horizon`].
pub fn test_interchain_filter(configured: ChainBridgeFilter) -> InterchainFilter {
    InterchainFilterConfig::new(configured, true).with_horizon(None)
}

/// As [`test_interchain_filter`], with the horizon restriction enabled and
/// resolved to `pairs`. Pass [`mock_interchain_horizon`] for what this fixture's
/// tables actually resolve to.
pub fn test_interchain_filter_with_horizon(
    configured: ChainBridgeFilter,
    pairs: Option<Vec<(i32, Vec<i64>)>>,
) -> InterchainFilter {
    InterchainFilterConfig::new(configured, false).with_horizon(pairs)
}

/// The most common test filter: a focal chain and nothing else.
pub fn test_interchain_home_chain_filter(home_chain_id: i64) -> InterchainFilter {
    test_interchain_filter(ChainBridgeFilter {
        home_chain_id: Some(home_chain_id),
        ..Default::default()
    })
}

/// `index` is an unreserved keyword in Postgres, so it needs no quoting here
/// (the schema itself declares the column unquoted).
const MESSAGE_COLUMNS: [&str; 7] = [
    "id",
    "bridge_id",
    "init_timestamp",
    "src_chain_id",
    "dst_chain_id",
    "src_tx_hash",
    "dst_tx_hash",
];

const TRANSFER_COLUMNS: [&str; 8] = [
    "id",
    "message_id",
    "bridge_id",
    "index",
    "token_src_chain_id",
    "token_dst_chain_id",
    "sender_address",
    "recipient_address",
];

/// One fixture message and the transfers hanging off it.
struct MockMessage {
    init_timestamp: NaiveDateTime,
    bridge_id: i32,
    /// `crosschain_messages.id`; unique only **within** a bridge.
    message_id: i64,
    src_chain_id: i64,
    dst_chain_id: Option<i64>,
    has_src_tx: bool,
    has_dst_tx: bool,
    /// `(token_src_chain_id, token_dst_chain_id)` per transfer, in `index` order.
    transfers: Vec<(i64, i64)>,
}

/// A message on [`MOCK_BRIDGE_ID`] with a known destination and `num_transfers`
/// transfers whose token chains equal the message route — the shape every
/// original fixture row has.
fn routed(
    init_timestamp: &str,
    message_id: i64,
    src_chain_id: i64,
    dst_chain_id: i64,
    has_src_tx: bool,
    has_dst_tx: bool,
    num_transfers: usize,
) -> MockMessage {
    MockMessage {
        init_timestamp: timestamp(init_timestamp),
        bridge_id: MOCK_BRIDGE_ID,
        message_id,
        src_chain_id,
        dst_chain_id: Some(dst_chain_id),
        has_src_tx,
        has_dst_tx,
        transfers: vec![(src_chain_id, dst_chain_id); num_transfers],
    }
}

fn timestamp(value: &str) -> NaiveDateTime {
    NaiveDateTime::from_str(value).unwrap()
}

/// The fixture, in insertion order. Transfer ids are assigned by walking this
/// list, so appending (rather than inserting) keeps every existing transfer's id
/// — and therefore its cycled addresses — unchanged.
fn mock_rows() -> Vec<MockMessage> {
    vec![
        // Dec 2022: 7 messages, 16 transfers
        routed("2022-12-20T10:00:00", 1, 1, 2, true, true, 2),
        routed("2022-12-21T10:00:00", 2, 1, 3, true, true, 0),
        routed("2022-12-21T11:00:00", 3, 2, 1, true, false, 3),
        routed("2022-12-23T10:00:00", 4, 2, 1, false, true, 1), // hole on 22nd
        routed("2022-12-26T10:00:00", 5, 1, 2, true, false, 5), // hole 24th-25th
        routed("2022-12-27T10:00:00", 6, 2, 3, true, true, 4),
        routed("2022-12-27T11:00:00", 7, 3, 1, false, true, 1),
        // Jan 2023: 10 messages, 17 transfers
        routed("2023-01-01T10:00:00", 8, 1, 2, true, true, 2),
        routed("2023-01-01T11:00:00", 9, 1, 3, true, false, 1),
        routed("2023-01-02T10:00:00", 10, 2, 1, false, true, 0),
        routed("2023-01-04T10:00:00", 11, 1, 2, true, true, 3), // hole 3rd
        routed("2023-01-10T10:00:00", 12, 1, 3, true, false, 2), // holes 5th-9th
        routed("2023-01-10T11:00:00", 13, 2, 1, true, false, 4),
        routed("2023-01-11T10:00:00", 14, 3, 2, false, true, 1),
        routed("2023-01-20T10:00:00", 15, 1, 2, true, false, 1), // holes 12th-19th
        routed("2023-01-21T10:00:00", 16, 2, 1, true, true, 3),
        routed("2023-01-21T11:00:00", 17, 3, 1, false, true, 0),
        // Feb 2023: 4 messages, 8 transfers
        routed("2023-02-01T10:00:00", 18, 1, 2, true, true, 2),
        routed("2023-02-01T11:00:00", 19, 1, 3, true, false, 5),
        routed("2023-02-05T10:00:00", 20, 2, 1, false, true, 1), // holes 2nd-04th
        routed("2023-02-10T10:00:00", 21, 1, 2, true, false, 0), // holes 6th-9th
        // --- the awkward cases; see the module docs. Spelled out as struct
        // literals rather than squeezed through another positional constructor:
        // every one of them differs from `routed` in a different field.
        //
        // NULL destination. Its transfer's token destination is known anyway.
        MockMessage {
            init_timestamp: timestamp("2023-02-07T10:00:00"),
            bridge_id: MOCK_BRIDGE_ID,
            message_id: 22,
            src_chain_id: 1,
            dst_chain_id: None,
            has_src_tx: true,
            has_dst_tx: false,
            transfers: vec![(1, 2)],
        },
        // an endpoint outside bridge 1's contract chains
        MockMessage {
            init_timestamp: timestamp("2023-02-08T10:00:00"),
            bridge_id: MOCK_BRIDGE_ID,
            message_id: 23,
            src_chain_id: 1,
            dst_chain_id: Some(4),
            has_src_tx: true,
            has_dst_tx: true,
            transfers: vec![(1, 4)],
        },
        // token chains deliberately unequal to the message route
        MockMessage {
            init_timestamp: timestamp("2023-02-09T10:00:00"),
            bridge_id: MOCK_BRIDGE_ID,
            message_id: 24,
            src_chain_id: 1,
            dst_chain_id: Some(2),
            has_src_tx: true,
            has_dst_tx: true,
            transfers: vec![(3, 4)],
        },
        // the id collision: bridge 2 reuses message id 1
        MockMessage {
            init_timestamp: timestamp("2023-02-06T10:00:00"),
            bridge_id: MOCK_SECOND_BRIDGE_ID,
            message_id: 1,
            src_chain_id: 1,
            dst_chain_id: Some(2),
            has_src_tx: true,
            has_dst_tx: true,
            transfers: vec![(1, 2), (1, 2)],
        },
        // ...and one bridge-2 message with an id of its own, so that bridge-2
        // rows are not all collisions
        MockMessage {
            init_timestamp: timestamp("2023-02-06T11:00:00"),
            bridge_id: MOCK_SECOND_BRIDGE_ID,
            message_id: 100,
            src_chain_id: 2,
            dst_chain_id: Some(3),
            has_src_tx: true,
            has_dst_tx: false,
            transfers: vec![(2, 3)],
        },
    ]
}

/// Builds `($1, ..., $n), ($n+1, ..., $2n), ...` for `rows` rows of `columns`
/// columns each.
fn placeholders(rows: usize, columns: usize) -> String {
    (0..rows)
        .map(|row| {
            let row_placeholders: Vec<String> = (0..columns)
                .map(|column| format!("${}", row * columns + column + 1))
                .collect();
            format!("({})", row_placeholders.join(", "))
        })
        .collect::<Vec<String>>()
        .join(", ")
}

/// Bulk-inserts `values` into `table`. `values` is row-major, with exactly
/// `columns.len()` entries per row.
async fn bulk_insert(
    interchain: &DatabaseConnection,
    table: &str,
    columns: &[&str],
    values: Vec<Value>,
) {
    if values.is_empty() {
        return;
    }
    assert_eq!(
        values.len() % columns.len(),
        0,
        "{table}: {} values do not divide into rows of {} columns",
        values.len(),
        columns.len()
    );
    let sql = format!(
        "INSERT INTO {table} ({}) VALUES {}",
        columns.join(", "),
        placeholders(values.len() / columns.len(), columns.len())
    );
    interchain
        .execute(Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            &sql,
            values,
        ))
        .await
        .unwrap();
}

/// The `chains` / `bridges` / `bridge_contracts` reference rows. Exactly once
/// per database — the message/transfer helpers do not insert them.
pub async fn fill_mock_interchain_reference_data(interchain: &DatabaseConnection) {
    // Reference rows first, so the foreign keys of the two canonical tables
    // resolve. `chains.name` and `bridges.name` are both `TEXT NOT NULL UNIQUE`.
    let chain_values: Vec<Value> = MOCK_CHAIN_IDS
        .iter()
        .flat_map(|chain_id| {
            [
                Value::BigInt(Some(*chain_id)),
                Value::String(Some(Box::new(format!("mock_chain_{chain_id}")))),
            ]
        })
        .collect();
    bulk_insert(interchain, "chains", &["id", "name"], chain_values).await;

    let bridge_values: Vec<Value> = [MOCK_BRIDGE_ID, MOCK_SECOND_BRIDGE_ID]
        .iter()
        .flat_map(|bridge_id| {
            [
                Value::Int(Some(*bridge_id)),
                Value::String(Some(Box::new(format!("mock_bridge_{bridge_id}")))),
            ]
        })
        .collect();
    bulk_insert(interchain, "bridges", &["id", "name"], bridge_values).await;

    // Contracts of bridge 1 only, on chains 1/2/3 — chain 4 stays outside its
    // observed set on purpose. Chain 1 gets a second contract row (a different
    // address, which `UNIQUE (bridge_id, chain_id, address, version)` permits) so
    // that `resolve_only_indexed_by_bridge` has something to de-duplicate.
    // Bridge 2 gets no rows at all; see the module docs.
    let mut contract_values: Vec<Value> = Vec::new();
    let mut push_contract = |chain_id: i64, address_byte: u8| {
        contract_values.extend([
            Value::Int(Some(MOCK_BRIDGE_ID)),
            Value::BigInt(Some(chain_id)),
            Value::Bytes(Some(Box::new(vec![address_byte; 20]))),
            Value::SmallInt(Some(1)),
        ]);
    };
    for chain_id in MOCK_BRIDGE_CONTRACT_CHAIN_IDS {
        push_contract(chain_id, chain_id as u8);
    }
    push_contract(MOCK_BRIDGE_CONTRACT_CHAIN_IDS[0], 0xaa);
    bulk_insert(
        interchain,
        "bridge_contracts",
        &["bridge_id", "chain_id", "address", "version"],
        contract_values,
    )
    .await;
}

/// The fixture's messages and their transfers, restricted to those whose
/// `init_timestamp` date falls in `dates`.
///
/// Ids are assigned by walking the **whole** fixture, so filling it in two
/// complementary ranges leaves the database identical to one full fill — which
/// is what the backfill-convergence tests compare against.
pub async fn fill_mock_interchain_messages_in_range(
    interchain: &DatabaseConnection,
    dates: impl RangeBounds<NaiveDate>,
) {
    let rows = mock_rows();

    let mut msg_values: Vec<Value> = Vec::new();
    for (i, message) in rows.iter().enumerate() {
        // the tx hashes only have to be distinct and non-NULL; the row's position
        // in the *whole* fixture is a convenient source of distinct bytes, kept
        // stable regardless of which range is being inserted
        let src_tx_hash = message.has_src_tx.then(|| vec![(i + 1) as u8; 32]);
        let dst_tx_hash = message.has_dst_tx.then(|| vec![(i + 100) as u8; 32]);
        if !dates.contains(&message.init_timestamp.date()) {
            continue;
        }
        msg_values.extend([
            Value::BigInt(Some(message.message_id)),
            Value::Int(Some(message.bridge_id)),
            Value::ChronoDateTime(Some(Box::new(message.init_timestamp))),
            Value::BigInt(Some(message.src_chain_id)),
            Value::BigInt(message.dst_chain_id),
            Value::Bytes(src_tx_hash.map(Box::new)),
            Value::Bytes(dst_tx_hash.map(Box::new)),
        ]);
    }
    bulk_insert(
        interchain,
        "crosschain_messages",
        &MESSAGE_COLUMNS,
        msg_values,
    )
    .await;

    // Transfers. `index` counts within the parent message (required by
    // `UNIQUE (message_id, bridge_id, index)`), while the address cycling uses
    // the *global* transfer id so that exactly 8 distinct 20-byte addresses
    // appear — which is what `totalInterchainTransferUsers = 8` relies on.
    //
    // `transfer_id` is advanced by walking the *whole* fixture (not just the
    // messages in `dates`), so that a staged fill in two complementary ranges
    // assigns exactly the same ids — and therefore the same cycled addresses —
    // as one full fill.
    let mut transfer_values: Vec<Value> = Vec::new();
    let mut transfer_id: i64 = 1;
    for message in rows.iter() {
        let message_in_range = dates.contains(&message.init_timestamp.date());
        for (index, (token_src_chain_id, token_dst_chain_id)) in
            message.transfers.iter().enumerate()
        {
            let sender_idx = ((transfer_id - 1) % 8) as u8;
            let recipient_idx = ((transfer_id + 2) % 8) as u8;
            if message_in_range {
                transfer_values.extend([
                    Value::BigInt(Some(transfer_id)),
                    Value::BigInt(Some(message.message_id)),
                    Value::Int(Some(message.bridge_id)),
                    Value::SmallInt(Some(index as i16)),
                    Value::BigInt(Some(*token_src_chain_id)),
                    Value::BigInt(Some(*token_dst_chain_id)),
                    Value::Bytes(Some(Box::new(vec![sender_idx; 20]))),
                    Value::Bytes(Some(Box::new(vec![recipient_idx; 20]))),
                ]);
            }
            transfer_id += 1;
        }
    }
    bulk_insert(
        interchain,
        "crosschain_transfers",
        &TRANSFER_COLUMNS,
        transfer_values,
    )
    .await;
}

/// Fills the interchain indexer DB with the mock fixture described in the
/// module docs.
pub async fn fill_mock_interchain_data(interchain: &DatabaseConnection, _max_date: NaiveDate) {
    fill_mock_interchain_reference_data(interchain).await;
    fill_mock_interchain_messages_in_range(interchain, ..).await;
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// The fixture's own shape, asserted rather than assumed: the chart
    /// expectations throughout the suite are derived from these two numbers and
    /// from the five cases below.
    #[test]
    fn fixture_has_the_expected_shape() {
        let rows = mock_rows();
        assert_eq!(rows.len(), 26, "message count");
        assert_eq!(
            rows.iter().map(|m| m.transfers.len()).sum::<usize>(),
            47,
            "transfer count"
        );

        let collisions = rows
            .iter()
            .filter(|m| {
                rows.iter()
                    .any(|o| o.message_id == m.message_id && o.bridge_id != m.bridge_id)
            })
            .count();
        assert_eq!(
            collisions, 2,
            "one message id reused across the two bridges"
        );
        assert_eq!(
            rows.iter().filter(|m| m.dst_chain_id.is_none()).count(),
            1,
            "one message with a NULL destination"
        );
        assert_eq!(
            rows.iter()
                .flat_map(|m| m.transfers.iter().map(move |t| (m, t)))
                .filter(|(m, (src, dst))| Some(*src) != Some(m.src_chain_id)
                    || m.dst_chain_id != Some(*dst))
                .count(),
            2,
            "message 22 (NULL route) and message 24 (divergent token chains)"
        );
        assert!(
            rows.iter().any(|m| m.dst_chain_id == Some(4)),
            "one message with an endpoint outside its bridge's contract chains"
        );
    }
}
