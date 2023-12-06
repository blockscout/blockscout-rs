use ethers::prelude::Address;
use sea_orm::prelude::DateTime;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

use crate::types::account::Account;

#[derive(FromQueryResult)]
pub struct AccountDB {
    pub address: Vec<u8>,
    pub factory: Option<Vec<u8>>,
    pub creation_tx_hash: Option<Vec<u8>>,
    pub creation_op_hash: Option<Vec<u8>>,
    pub creation_timestamp: Option<DateTime>,
    pub total_ops: i64,
}

pub async fn find_account_by_address(
    db: &DatabaseConnection,
    addr: Address,
) -> Result<Option<Account>, anyhow::Error> {
    let acc = AccountDB::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
WITH account_ops_cte AS (SELECT sender, factory, tx_hash, op_hash, blocks.timestamp
                         FROM user_operations
                                  JOIN blocks ON blocks.hash = block_hash AND consensus
                         WHERE sender = $1),
     account_creation_op_cte AS (SELECT DISTINCT ON (sender) sender, factory, tx_hash, timestamp
                                 FROM account_ops_cte
                                 WHERE factory IS NOT NULL),
     account_total_cte AS (SELECT sender, count(*) as total_ops FROM account_ops_cte GROUP BY sender)
SELECT account_total_cte.sender    as address,
       account_total_cte.total_ops as total_ops,
       account_ops_cte.factory     as factory,
       account_ops_cte.tx_hash     as creation_tx_hash,
       account_ops_cte.op_hash     as creation_op_hash,
       account_ops_cte.timestamp   as creation_timestamp
FROM account_total_cte,
     account_ops_cte"#,
        [addr.as_bytes().into()],
    ))
        .one(db)
        .await?
        .map(Account::from);

    Ok(acc)
}

pub async fn list_accounts(
    db: &DatabaseConnection,
    factory_filter: Option<Address>,
    page_token: Option<Address>,
    limit: u64,
) -> Result<(Vec<Account>, Option<Address>), anyhow::Error> {
    let accounts: Vec<Account> = AccountDB::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
WITH accounts_cte AS (SELECT DISTINCT ON (sender) sender,
                                                  factory,
                                                  CASE WHEN factory IS NOT NULL THEN tx_hash END          as tx_hash,
                                                  CASE WHEN factory IS NOT NULL THEN op_hash END          as op_hash,
                                                  CASE WHEN factory IS NOT NULL THEN blocks.timestamp END as timestamp
                      FROM user_operations
                               JOIN blocks
                                    ON blocks.hash = block_hash AND consensus
                      WHERE sender >= $2
                      AND ($1 IS NULL OR factory = $1)
                      ORDER BY sender, factory NULLS LAST
                      LIMIT $3),
     accounts_total_cte AS (SELECT accounts_cte.sender, count(*) as total_ops
                            FROM accounts_cte
                                     JOIN user_operations ON accounts_cte.sender = user_operations.sender
                                     JOIN blocks ON blocks.hash = block_hash AND consensus
                            GROUP BY accounts_cte.sender)
SELECT accounts_cte.sender          as address,
       accounts_total_cte.total_ops as total_ops,
       accounts_cte.factory         as factory,
       accounts_cte.tx_hash         as creation_tx_hash,
       accounts_cte.op_hash         as creation_op_hash,
       accounts_cte.timestamp       as creation_timestamp
FROM accounts_cte
         JOIN accounts_total_cte ON accounts_cte.sender = accounts_total_cte.sender"#,
        [
            factory_filter.map(|f| f.as_bytes().to_vec()).into(),
            page_token.unwrap_or(Address::zero()).as_bytes().into(),
            (limit + 1).into(),
        ],
    ))
        .all(db)
        .await?
        .into_iter()
        .map(Account::from)
        .collect();

    match accounts.get(limit as usize) {
        Some(a) => Ok((accounts[0..limit as usize].to_vec(), Some(a.address))),
        None => Ok((accounts, None)),
    }
}
