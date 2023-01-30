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

    let failed_block = blocks.last().unwrap();

    let txns = blocks[0..blocks.len() - 1]
        .iter()
        // make 1/3 of blocks empty
        .filter(|b| b.number.as_ref() % 3 != 1)
        // add 2 transactions to block
        .flat_map(|b| {
            [
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    &accounts,
                    0,
                ),
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    &accounts,
                    1,
                ),
            ]
        });
    transactions::Entity::insert_many(txns)
        .exec(blockscout)
        .await
        .unwrap();

    let failed_txns = vec![
        mock_failed_transaction(vec![123, 21], None, None),
        mock_failed_transaction(
            vec![123, 22],
            Some(failed_block),
            Some("dropped/replaced".into()),
        ),
    ];
    transactions::Entity::insert_many(failed_txns)
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
    let size = 1000 + (index as i32 * 15485863) % 5000;
    let gas_limit = if index <= 3 { 12_500_000 } else { 30_000_000 };
    blocks::ActiveModel {
        number: Set(index),
        hash: Set(index.to_le_bytes().to_vec()),
        timestamp: Set(NaiveDateTime::from_str(ts).unwrap()),
        consensus: Set(consensus),
        gas_limit: Set(Decimal::new(gas_limit, 0)),
        gas_used: Set(Default::default()),
        miner_hash: Set(Default::default()),
        nonce: Set(Default::default()),
        parent_hash: Set((index - 1).to_le_bytes().to_vec()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        size: Set(Some(size)),
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
    index: i32,
) -> transactions::ActiveModel {
    let block_number = block.number.as_ref().to_owned() as i32;
    let hash = vec![0, 0, 0, 0, block_number as u8, index as u8];
    let address_index = (block_number as usize) % address_list.len();
    let from_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let address_index = (block_number as usize + 1) % address_list.len();
    let to_address_hash = address_list[address_index].hash.as_ref().to_vec();

    transactions::ActiveModel {
        block_number: Set(Some(block_number)),
        block_hash: Set(Some(block.hash.as_ref().to_vec())),
        hash: Set(hash),
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
        index: Set(Some(index)),
        ..Default::default()
    }
}

fn mock_failed_transaction(
    hash: Vec<u8>,
    block: Option<&blocks::ActiveModel>,
    error: Option<String>,
) -> transactions::ActiveModel {
    let gas = Decimal::new(21_000, 0);
    transactions::ActiveModel {
        block_number: Set(block.map(|block| *block.number.as_ref() as i32)),
        block_hash: Set(block.map(|block| block.hash.as_ref().to_vec())),
        cumulative_gas_used: Set(block.map(|_| Default::default())),
        gas_used: Set(block.map(|_| gas)),
        index: Set(block.map(|_| Default::default())),
        error: Set(error),
        hash: Set(hash),
        gas_price: Set(Decimal::new(1_123_456_789, 0)),
        gas: Set(gas),
        input: Set(Default::default()),
        nonce: Set(Default::default()),
        r: Set(Default::default()),
        s: Set(Default::default()),
        v: Set(Default::default()),
        value: Set(Default::default()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        from_address_hash: Set(vec![]),
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
