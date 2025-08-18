#![cfg(any(feature = "test-utils", test))]

use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use multichain_aggregator_entity::{
    addresses, block_ranges, chains, counters_global_imported, interop_messages,
    interop_messages_transfers,
};
use sea_orm::{ActiveValue::NotSet, DatabaseConnection, EntityTrait, Set};

pub async fn fill_mock_multichain_data(multichain: &DatabaseConnection, max_date: NaiveDate) {
    let accounts = mock_addresses();
    let (messages, transfers) = mock_interop_messages_with_transfers(&accounts, max_date);
    interop_messages::Entity::insert_many(messages.clone())
        .exec(multichain)
        .await
        .unwrap();
    interop_messages_transfers::Entity::insert_many(transfers.clone())
        .exec(multichain)
        .await
        .unwrap();
    let chains = mock_chains();
    chains::Entity::insert_many(chains)
        .exec(multichain)
        .await
        .unwrap();
    let block_ranges = mock_block_ranges();
    block_ranges::Entity::insert_many(block_ranges)
        .exec(multichain)
        .await
        .unwrap();
    let counters_global_imported = mock_counters_global_imported(max_date);
    counters_global_imported::Entity::insert_many(counters_global_imported)
        .exec(multichain)
        .await
        .unwrap();
}

fn mock_addresses() -> Vec<addresses::ActiveModel> {
    (0..3).map(mock_address).collect()
}

