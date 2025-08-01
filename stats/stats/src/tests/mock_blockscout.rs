#![cfg(any(feature = "test-utils", test))]

use blockscout_db::entity::{
    address_coin_balances_daily, addresses, block_rewards, blocks, internal_transactions,
    migrations_status,
    sea_orm_active_enums::{EntryPointVersion, SponsorType},
    signed_authorizations, smart_contracts, tokens, transactions, user_operations,
};
use chrono::{NaiveDate, NaiveDateTime, TimeDelta};
use hex_literal::hex;
use itertools::Itertools;
use rand::{Rng, SeedableRng};
use sea_orm::{ActiveValue::NotSet, DatabaseConnection, EntityTrait, Set, prelude::Decimal};
use std::str::FromStr;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::lines::{ATTRIBUTES_DEPOSITED_FROM_HASH, ATTRIBUTES_DEPOSITED_TO_HASH};

pub async fn default_mock_blockscout_api() -> MockServer {
    mock_blockscout_api(
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        })),
        Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(true))),
    )
    .await
}

pub async fn mock_blockscout_api(
    blockscout_indexing_status_response: ResponseTemplate,
    user_ops_indexing_status_response: Option<ResponseTemplate>,
) -> MockServer {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/main-page/indexing-status"))
        .respond_with(blockscout_indexing_status_response)
        .mount(&mock_server)
        .await;

    if let Some(response) = user_ops_indexing_status_response {
        Mock::given(method("GET"))
            .and(path("/api/v2/proxy/account-abstraction/status"))
            .respond_with(response)
            .mount(&mock_server)
            .await;
    }
    mock_server
}

pub fn user_ops_status_response_json(past_finished: bool) -> serde_json::Value {
    serde_json::json!({
        "finished_past_indexing": past_finished,
        "v06": {
            "enabled": true,
            "live": false,
            "past_db_logs_indexing_finished": false,
            "past_rpc_logs_indexing_finished": false
        },
        "v07": {
            "enabled": true,
            "live": false,
            "past_db_logs_indexing_finished": false,
            "past_rpc_logs_indexing_finished": false
        }
    })
}

