use blockscout_db::entity::{
    address_coin_balances_daily, addresses, block_rewards, blocks, internal_transactions,
    smart_contracts, tokens, transactions,
};
use chrono::{NaiveDate, NaiveDateTime};
use sea_orm::{prelude::Decimal, ActiveValue::NotSet, DatabaseConnection, EntityTrait, Set};
use std::str::FromStr;

pub async fn fill_mock_blockscout_data(blockscout: &DatabaseConnection, max_date: &str) {
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
        "2022-12-01T10:00:00",
        "2023-01-01T10:00:00",
        "2023-02-01T10:00:00",
        "2023-03-01T10:00:00",
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

    let accounts = (1..9)
        .map(|seed| mock_address(seed, false, false))
        .collect::<Vec<_>>();
    addresses::Entity::insert_many(accounts.clone())
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

    let txns = blocks[0..blocks.len() - 1]
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
                    &accounts,
                    0,
                    TxType::Transfer,
                ),
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    &accounts,
                    1,
                    TxType::Transfer,
                ),
                mock_transaction(
                    b,
                    21_000,
                    (b.number.as_ref() * 1_123_456_789) % 70_000_000_000,
                    &accounts,
                    2,
                    TxType::ContractCall,
                ),
            ]
        });
    transactions::Entity::insert_many(txns)
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
                (3 + i) as i32,
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
        "2022-11-17T00:00:00",
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

    let rewards = blocks.iter().enumerate().map(|(i, block)| {
        mock_block_rewards(
            accounts
                .get(i % (accounts.len() / 2))
                .unwrap()
                .hash
                .as_ref()
                .to_vec(),
            block.hash.as_ref().to_vec(),
            Some(Decimal::from(i % 5) * Decimal::try_from(1e18).unwrap()),
        )
    });
    block_rewards::Entity::insert_many(rewards)
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

fn mock_address(seed: i64, is_contract: bool, is_verified: bool) -> addresses::ActiveModel {
    let mut hash = seed.to_le_bytes().to_vec();
    hash.extend(std::iter::repeat(0).take(32 - hash.len()));
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

fn mock_transaction(
    block: &blocks::ActiveModel,
    gas: i64,
    gas_price: i64,
    address_list: &Vec<addresses::ActiveModel>,
    index: i32,
    tx_type: TxType,
) -> transactions::ActiveModel {
    let block_number = block.number.as_ref().to_owned() as i32;
    let hash = vec![0, 0, 0, 0, block_number as u8, index as u8];
    let address_index = (block_number as usize) % address_list.len();
    let from_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let address_index = (block_number as usize + 1) % address_list.len();
    let to_address_hash = address_list[address_index].hash.as_ref().to_vec();
    let input = tx_type
        .needs_input()
        .then(|| vec![60u8, 80u8])
        .unwrap_or_default();
    let value = (tx_type.needs_value())
        .then_some(1_000_000_000_000)
        .unwrap_or_default();
    let created_contract_address_hash = match tx_type {
        TxType::ContractCreation(contract_address) => Some(contract_address),
        _ => None,
    };

    transactions::ActiveModel {
        block_number: Set(Some(block_number)),
        block_hash: Set(Some(block.hash.as_ref().to_vec())),
        hash: Set(hash),
        gas_price: Set(Decimal::new(gas_price, 0)),
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
    address: Vec<u8>,
    block_hash: Vec<u8>,
    reward: Option<Decimal>,
) -> block_rewards::ActiveModel {
    block_rewards::ActiveModel {
        address_hash: Set(address),
        address_type: Set("".into()),
        block_hash: Set(block_hash),
        reward: Set(reward),
        inserted_at: Set(Default::default()),
        updated_at: Set(Default::default()),
    }
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
