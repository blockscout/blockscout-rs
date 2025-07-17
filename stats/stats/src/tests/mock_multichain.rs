#![cfg(any(feature = "test-utils", test))]

use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime};
use multichain_aggregator_entity::{addresses, interop_messages};
use sea_orm::{DatabaseConnection, EntityTrait, Set};

pub async fn fill_mock_multichain_data(multichain: &DatabaseConnection, max_date: NaiveDate) {
    let accounts = mock_addresses();
    let interop_messages = mock_interop_messages(&accounts, max_date);
    interop_messages::Entity::insert_many(interop_messages.clone())
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

fn mock_interop_messages(
    accounts: &[addresses::ActiveModel],
    max_date: NaiveDate,
) -> Vec<interop_messages::ActiveModel> {
    vec![
        "2022-11-09T23:59:59",
        "2022-11-10T00:00:00",
        "2022-11-10T12:00:00",
    ]
    .into_iter()
    .map(|val| NaiveDateTime::from_str(val).unwrap())
    .filter(|ts| ts.date() <= max_date)
    .enumerate()
    .map(|(i, ts)| mock_interop_message(i, ts, accounts))
    .collect()
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
        ..Default::default()
    }
}
