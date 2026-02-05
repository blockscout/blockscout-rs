#![cfg(any(feature = "test-utils", test))]

//! Mock data for interchain indexer DB (crosschain_messages table).
//! Schema: id, init_timestamp, src_chain_id, dst_chain_id, src_tx_hash, dst_tx_hash.
//!
//! Covers at least two weeks, months and years with holes (gaps in dates).
//! Dates: late Dec 2022, Jan 2023, early Feb 2023.

use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};

/// One row: (init_timestamp, src_chain_id, dst_chain_id, src_tx_hash set?, dst_tx_hash set?)
fn mock_rows() -> Vec<(NaiveDateTime, i64, i64, bool, bool)> {
    type D = NaiveDateTime;
    let d = |s: &str| D::from_str(s).unwrap();
    vec![
        // Dec 2022 (2+ weeks back from Jan): 7 messages
        (d("2022-12-20T10:00:00"), 1, 2, true, true),
        (d("2022-12-21T10:00:00"), 1, 3, true, true),
        (d("2022-12-21T11:00:00"), 2, 1, true, false),
        (d("2022-12-23T10:00:00"), 2, 1, false, true), // hole on 22nd
        (d("2022-12-26T10:00:00"), 1, 2, true, false), // hole 24th-25th
        (d("2022-12-27T10:00:00"), 2, 3, true, true),
        (d("2022-12-27T11:00:00"), 3, 1, false, true),
        // Jan 2023: 10 messages
        (d("2023-01-01T10:00:00"), 1, 2, true, true),
        (d("2023-01-01T11:00:00"), 1, 3, true, false),
        (d("2023-01-02T10:00:00"), 2, 1, false, true),
        (d("2023-01-04T10:00:00"), 1, 2, true, true),  // hole 3rd
        (d("2023-01-10T10:00:00"), 1, 3, true, false), // holes 5th-9th
        (d("2023-01-10T11:00:00"), 2, 1, true, false),
        (d("2023-01-11T10:00:00"), 3, 2, false, true),
        (d("2023-01-20T10:00:00"), 1, 2, true, false), // holes 12th-19th
        (d("2023-01-21T10:00:00"), 2, 1, true, true),
        (d("2023-01-21T11:00:00"), 3, 1, false, true),
        // Feb 2023: 4 messages
        (d("2023-02-01T10:00:00"), 1, 2, true, true),
        (d("2023-02-01T11:00:00"), 1, 3, true, false),
        (d("2023-02-05T10:00:00"), 2, 1, false, true), // holes 2nd-04th
        (d("2023-02-10T10:00:00"), 1, 2, true, false), // holes 6th-9th
    ]
}

/// Fills crosschain_messages with test rows spanning 2+ weeks, 2+ months, 2 years, with holes.
pub async fn fill_mock_interchain_data(interchain: &DatabaseConnection, _max_date: NaiveDate) {
    let rows = mock_rows();
    for (i, (ts, src_chain_id, dst_chain_id, has_src_tx, has_dst_tx)) in rows.into_iter().enumerate() {
        let src_tx_hash = if has_src_tx {
            Some(vec![(i + 1) as u8; 32])
        } else {
            None
        };
        let dst_tx_hash = if has_dst_tx {
            Some(vec![(i + 100) as u8; 32])
        } else {
            None
        };
        let stmt = Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            r#"
            INSERT INTO crosschain_messages (id, init_timestamp, src_chain_id, dst_chain_id, src_tx_hash, dst_tx_hash)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            [
                Value::BigInt(Some((i + 1) as i64)),
                Value::ChronoDateTime(Some(Box::new(ts))),
                Value::BigInt(Some(src_chain_id)),
                Value::BigInt(Some(dst_chain_id)),
                src_tx_hash
                    .map(|v| Value::Bytes(Some(Box::new(v))))
                    .unwrap_or(Value::Bytes(None)),
                dst_tx_hash
                    .map(|v| Value::Bytes(Some(Box::new(v))))
                    .unwrap_or(Value::Bytes(None)),
            ],
        );
        interchain.execute(stmt).await.unwrap();
    }
}
