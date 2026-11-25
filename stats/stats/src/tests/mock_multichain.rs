// SPDX-License-Identifier: LicenseRef-Blockscout

#![cfg(any(feature = "test-utils", test))]

use std::str::FromStr;

use chrono::NaiveDate;
use multichain_aggregator_entity::{block_ranges, chains, counters_global_imported};
use sea_orm::{ActiveValue::NotSet, DatabaseConnection, EntityTrait, Set};

pub async fn fill_mock_multichain_data(multichain: &DatabaseConnection, max_date: NaiveDate) {
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
        ("2023-02-04", 1, 10, 46, 170),
        ("2023-02-04", 2, 20, 55, 300),
        ("2023-02-04", 3, 30, 109, 450),
        ("2023-02-03", 1, 4, 36, 160),
        ("2023-02-03", 2, 7, 35, 290),
        ("2023-02-03", 3, 38, 79, 422),
        ("2023-02-02", 1, 18, 32, 155),
        ("2023-02-02", 2, 3, 28, 250),
        ("2023-02-02", 3, 4, 41, 420),
        ("2023-01-01", 1, 3, 14, 150),
        ("2023-01-01", 2, 3, 25, 250),
        ("2023-01-01", 3, 4, 37, 350),
        ("2022-12-28", 1, 11, 11, 111),
        ("2022-12-28", 2, 22, 22, 222),
        ("2022-12-28", 3, 33, 33, 333),
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

pub async fn add_mock_multichain_data(indexer: &DatabaseConnection, at_date: NaiveDate) {
    let counters_global_imported_new = mock_counter_global_imported(1, at_date, 10, 189, 1234);

    counters_global_imported::Entity::insert_many([counters_global_imported_new])
        .exec(indexer)
        .await
        .unwrap();
}
