#![cfg(any(feature = "test-utils", test))]

//! Mock data for interchain indexer DB (crosschain_messages, crosschain_transfers).
//! Messages: id, init_timestamp, src_chain_id, dst_chain_id, src_tx_hash, dst_tx_hash.
//! Transfers: id, message_id (no init_timestamp; date comes from message's init_timestamp).
//! Each message has 0..=5 associated transfers (last field in mock_rows).
//!
//! Covers at least two weeks, months and years with holes (gaps in dates).
//! Dates: late Dec 2022, Jan 2023, early Feb 2023.

use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, Value};

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

/// Fills crosschain_messages and crosschain_transfers with bulk INSERTs.
pub async fn fill_mock_interchain_data(interchain: &DatabaseConnection, _max_date: NaiveDate) {
    let rows = mock_rows();
    let n = rows.len();

    // Bulk insert messages: VALUES ($1..$6), ($7..$12), ...
    let mut msg_values: Vec<Value> = Vec::with_capacity(n * 6);
    for (i, (ts, src_chain_id, dst_chain_id, has_src_tx, has_dst_tx, _num_transfers)) in
        rows.iter().enumerate()
    {
        let src_tx_hash = if *has_src_tx {
            Some(vec![(i + 1) as u8; 32])
        } else {
            None
        };
        let dst_tx_hash = if *has_dst_tx {
            Some(vec![(i + 100) as u8; 32])
        } else {
            None
        };
        msg_values.push(Value::BigInt(Some((i + 1) as i64)));
        msg_values.push(Value::ChronoDateTime(Some(Box::new(*ts))));
        msg_values.push(Value::BigInt(Some(*src_chain_id)));
        msg_values.push(Value::BigInt(Some(*dst_chain_id)));
        msg_values.push(
            src_tx_hash
                .map(|v| Value::Bytes(Some(Box::new(v))))
                .unwrap_or(Value::Bytes(None)),
        );
        msg_values.push(
            dst_tx_hash
                .map(|v| Value::Bytes(Some(Box::new(v))))
                .unwrap_or(Value::Bytes(None)),
        );
    }
    let msg_placeholders: Vec<String> = (0..n)
        .map(|i| {
            let b = i * 6 + 1;
            format!(
                "(${}, ${}, ${}, ${}, ${}, ${})",
                b,
                b + 1,
                b + 2,
                b + 3,
                b + 4,
                b + 5
            )
        })
        .collect();
    let msg_sql = format!(
        r#"INSERT INTO crosschain_messages (id, init_timestamp, src_chain_id, dst_chain_id, src_tx_hash, dst_tx_hash) VALUES {}"#,
        msg_placeholders.join(", ")
    );
    interchain
        .execute(Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            &msg_sql,
            msg_values,
        ))
        .await
        .unwrap();

    // Bulk insert transfers: (id, message_id, sender_address, recipient_address).
    // Use 8 distinct 20-byte addresses so totalInterchainTransferUsers = 8.
    let mut transfer_rows: Vec<(i64, i64, Vec<u8>, Vec<u8>)> = Vec::new();
    let mut transfer_id: i64 = 1;
    for (i, (_ts, _src, _dst, _has_src, _has_dst, num_transfers)) in rows.iter().enumerate() {
        let message_id = (i + 1) as i64;
        for _ in 0..*num_transfers {
            let sender_idx = ((transfer_id - 1) % 8) as u8;
            let recipient_idx = ((transfer_id + 2) % 8) as u8;
            transfer_rows.push((
                transfer_id,
                message_id,
                vec![sender_idx; 20],
                vec![recipient_idx; 20],
            ));
            transfer_id += 1;
        }
    }
    if transfer_rows.is_empty() {
        return;
    }
    let t = transfer_rows.len();
    let transfer_placeholders: Vec<String> = (0..t)
        .map(|i| {
            let b = i * 4 + 1;
            format!("(${}, ${}, ${}, ${})", b, b + 1, b + 2, b + 3)
        })
        .collect();
    let mut transfer_values: Vec<Value> = Vec::with_capacity(t * 4);
    for (id, message_id, sender, recipient) in transfer_rows {
        transfer_values.push(Value::BigInt(Some(id)));
        transfer_values.push(Value::BigInt(Some(message_id)));
        transfer_values.push(Value::Bytes(Some(Box::new(sender))));
        transfer_values.push(Value::Bytes(Some(Box::new(recipient))));
    }
    let transfer_sql = format!(
        r#"INSERT INTO crosschain_transfers (id, message_id, sender_address, recipient_address) VALUES {}"#,
        transfer_placeholders.join(", ")
    );
    interchain
        .execute(Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            &transfer_sql,
            transfer_values,
        ))
        .await
        .unwrap();
}
