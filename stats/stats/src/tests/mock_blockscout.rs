use blockscout_db::entity::{addresses, blocks, transactions};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{prelude::Decimal, DatabaseConnection, EntityTrait, Set};
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
    .map(|(ind, ts)| mock_block(ind as i64, ts))
    .collect::<Vec<_>>();
    blocks::Entity::insert_many(block_timestamps.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let transactios = block_timestamps
        .iter()
        // make 1/3 of blocks empty
        .filter(|b| b.number.as_ref() % 3 != 1)
        .map(|b| {
            mock_transaction(
                b,
                21_000,
                (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
            )
        });
    transactions::Entity::insert_many(transactios)
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

fn mock_transaction(
    block: &blocks::ActiveModel,
    gas: i64,
    gas_price: i64,
) -> transactions::ActiveModel {
    let block_number = block.number.as_ref().to_owned() as i32;
    transactions::ActiveModel {
        block_number: Set(Some(block_number)),
        block_hash: Set(Some(block.hash.as_ref().to_vec())),
        hash: Set(block_number.to_le_bytes().to_vec()),
        gas_price: Set(Decimal::new(gas_price, 0)),
        gas: Set(Decimal::new(gas, 0)),
        input: Set(Default::default()),
        nonce: Set(Default::default()),
        r: Set(Default::default()),
        s: Set(Default::default()),
        v: Set(Default::default()),
        value: Set(Default::default()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        from_address_hash: Set(Default::default()),
        cumulative_gas_used: Set(Some(Default::default())),
        gas_used: Set(Some(Default::default())),
        index: Set(Some(Default::default())),
        ..Default::default()
    }
}
