use blockscout_db::entity::{addresses, blocks, tokens, transactions};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{prelude::Decimal, DatabaseConnection, EntityTrait, Set};
use std::str::FromStr;

pub async fn fill_mock_blockscout_data(blockscout: &DatabaseConnection, max_date: &str) {
    addresses::Entity::insert(addresses::ActiveModel {
        hash: Set(vec![]),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    })
    .exec(blockscout)
    .await
    .unwrap();

    let blocks = vec![
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
    .map(|(ind, ts)| mock_block(ind as i64, ts, true))
    .collect::<Vec<_>>();
    blocks::Entity::insert_many(blocks.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let accounts = (1..9).into_iter().map(mock_address).collect::<Vec<_>>();
    addresses::Entity::insert_many(accounts.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let tokens = accounts
        .iter()
        .take(4)
        .map(|addr| mock_token(addr.hash.as_ref().clone()));
    tokens::Entity::insert_many(tokens)
        .exec(blockscout)
        .await
        .unwrap();

    let txns = blocks
        .iter()
        // make 1/3 of blocks empty
        .filter(|b| b.number.as_ref() % 3 != 1)
        .map(|b| {
            mock_transaction(
                b,
                21_000,
                (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                &accounts,
            )
        });
    transactions::Entity::insert_many(txns)
        .exec(blockscout)
        .await
        .unwrap();

    let useless_blocks = [
        "1970-01-01T00:00:00",
        "2010-11-01T23:59:59",
        "2022-11-08T12:00:00",
    ]
    .into_iter()
    .filter(|val| {
        NaiveDateTime::from_str(val).unwrap().date() <= NaiveDate::from_str(max_date).unwrap()
    })
    .enumerate()
    .map(|(ind, ts)| mock_block((ind + blocks.len()) as i64, ts, false));
    blocks::Entity::insert_many(useless_blocks)
        .exec(blockscout)
        .await
        .unwrap();
}

fn mock_block(index: i64, ts: &str, consensus: bool) -> blocks::ActiveModel {
    blocks::ActiveModel {
        number: Set(index),
        hash: Set(index.to_le_bytes().to_vec()),
        timestamp: Set(NaiveDateTime::from_str(ts).unwrap()),
        consensus: Set(consensus),
        gas_limit: Set(Default::default()),
        gas_used: Set(Default::default()),
        miner_hash: Set(Default::default()),
        nonce: Set(Default::default()),
        parent_hash: Set((index - 1).to_le_bytes().to_vec()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    }
}

fn mock_address(seed: u8) -> addresses::ActiveModel {
    let hash = std::iter::repeat(seed).take(20).collect();
    addresses::ActiveModel {
        hash: Set(hash),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    }
}

fn mock_transaction(
    block: &blocks::ActiveModel,
    gas: i64,
    gas_price: i64,
    address_list: &Vec<addresses::ActiveModel>,
) -> transactions::ActiveModel {
    let block_number = block.number.as_ref().to_owned() as i32;

    let address_index = (block_number as usize) % address_list.len();
    let from_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let address_index = (block_number as usize + 1) % address_list.len();
    let to_address_hash = address_list[address_index].hash.as_ref().to_vec();

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
        from_address_hash: Set(from_address_hash),
        to_address_hash: Set(Some(to_address_hash)),
        cumulative_gas_used: Set(Some(Default::default())),
        gas_used: Set(Some(Decimal::new(gas, 0))),
        index: Set(Some(Default::default())),
        ..Default::default()
    }
}

fn mock_token(hash: Vec<u8>) -> tokens::ActiveModel {
    tokens::ActiveModel {
        r#type: Set(Default::default()),
        contract_address_hash: Set(hash),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    }
}