pub async fn fill_mock_blockscout_data(blockscout: &DatabaseConnection, max_date: NaiveDate) {
    addresses::Entity::insert_many([
        addresses::ActiveModel {
            hash: Set(vec![]),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        },
        addresses::ActiveModel {
            hash: Set(vec![0; 20]),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        },
    ])
    .exec(blockscout)
    .await
    .unwrap();

    let blocks = mock_blocks(max_date);
    blocks::Entity::insert_many(blocks.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let accounts = mock_addresses();
    addresses::Entity::insert_many(accounts.clone())
        .exec(blockscout)
        .await
        .unwrap();
    let attributes_deposited_transaction_accounts =
        mock_attributes_deposited_transaction_addresses();
    addresses::Entity::insert_many(attributes_deposited_transaction_accounts)
        .exec(blockscout)
        .await
        .unwrap();

    let contracts = (21..40)
        .map(|seed| mock_address(seed, true, false))
        .collect::<Vec<_>>();
    addresses::Entity::insert_many(contracts.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let verified_contracts = (41..44)
        .map(|seed| mock_address(seed, true, true))
        .collect::<Vec<_>>();
    addresses::Entity::insert_many(verified_contracts.clone())
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

    let txns = mock_transactions(&blocks, &accounts);
    transactions::Entity::insert_many(txns.clone())
        .exec(blockscout)
        .await
        .unwrap();

    let user_ops_with_txns = mock_user_operations(&blocks, &accounts);
    let (user_ops_txns, user_ops): (Vec<_>, Vec<_>) = user_ops_with_txns.into_iter().unzip();
    transactions::Entity::insert_many(user_ops_txns)
        .exec(blockscout)
        .await
        .unwrap();
    user_operations::Entity::insert_many(user_ops)
        .exec(blockscout)
        .await
        .unwrap();

    let signed_authorizations = mock_signed_authorizations(&txns, &contracts, &accounts);
    signed_authorizations::Entity::insert_many(signed_authorizations)
        .exec(blockscout)
        .await
        .unwrap();

    let contract_creation_txns = contracts
        .iter()
        .chain(verified_contracts.iter())
        .enumerate()
        .map(|(i, contract)| {
            mock_transaction(
                &blocks[i % (blocks.len() - 1)],
                21_000,
                1_123_456_789,
                &accounts,
                (4 + i) as i32,
                TxType::ContractCreation(contract.hash.as_ref().clone()),
            )
        })
        .collect::<Vec<_>>();
    transactions::Entity::insert_many(contract_creation_txns.clone())
        .exec(blockscout)
        .await
        .unwrap();

    // contract created during internal transaction
    {
        let contract_in_internal_txn = mock_address(100, true, false);
        addresses::Entity::insert(contract_in_internal_txn.clone())
            .exec(blockscout)
            .await
            .unwrap();
        let internal_txn = mock_internal_transaction(
            &contract_creation_txns[0],
            0,
            Some(&contract_in_internal_txn),
        );
        internal_transactions::Entity::insert(internal_txn)
            .exec(blockscout)
            .await
            .unwrap();
    }

    let verified_date = vec![
        "2022-11-14T12:00:00",
        "2022-11-15T15:00:00",
        "2022-11-16T23:59:59",
        // not used
        // "2022-11-17T00:00:00",
    ]
    .into_iter()
    .map(|val| NaiveDateTime::from_str(val).unwrap());
    assert!(verified_date.len() >= verified_contracts.len());
    let smart_contracts = verified_contracts
        .iter()
        .zip(verified_date)
        .map(|(contract, verified_at)| mock_smart_contract(contract, verified_at));
    smart_contracts::Entity::insert_many(smart_contracts)
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
        "2022-11-13T12:00:00",
    ]
    .into_iter()
    .filter(|val| NaiveDateTime::from_str(val).unwrap().date() <= max_date)
    .enumerate()
    .map(|(ind, ts)| {
        mock_block(
            (ind + blocks.len()) as i64,
            NaiveDateTime::from_str(ts).unwrap(),
            false,
        )
    })
    .collect_vec();
    blocks::Entity::insert_many(useless_blocks.clone())
        .exec(blockscout)
        .await
        .unwrap();

    if let Some(last_useless_block) = useless_blocks.last() {
        let (useless_txn, useless_user_op) =
            mock_user_operation(last_useless_block, 21_000, 1_123_456_789, &accounts, 0);
        transactions::Entity::insert(useless_txn)
            .exec(blockscout)
            .await
            .unwrap();
        user_operations::Entity::insert(useless_user_op)
            .exec(blockscout)
            .await
            .unwrap();
    }

    // 10000 eth
    let sum = 10_000_000_000_000_000_000_000_i128;
    let addrs: Vec<_> = std::iter::once(vec![0; 20])
        .chain(
            accounts
                .iter()
                .map(|account| account.hash.as_ref().to_vec()),
        )
        .collect();

    let addr_balance_daily: Vec<_> = ["2022-11-08", "2022-11-09", "2022-11-10", "2022-11-11"]
        .into_iter()
        .map(|d| NaiveDate::from_str(d).unwrap())
        .enumerate()
        .flat_map(|(i, day)| {
            let mut cur_sum = sum;
            let values: Vec<_> = addrs
                .clone()
                .into_iter()
                .enumerate()
                .map(|(j, addr)| {
                    let value = if i == 0 {
                        None
                    } else if j == addrs.len() - 1 {
                        Some(cur_sum)
                    } else if (i + j) % 5 != 0 {
                        let value = cur_sum / (7 - i as i128);
                        cur_sum -= value;
                        Some(value)
                    } else {
                        None
                    };
                    (addr, day, value)
                })
                .collect();
            values
                .into_iter()
                .map(|(addr, day, value)| mock_address_coin_balance_daily(addr, day, value))
        })
        .collect();

    address_coin_balances_daily::Entity::insert_many(addr_balance_daily)
        .exec(blockscout)
        .await
        .unwrap();

    let rewards = blocks.iter().enumerate().flat_map(|(i, block)| {
        mock_block_rewards(i as u8, block.hash.as_ref().to_vec(), &accounts, None)
    });

    block_rewards::Entity::insert_many(rewards)
        .exec(blockscout)
        .await
        .unwrap();

    let migrations = vec![
        ("denormalization", Some(true)),
        ("ctb_token_type", Some(false)),
        ("tb_token_type", None),
    ]
    .into_iter()
    .map(|(name, status)| mock_migration(name, status));

    migrations_status::Entity::insert_many(migrations)
        .exec(blockscout)
        .await
        .unwrap();
}

/// Expected `max_date` to be the same that was passed to `fill_mock_blockscout_data`
pub async fn imitate_reindex(blockscout: &DatabaseConnection, max_date: NaiveDate) {
    let existing_blocks = mock_blocks(max_date);
    let existing_accounts = mock_addresses();
    let new_txns: Vec<_> = reindexing_mock_txns(&existing_blocks, &existing_accounts);
    transactions::Entity::insert_many(new_txns)
        .exec(blockscout)
        .await
        .unwrap();
}

/// `block_times` - block time for each block from the 2nd to the latest.
///
/// `<number of block times> = <number of inserted blocks> + 1`
pub async fn fill_many_blocks(
    blockscout: &DatabaseConnection,
    latest_block_time: NaiveDateTime,
    block_times: &[TimeDelta],
) {
    let mut blocks_timestamps_reversed = Vec::with_capacity(block_times.len() + 1);
    blocks_timestamps_reversed.push(latest_block_time);
    for time_diff in block_times.iter().rev() {
        let next_timestamp = *blocks_timestamps_reversed.last().unwrap() - *time_diff;
        blocks_timestamps_reversed.push(next_timestamp);
    }
    let blocks_timestamps = blocks_timestamps_reversed.into_iter().rev();
    let blocks = blocks_timestamps
        .enumerate()
        .map(|(ind, ts)| mock_block(ind as i64, ts, true))
        .collect::<Vec<_>>();
    blocks::Entity::insert_many(blocks.clone())
        .exec(blockscout)
        .await
        .unwrap();
}

fn mock_blocks(max_date: NaiveDate) -> Vec<blocks::ActiveModel> {
    vec![
        "2022-11-09T23:59:59",
        "2022-11-10T00:00:00",
        "2022-11-10T12:00:00",
        "2022-11-10T23:59:59",
        "2022-11-11T00:00:00",
        "2022-11-11T12:00:00",
        "2022-11-11T15:00:00",
        "2022-11-11T23:59:59",
        "2022-11-12T00:00:00",
        "2022-12-01T10:00:00",
        "2023-01-01T10:00:00",
        "2023-02-01T10:00:00",
        "2023-03-01T10:00:00",
    ]
    .into_iter()
    .filter(|val| NaiveDateTime::from_str(val).unwrap().date() <= max_date)
    .enumerate()
    .map(|(ind, ts)| mock_block(ind as i64, NaiveDateTime::from_str(ts).unwrap(), true))
    .collect::<Vec<_>>()
}

fn mock_block(index: i64, ts: NaiveDateTime, consensus: bool) -> blocks::ActiveModel {
    let size = (1000 + (index * 15485863) % 5000) as i32;
    let gas_limit = if index <= 3 { 12_500_000 } else { 30_000_000 };
    blocks::ActiveModel {
        number: Set(index),
        hash: Set(index.to_le_bytes().to_vec()),
        timestamp: Set(ts),
        consensus: Set(consensus),
        gas_limit: Set(Decimal::new(gas_limit, 0)),
        gas_used: Set(Decimal::from(size * 10)),
        miner_hash: Set(Default::default()),
        nonce: Set(Default::default()),
        parent_hash: Set((index - 1).to_le_bytes().to_vec()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        size: Set(Some(size)),
        ..Default::default()
    }
}

fn mock_addresses() -> Vec<addresses::ActiveModel> {
    (1..9)
        .map(|seed| mock_address(seed, false, false))
        .collect::<Vec<_>>()
}

fn mock_address(seed: i64, is_contract: bool, is_verified: bool) -> addresses::ActiveModel {
    let mut hash = seed.to_le_bytes().to_vec();
    hash.extend(std::iter::repeat_n(0, 32 - hash.len()));
    let contract_code = is_contract.then(|| vec![60u8, 80u8]);
    let verified = is_contract.then_some(is_verified);
    addresses::ActiveModel {
        hash: Set(hash),
        contract_code: Set(contract_code),
        verified: Set(verified),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
enum TxType {
    Transfer,
    ContractCall,
    ContractCreation(Vec<u8>),
}

impl TxType {
    fn needs_input(&self) -> bool {
        matches!(self, TxType::ContractCall | TxType::ContractCreation(_))
    }
    fn needs_value(&self) -> bool {
        matches!(self, TxType::Transfer)
    }
}

fn mock_transactions(
    blocks: &[blocks::ActiveModel],
    accounts: &[addresses::ActiveModel],
) -> Vec<transactions::ActiveModel> {
    blocks[0..blocks.len() - 1]
        .iter()
        // make 1/3 of blocks empty
        .filter(|b| b.number.as_ref() % 3 != 1)
        // add 3 transactions to block
        .flat_map(|b| {
            [
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    accounts,
                    0,
                    TxType::Transfer,
                ),
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    accounts,
                    1,
                    TxType::Transfer,
                ),
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    accounts,
                    2,
                    TxType::ContractCall,
                ),
            ]
        })
        .chain([mock_attributes_deposit_transaction(
            blocks.last().unwrap(),
            43_887,
            // just in case the block number is `% 3 != 1`
            3,
        )])
        .collect()
}

fn reindexing_mock_txns(
    blocks: &[blocks::ActiveModel],
    accounts: &[addresses::ActiveModel],
) -> Vec<transactions::ActiveModel> {
    // not sure if can actually happen in blockscout, but let's
    // say empty blocks got their own transactions
    blocks[0..blocks.len() - 1]
        .iter()
        // fill in the empty blocks
        .filter(|b| b.number.as_ref() % 3 == 1)
        // add 2 transactions to block
        .flat_map(|b| {
            [
                mock_transaction(
                    b,
                    23_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    accounts,
                    0,
                    TxType::Transfer,
                ),
                mock_transaction(
                    b,
                    23_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    accounts,
                    1,
                    TxType::Transfer,
                ),
            ]
        })
        .collect()
}

fn mock_transaction(
    block: &blocks::ActiveModel,
    gas: i64,
    gas_price: i64,
    address_list: &[addresses::ActiveModel],
    index: i32,
    tx_type: TxType,
) -> transactions::ActiveModel {
    let block_number = block.number.as_ref().to_owned() as i32;
    let hash = vec![0, 0, 0, 0, block_number as u8, index as u8];
    let address_index = (block_number as usize) % address_list.len();
    let from_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let address_index = (block_number as usize + 1) % address_list.len();
    let to_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let input = if tx_type.needs_input() {
        vec![60u8, 80u8]
    } else {
        vec![]
    };
    let value = if tx_type.needs_value() {
        1_000_000_000_000
    } else {
        0
    };
    let created_contract_code_indexed_at = match &tx_type {
        TxType::ContractCreation(_) => Some(
            block
                .timestamp
                .as_ref()
                .checked_add_signed(TimeDelta::minutes(10))
                .unwrap(),
        ),
        _ => None,
    };
    let created_contract_address_hash = match tx_type {
        TxType::ContractCreation(contract_address) => Some(contract_address),
        _ => None,
    };

    transactions::ActiveModel {
        block_number: Set(Some(block_number)),
        block_hash: Set(Some(block.hash.as_ref().to_vec())),
        block_timestamp: Set(Some(*block.timestamp.as_ref())),
        block_consensus: Set(Some(*block.consensus.as_ref())),
        hash: Set(hash),
        gas_price: Set(Some(Decimal::new(gas_price, 0))),
        gas: Set(Decimal::new(gas, 0)),
        input: Set(input),
        nonce: Set(Default::default()),
        r: Set(Default::default()),
        s: Set(Default::default()),
        v: Set(Default::default()),
        value: Set(Decimal::new(value, 0)),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        from_address_hash: Set(from_address_hash),
        to_address_hash: Set(Some(to_address_hash)),
        cumulative_gas_used: Set(Some(Default::default())),
        gas_used: Set(Some(Decimal::new(gas, 0))),
        index: Set(Some(index)),
        status: Set(Some(1)),
        created_contract_address_hash: Set(created_contract_address_hash),
        created_contract_code_indexed_at: Set(created_contract_code_indexed_at),
        ..Default::default()
    }
}

fn mock_attributes_deposited_transaction_addresses() -> Vec<addresses::ActiveModel> {
    [ATTRIBUTES_DEPOSITED_FROM_HASH, ATTRIBUTES_DEPOSITED_TO_HASH]
        .into_iter()
        .map(|hash| {
            let hash = hex::decode(hash).unwrap();
            let contract_code = vec![60u8, 80u8];
            addresses::ActiveModel {
                hash: Set(hash),
                contract_code: Set(Some(contract_code)),
                verified: Set(Some(false)),
                inserted_at: Set(Default::default()),
                updated_at: Set(Default::default()),
                ..Default::default()
            }
        })
        .collect_vec()
}

// https://specs.optimism.io/protocol/deposits.html#l1-attributes-deposited-transaction
fn mock_attributes_deposit_transaction(
    block: &blocks::ActiveModel,
    gas: i64,
    index: i32,
) -> transactions::ActiveModel {
    let mut address_list = mock_attributes_deposited_transaction_addresses();
    // adjust choice of from/to in `mock_transaction`
    if block.number.as_ref() % 2 == 1 {
        address_list.reverse();
    };
    mock_transaction(block, gas, 0, &address_list, index, TxType::ContractCall)
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
        block_timestamp: Set(block.map(|b| *b.timestamp.as_ref())),
        block_consensus: Set(block.map(|b| *b.consensus.as_ref())),
        cumulative_gas_used: Set(block.map(|_| Default::default())),
        gas_used: Set(block.map(|_| gas)),
        index: Set(block.map(|_| Default::default())),
        error: Set(error),
        hash: Set(hash),
        gas_price: Set(Some(Decimal::new(1_123_456_789, 0))),
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
        status: Set(Some(0)),
        ..Default::default()
    }
}

