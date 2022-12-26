use blockscout_db::entity::{addresses, blocks};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use std::str::FromStr;

pub async fn fill_mock_blockscout_data(blockscout: &DatabaseConnection, max_date: &str) {
    // TODO: add transactions and tokens
    addresses::Entity::insert(addresses::ActiveModel {
        hash: Set(vec![]),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    })
    .exec(blockscout)
    .await
    .unwrap();

    let block_timestamps = vec![
        "2022-11-09T23:59:59",
        "2022-11-10T00:00:00",
        "2022-11-10T12:00:00",
        "2022-11-10T23:59:59",
        "2022-11-11T00:00:00",
        "2022-11-11T12:00:00",
        "2022-11-11T15:00:00",
        "2022-11-11T23:59:59",
        "2022-11-12T00:00:00",
    ]
    .into_iter()
    .filter(|val| {
        NaiveDateTime::from_str(val).unwrap().date() <= NaiveDate::from_str(max_date).unwrap()
    })
    .enumerate()
    .map(|(ind, ts)| mock_block(ind as i64, ts));
    blocks::Entity::insert_many(block_timestamps)
        .exec(blockscout)
        .await
        .unwrap();
}

fn mock_block(index: i64, ts: &str) -> blocks::ActiveModel {
    blocks::ActiveModel {
        number: Set(index),
        hash: Set(index.to_le_bytes().to_vec()),
        timestamp: Set(NaiveDateTime::from_str(ts).unwrap()),
        consensus: Set(Default::default()),
        gas_limit: Set(Default::default()),
        gas_used: Set(Default::default()),
        miner_hash: Set(Default::default()),
        nonce: Set(Default::default()),
        parent_hash: Set(Default::default()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    }
}