fn mock_address(seed: i64) -> addresses::ActiveModel {
    let mut hash = seed.to_le_bytes().to_vec();
    hash.extend(std::iter::repeat_n(0, 32 - hash.len()));
    addresses::ActiveModel {
        hash: Set(hash),
        chain_id: Set(1),
        ens_name: Set(None),
        contract_name: Set(None),
        token_name: Set(None),
        token_type: Set(None),
        is_contract: Set(false),
        is_verified_contract: Set(false),
        is_token: Set(false),
        created_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
}

fn mock_interop_messages_with_transfers(
    accounts: &[addresses::ActiveModel],
    max_date: NaiveDate,
) -> (
    Vec<interop_messages::ActiveModel>,
    Vec<interop_messages_transfers::ActiveModel>,
) {
    let messages: Vec<interop_messages::ActiveModel> = vec![
        "2022-11-09T23:59:59",
        "2022-11-10T00:00:00",
        "2022-11-10T12:00:00",
        "2022-11-15T12:00:00",
        "2022-11-16T12:00:00",
        "2022-11-18T12:00:00",
    ]
    .into_iter()
    .map(|val| NaiveDateTime::from_str(val).unwrap())
    .filter(|ts| ts.date() <= max_date)
    .enumerate()
    .map(|(i, ts)| mock_interop_message(i, ts, accounts))
    .collect();

    // link the first 3 messages with transfers
    let transfers: Vec<interop_messages_transfers::ActiveModel> = messages
        .iter()
        .take(3)
        .enumerate()
        .map(|(i, _)| mock_interop_message_transfer(i as i64 + 1, accounts))
        .collect();

    (messages, transfers)
}

fn mock_interop_message(
    index: usize,
    ts: NaiveDateTime,
    accounts: &[addresses::ActiveModel],
) -> interop_messages::ActiveModel {
    let account_index = index % accounts.len();
    let from_address_hash = accounts[account_index].hash.as_ref().to_vec();
    let account_index = (account_index + 1) % accounts.len();
    let to_address_hash = accounts[account_index].hash.as_ref().to_vec();

    interop_messages::ActiveModel {
        id: Set(index as i64 + 1),
        sender_address_hash: Set(Some(from_address_hash)),
        target_address_hash: Set(Some(to_address_hash)),
        nonce: Set(index as i64),
        init_chain_id: Set(1),
        init_transaction_hash: Set(None),
        timestamp: Set(Some(ts)),
        relay_chain_id: Set(1),
        relay_transaction_hash: Set(None),
        payload: Set(None),
        failed: Set(None),
        created_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
}

fn mock_interop_message_transfer(
    interop_message_id: i64,
    accounts: &[addresses::ActiveModel],
) -> interop_messages_transfers::ActiveModel {
    let from_index = (interop_message_id as usize) % accounts.len();
    let to_index = (from_index + 1) % accounts.len();

    interop_messages_transfers::ActiveModel {
        interop_message_id: Set(interop_message_id),
        token_address_hash: Set(Some(vec![0u8; 20])),
        from_address_hash: Set(accounts[from_index].hash.as_ref().to_vec()),
        to_address_hash: Set(accounts[to_index].hash.as_ref().to_vec()),
        amount: Set("1".parse().unwrap()),
    }
}

fn mock_chains() -> Vec<chains::ActiveModel> {
    vec![(1, "Ethereum"), (2, "Ethereum 2"), (3, "Ethereum 3")]
        .into_iter()
        .map(|(id, name)| chains::ActiveModel {
            id: Set(id),
            name: Set(Some(name.to_string())),
            ..Default::default()
        })
        .collect()
}

fn mock_block_ranges() -> Vec<block_ranges::ActiveModel> {
    vec![(1, 1, 10), (2, 1, 12345), (3, 100, 150)]
        .into_iter()
        .map(
            |(chain_id, min_block, max_block)| block_ranges::ActiveModel {
                chain_id: Set(chain_id),
                min_block_number: Set(min_block),
                max_block_number: Set(max_block),
                ..Default::default()
            },
        )
        .collect()
}

fn mock_counters_global_imported(
    max_date: NaiveDate,
) -> Vec<counters_global_imported::ActiveModel> {
    // each tuple includes: (date, chain_id, daily_txns, total_txns, total_addresses)
    let dates_and_txns = vec![
        ("2022-08-06", 1, 10, 46, 170),
        ("2022-08-06", 2, 20, 55, 300),
        ("2022-08-06", 3, 30, 109, 450),
        ("2022-08-05", 1, 4, 36, 160),
        ("2022-08-05", 2, 7, 35, 290),
        ("2022-08-05", 3, 38, 79, 422),
        ("2022-08-04", 1, 18, 32, 155),
        ("2022-08-04", 2, 3, 28, 250),
        ("2022-08-04", 3, 4, 41, 420),
        ("2022-07-01", 1, 3, 14, 150),
        ("2022-07-01", 2, 3, 25, 250),
        ("2022-07-01", 3, 4, 37, 350),
        ("2022-06-28", 1, 11, 11, 111),
        ("2022-06-28", 2, 22, 22, 222),
        ("2022-06-28", 3, 33, 33, 333),
    ];

    dates_and_txns
        .into_iter()
        .filter_map(
            |(date_str, chain_id, daily_txns, total_txns, total_addresses)| {
                let date = NaiveDate::from_str(date_str).unwrap();
                if date <= max_date {
                    Some(mock_counter_global_imported(
                        chain_id,
                        date,
                        daily_txns,
                        total_txns,
                        total_addresses,
                    ))
                } else {
                    None
                }
            },
        )
        .collect()
}

fn mock_counter_global_imported(
    chain_id: i64,
    date: NaiveDate,
    daily_transactions: i64,
    total_transactions: i64,
    total_addresses: i64,
) -> counters_global_imported::ActiveModel {
    counters_global_imported::ActiveModel {
        id: NotSet,
        chain_id: Set(chain_id),
        date: Set(date),
        daily_transactions_number: Set(Some(daily_transactions)),
        total_transactions_number: Set(Some(total_transactions)),
        total_addresses_number: Set(Some(total_addresses)),
        created_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
}

pub async fn imitate_reindex_multichain(indexer: &DatabaseConnection) {
    let counters_global_imported_new =
        mock_counter_global_imported(1, NaiveDate::from_str("2022-08-15").unwrap(), 10, 189, 1234);

    counters_global_imported::Entity::insert_many([counters_global_imported_new])
        .exec(indexer)
        .await
        .unwrap();
}