fn mock_address_coin_balance_daily(
    addr: Vec<u8>,
    day: NaiveDate,
    value: Option<i128>,
) -> address_coin_balances_daily::ActiveModel {
    address_coin_balances_daily::ActiveModel {
        address_hash: Set(addr),
        day: Set(day),
        value: Set(value.map(Decimal::from)),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
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

fn mock_block_rewards(
    random_seed: u8,
    block_hash: Vec<u8>,
    addresses_pool: &[addresses::ActiveModel],
    amount_overwrite: Option<Decimal>,
) -> Vec<block_rewards::ActiveModel> {
    // `Vec` because it's possible to have multiple rewards for a single
    // block in some chains.
    // E.g. in presence of additional rewards
    let mut rewards = vec![];
    let seed = [random_seed; 32];
    let mut rng = rand::prelude::StdRng::from_seed(seed);
    let n_rewards = rng.gen_range(1..=3);
    for i in 0..n_rewards {
        let amount = amount_overwrite
            .unwrap_or(Decimal::from(rng.gen_range(0..10)) * Decimal::try_from(5e17).unwrap());
        rewards.push(block_rewards::ActiveModel {
            address_hash: Set(addresses_pool
                .get(i % (addresses_pool.len() / 2))
                .unwrap()
                .hash
                .as_ref()
                .to_vec()),
            address_type: Set("".into()),
            block_hash: Set(block_hash.clone()),
            reward: Set(Some(amount)),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
        });
    }
    rewards
}

fn mock_smart_contract(
    contract: &addresses::ActiveModel,
    verified_at: NaiveDateTime,
) -> smart_contracts::ActiveModel {
    smart_contracts::ActiveModel {
        address_hash: Set(contract.hash.as_ref().clone()),
        name: Set(Default::default()),
        compiler_version: Set(Default::default()),
        contract_source_code: Set(Default::default()),
        abi: Set(Default::default()),
        contract_code_md5: Set(Default::default()),
        inserted_at: Set(verified_at),
        updated_at: Set(Default::default()),
        optimization: Set(false),
        ..Default::default()
    }
}

fn mock_internal_transaction(
    tx: &transactions::ActiveModel,
    index: i32,
    contract: Option<&addresses::ActiveModel>,
) -> internal_transactions::ActiveModel {
    let created_contract_address_hash = match contract {
        Some(contract) => Set(Some(contract.hash.as_ref().clone())),
        None => NotSet,
    };

    internal_transactions::ActiveModel {
        index: Set(index),
        transaction_hash: Set(tx.hash.as_ref().clone()),
        created_contract_address_hash,
        trace_address: Set(Default::default()),
        r#type: Set(Default::default()),
        value: Set(Default::default()),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        block_hash: Set(tx.block_hash.as_ref().clone().unwrap()),
        block_index: Set((*tx.index.as_ref()).unwrap()),
        ..Default::default()
    }
}

fn mock_user_operations(
    blocks: &[blocks::ActiveModel],
    accounts: &[addresses::ActiveModel],
) -> Vec<(transactions::ActiveModel, user_operations::ActiveModel)> {
    blocks[0..blocks.len() - 1]
        .iter()
        // leave 1/3 of blocks empty
        .filter(|b| b.number.as_ref() % 3 != 1)
        // add 1 (user op + transaction) to block
        .map(|b| {
            mock_user_operation(
                b,
                21_000,
                (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                accounts,
                // 0-2 are created at the same blocks in `mock_transactions`
                3,
            )
        })
        .collect_vec()
}

fn mock_user_operation(
    block: &blocks::ActiveModel,
    gas: i64,
    gas_price: i64,
    address_list: &[addresses::ActiveModel],
    index: i32,
) -> (transactions::ActiveModel, user_operations::ActiveModel) {
    // tranasction for the user operation
    let block_number = block.number.as_ref().to_owned() as i32;
    let block_hash = block.hash.as_ref().to_vec();
    let txn_hash = vec![0, 0, 0, 0, block_number as u8, index as u8];
    let address_index = (block_number as usize) % address_list.len();
    let from_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let address_index = (block_number as usize + 1) % address_list.len();
    let to_address_hash = address_list[address_index].hash.as_ref().to_vec();

    // user operation
    let bundler = from_address_hash.clone();
    let entry_point = to_address_hash.clone();
    let op_index = index;

    // data is from some random tx in sepolia;
    // taken from user ops indexer unit tests
    let txn = transactions::ActiveModel {
        block_number: Set(Some(block_number)),
        block_hash: Set(Some(block_hash.clone())),
        block_timestamp: Set(Some(*block.timestamp.as_ref())),
        block_consensus: Set(Some(*block.consensus.as_ref())),
        hash: Set(txn_hash.clone()),
        gas_price: Set(Some(Decimal::new(gas_price, 0))),
        gas: Set(Decimal::new(gas, 0)),
        input: Set(hex!("765e827f000000000000000000000000000000000000000000000000000000000000004000000000000000000000000043d1089285a94bf481e1f6b1a7a114acbc83379600000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000f4240000000000000000000000000001e8480000000000000000000000000000000000000000000000000000000000007a120000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000005a0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000002d81f5806eafab78028b6e29ab65208f54cfdd4ce45a1aafc9e0000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000244ac27308a000000000000000000000000000000000000000000000000000000000000008000000000000000000000000080ee560d57f4b1d2acfeb2174d09d54879c7408800000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000002200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000598991c9d726cbac7eb023ca974fe6e7e7a57ce80000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000003479096622cf141e3cc93126bbccc3ef10b952c1ef000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000074115cff9c5b847b402c382f066cf275ab6440b75aaa1b881c164e5d43131cfb3895759573bc597baf526002f8d1943f1aaa67dbf7fa99cd30d12a235169eef4f3d5c96fc1619c60bc9d8028dfea0f89c7ec5e3f27000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014434fcd5be00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000000000000000000000000000000002540be400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000741b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000061a8000000000000000000000000000061a8000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c800000000000000000000000000000000000000000000000000000002540be400000000000000000000000000000000000000000000000000000000000000000000000000000000000000005a89d0e2cdece3d2f2e2497f2b68c5f96ef073c1800000004200775c0e5049afa24e5370a754faade91452b89dfc97907588ac49b441bcf43d06067f220a252454360907199ae8dfdc7fef2caf6c2aae03e4e0676b2c1ae351601b000000000000").to_vec()),
        nonce: Set(Default::default()),
        r: Set(Default::default()),
        s: Set(Default::default()),
        v: Set(Default::default()),
        value: Set(Decimal::new(0, 0)),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        from_address_hash: Set(from_address_hash),
        to_address_hash: Set(Some(to_address_hash)),
        cumulative_gas_used: Set(Some(Default::default())),
        gas_used: Set(Some(Decimal::new(gas, 0))),
        index: Set(Some(index)),
        status: Set(Some(1)),
        created_contract_address_hash: Set(None),
        created_contract_code_indexed_at: Set(None),
        r#type: Set(Some(2)),
        ..Default::default()
    };

    let op = user_operations::ActiveModel {
        hash: Set(vec![0, 0, 0, 123, block_number as u8, index as u8]),
        sender: Set(hex!("f098c91823f1ef080f22645d030a7196e72d31eb").to_vec()),
        nonce: Set(vec![0u8; 32]),
        init_code: Set(Some(hex!("1f5806eafab78028b6e29ab65208f54cfdd4ce45a1aafc9e0000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000244ac27308a000000000000000000000000000000000000000000000000000000000000008000000000000000000000000080ee560d57f4b1d2acfeb2174d09d54879c7408800000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000002200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000598991c9d726cbac7eb023ca974fe6e7e7a57ce80000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000003479096622cf141e3cc93126bbccc3ef10b952c1ef000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000074115cff9c5b847b402c382f066cf275ab6440b75aaa1b881c164e5d43131cfb3895759573bc597baf526002f8d1943f1aaa67dbf7fa99cd30d12a235169eef4f3d5c96fc1619c60bc9d8028dfea0f89c7ec5e3f27000000000000000000000000000000000000000000000000000000000002a300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec())),
        call_data: Set(hex!("34fcd5be00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000000000000000000000000000000002540be40000000000000000000000000000000000000000000000000000000000").to_vec()),
        call_gas_limit: Set(Decimal::new(2000000, 0)),
        verification_gas_limit: Set(Decimal::new(1000000, 0)),
        pre_verification_gas: Set(Decimal::new(500000, 0)),
        max_fee_per_gas: Set(Decimal::new(1, 0)),
        max_priority_fee_per_gas: Set(Decimal::new(1, 0)),
        paymaster_and_data: Set(Some(hex!("1b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000061a8000000000000000000000000000061a8000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c800000000000000000000000000000000000000000000000000000002540be400").to_vec())),
        signature: Set(hex!("89d0e2cdece3d2f2e2497f2b68c5f96ef073c1800000004200775c0e5049afa24e5370a754faade91452b89dfc97907588ac49b441bcf43d06067f220a252454360907199ae8dfdc7fef2caf6c2aae03e4e0676b2c1ae351601b").to_vec()),
        aggregator: Set(None),
        aggregator_signature: Set(None),
        entry_point: Set(entry_point),
        entry_point_version: Set(EntryPointVersion::V07),
        transaction_hash: Set(txn_hash),
        block_number: Set(block_number),
        block_hash: Set(block_hash),
        bundle_index: Set(0),
        index: Set(op_index),
        user_logs_start_index: Set(42),
        user_logs_count: Set(3),
        bundler: Set(bundler),
        factory: Set(Some(hex!("1f5806eAFab78028B6E29Ab65208F54CFdD4ce45").to_vec())),
        paymaster: Set(Some(hex!("1b637a3008dc1f86d92031a97FC4B5aC0803329e").to_vec())),
        status: Set(true),
        revert_reason: Set(None),
        gas: Set(Decimal::new(4300000, 0)),
        gas_price: Set(Decimal::new(1, 0)),
        gas_used: Set(Decimal::new(1534051, 0)),
        sponsor_type: Set(SponsorType::PaymasterSponsor),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    };
    (txn, op)
}

fn mock_signed_authorizations(
    transactions: &[transactions::ActiveModel],
    contracts: &[addresses::ActiveModel],
    accounts: &[addresses::ActiveModel],
) -> Vec<signed_authorizations::ActiveModel> {
    let transaction_indices = [3usize, 10, 24];
    let mut authorizations = Vec::new();
    for (i, transaction_idx) in transaction_indices.into_iter().enumerate() {
        let Some(transaction) = &transactions.get(transaction_idx) else {
            continue;
        };
        for auth_index in 0..=i {
            authorizations.push(mock_signed_authorization(
                transaction,
                contracts[auth_index].hash.clone().unwrap(),
                accounts[auth_index].hash.clone().unwrap(),
                auth_index as i32,
            ));
        }
    }
    authorizations
}

fn mock_signed_authorization(
    transaction: &transactions::ActiveModel,
    address: Vec<u8>,
    authority: Vec<u8>,
    index: i32,
) -> signed_authorizations::ActiveModel {
    signed_authorizations::ActiveModel {
        transaction_hash: Set(transaction.hash.as_ref().clone()),
        index: Set(index),
        chain_id: Set(1),
        address: Set(address),
        nonce: Set(index * 1000),
        v: Set(27), // Dummy signature components
        r: Set(Decimal::from(123)),
        s: Set(Decimal::from(321)),
        authority: Set(Some(authority)),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
}

fn mock_migration(name: &str, completed: Option<bool>) -> migrations_status::ActiveModel {
    let status = completed
        .map(|done| if done { "completed" } else { "started" })
        .map(|s| s.to_string());
    migrations_status::ActiveModel {
        migration_name: Set(name.to_string()),
        status: Set(status),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
        meta: Set(None),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use itertools::Itertools;
    use pretty_assertions::assert_eq;

    use super::*;

    fn assert_block_hashes_do_not_overlap(
        existing_txns: &[transactions::ActiveModel],
        new_txns: &[transactions::ActiveModel],
    ) {
        let existing_blocks: HashSet<_> = existing_txns
            .iter()
            .map(|t| t.block_hash.clone().unwrap().unwrap())
            .collect();
        let new_blocks: HashSet<_> = new_txns
            .iter()
            .map(|t| t.block_hash.clone().unwrap().unwrap())
            .collect();
        let overlapping_blocks: Vec<&Vec<u8>> =
            new_blocks.intersection(&existing_blocks).collect_vec();
        assert_eq!(overlapping_blocks, Vec::<&Vec<u8>>::new());
    }

    #[test]
    fn reindexing_does_not_produce_overlapping_txns() {
        let existing_blocks = mock_blocks(NaiveDate::MAX);
        let existing_accounts = mock_addresses();
        let existing_txns = mock_transactions(&existing_blocks, &existing_accounts);
        let new_txns: Vec<_> = reindexing_mock_txns(&existing_blocks, &existing_accounts);
        assert_block_hashes_do_not_overlap(&existing_txns, &new_txns);
    }
}
