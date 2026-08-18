// SPDX-License-Identifier: LicenseRef-Blockscout

#![cfg(any(feature = "test-utils", test))]

//! Mock data for the interchain indexer DB, whose schema is created by the
//! indexer's own migrator (`interchain_indexer_migration::Migrator`, see
//! [`crate::tests::init_db::init_db_interchain`]).
//!
//! Tables filled here:
//! - `chains` — reference rows for [`MOCK_CHAIN_IDS`]; every chain-id column in
//!   the tables below is a foreign key into it.
//! - `bridges` — a single row, [`MOCK_BRIDGE_ID`].
//! - `crosschain_messages` — `(id, bridge_id, init_timestamp, src_chain_id,
//!   dst_chain_id, src_tx_hash, dst_tx_hash)`; every other column keeps its
//!   default or stays NULL.
//! - `crosschain_transfers` — `(id, message_id, bridge_id, index,
//!   token_src_chain_id, token_dst_chain_id, sender_address, recipient_address)`.
//!   A transfer has no timestamp of its own: its date comes from the parent
//!   message's `init_timestamp`. `index` is the transfer's 0-based position
//!   within its message, which `UNIQUE (message_id, bridge_id, index)` requires.
//!
//! `bridge_contracts` is deliberately left empty — no chart reads it.
//!
//! Invariants this fixture deliberately maintains, because the expected chart
//! values depend on them:
//! - there is exactly **one bridge**, so joining transfers to messages on
//!   `message_id` alone still resolves 1:1 even though the real key is
//!   `(message_id, bridge_id)`;
//! - every message has a **non-NULL `dst_chain_id`**;
//! - a transfer's `token_src_chain_id`/`token_dst_chain_id` always **equal its
//!   message's** `src_chain_id`/`dst_chain_id`.
//!
//! These are intentional, not incidental. A later change will break them on
//! purpose (a second bridge, a NULL destination, transfers whose token chains
//! diverge from their message route) in order to exercise the read filters —
//! and doing so will move existing expected values.
//!
//! Each message has 0..=5 associated transfers (last field in [`mock_rows`]).
//!
//! Covers at least two weeks, months and years with holes (gaps in dates).
//! Dates: late Dec 2022, Jan 2023, early Feb 2023.

use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};

/// Chain ids used by the fixture. Inserted into `chains` so that the
/// `crosschain_messages.{src,dst}_chain_id` and
/// `crosschain_transfers.token_{src,dst}_chain_id` foreign keys resolve.
pub const MOCK_CHAIN_IDS: [i64; 3] = [1, 2, 3];

/// The single bridge every fixture message and transfer belongs to.
pub const MOCK_BRIDGE_ID: i32 = 1;

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

/// One row: (init_timestamp, src_chain_id, dst_chain_id, src_tx_hash set?, dst_tx_hash set?, num_transfers)
fn mock_rows() -> Vec<(NaiveDateTime, i64, i64, bool, bool, u8)> {
    type D = NaiveDateTime;
    let d = |s: &str| D::from_str(s).unwrap();
    vec![
        // Dec 2022: 7 messages, 16 transfers
        (d("2022-12-20T10:00:00"), 1, 2, true, true, 2),
        (d("2022-12-21T10:00:00"), 1, 3, true, true, 0),
        (d("2022-12-21T11:00:00"), 2, 1, true, false, 3),
        (d("2022-12-23T10:00:00"), 2, 1, false, true, 1), // hole on 22nd
        (d("2022-12-26T10:00:00"), 1, 2, true, false, 5), // hole 24th-25th
        (d("2022-12-27T10:00:00"), 2, 3, true, true, 4),
        (d("2022-12-27T11:00:00"), 3, 1, false, true, 1),
        // Jan 2023: 10 messages, 17 transfers
        (d("2023-01-01T10:00:00"), 1, 2, true, true, 2),
        (d("2023-01-01T11:00:00"), 1, 3, true, false, 1),
        (d("2023-01-02T10:00:00"), 2, 1, false, true, 0),
        (d("2023-01-04T10:00:00"), 1, 2, true, true, 3), // hole 3rd
        (d("2023-01-10T10:00:00"), 1, 3, true, false, 2), // holes 5th-9th
        (d("2023-01-10T11:00:00"), 2, 1, true, false, 4),
        (d("2023-01-11T10:00:00"), 3, 2, false, true, 1),
        (d("2023-01-20T10:00:00"), 1, 2, true, false, 1), // holes 12th-19th
        (d("2023-01-21T10:00:00"), 2, 1, true, true, 3),
        (d("2023-01-21T11:00:00"), 3, 1, false, true, 0),
        // Feb 2023: 4 messages, 8 transfers
        (d("2023-02-01T10:00:00"), 1, 2, true, true, 2),
        (d("2023-02-01T11:00:00"), 1, 3, true, false, 5),
        (d("2023-02-05T10:00:00"), 2, 1, false, true, 1), // holes 2nd-04th
        (d("2023-02-10T10:00:00"), 1, 2, true, false, 0), // holes 6th-9th
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

/// Fills the interchain indexer DB with the mock fixture described in the
/// module docs.
pub async fn fill_mock_interchain_data(interchain: &DatabaseConnection, _max_date: NaiveDate) {
    let rows = mock_rows();

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

    bulk_insert(
        interchain,
        "bridges",
        &["id", "name"],
        vec![
            Value::Int(Some(MOCK_BRIDGE_ID)),
            Value::String(Some(Box::new(format!("mock_bridge_{MOCK_BRIDGE_ID}")))),
        ],
    )
    .await;

    let mut msg_values: Vec<Value> = Vec::with_capacity(rows.len() * MESSAGE_COLUMNS.len());
    for (i, (ts, src_chain_id, dst_chain_id, has_src_tx, has_dst_tx, _num_transfers)) in
        rows.iter().enumerate()
    {
        let src_tx_hash = has_src_tx.then(|| vec![(i + 1) as u8; 32]);
        let dst_tx_hash = has_dst_tx.then(|| vec![(i + 100) as u8; 32]);
        msg_values.extend([
            Value::BigInt(Some((i + 1) as i64)),
            Value::Int(Some(MOCK_BRIDGE_ID)),
            Value::ChronoDateTime(Some(Box::new(*ts))),
            Value::BigInt(Some(*src_chain_id)),
            Value::BigInt(Some(*dst_chain_id)),
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
    let mut transfer_values: Vec<Value> = Vec::new();
    let mut transfer_id: i64 = 1;
    for (i, (_ts, src_chain_id, dst_chain_id, _has_src_tx, _has_dst_tx, num_transfers)) in
        rows.iter().enumerate()
    {
        let message_id = (i + 1) as i64;
        for index in 0..*num_transfers {
            let sender_idx = ((transfer_id - 1) % 8) as u8;
            let recipient_idx = ((transfer_id + 2) % 8) as u8;
            transfer_values.extend([
                Value::BigInt(Some(transfer_id)),
                Value::BigInt(Some(message_id)),
                Value::Int(Some(MOCK_BRIDGE_ID)),
                Value::SmallInt(Some(index as i16)),
                // token chains == the message's route; see the module docs.
                Value::BigInt(Some(*src_chain_id)),
                Value::BigInt(Some(*dst_chain_id)),
                Value::Bytes(Some(Box::new(vec![sender_idx; 20]))),
                Value::Bytes(Some(Box::new(vec![recipient_idx; 20]))),
            ]);
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
