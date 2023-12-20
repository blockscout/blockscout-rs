use crate::types::account::Account;
use ethers::prelude::Address;
use sea_orm::{prelude::DateTime, ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

#[derive(FromQueryResult)]
pub struct AccountDB {
    pub address: Vec<u8>,
    pub factory: Option<Vec<u8>>,
    pub creation_transaction_hash: Option<Vec<u8>>,
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
WITH account_ops_cte AS (SELECT sender, factory, user_operations.transaction_hash, user_operations.hash, blocks.timestamp
                         FROM user_operations
                                  JOIN blocks ON blocks.hash = block_hash AND consensus
                         WHERE sender = $1),
     account_creation_op_cte AS (SELECT DISTINCT ON (sender) sender, factory, hash, transaction_hash, timestamp
                                 FROM account_ops_cte
                                 WHERE factory IS NOT NULL),
     account_total_cte AS (SELECT sender, count(*) as total_ops FROM account_ops_cte GROUP BY sender)
SELECT account_total_cte.sender                 as address,
       account_total_cte.total_ops              as total_ops,
       account_creation_op_cte.factory          as factory,
       account_creation_op_cte.transaction_hash as creation_transaction_hash,
       account_creation_op_cte.hash             as creation_op_hash,
       account_creation_op_cte.timestamp        as creation_timestamp
FROM account_total_cte
         LEFT JOIN account_creation_op_cte ON account_total_cte.sender = account_creation_op_cte.sender"#,
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
                                                  CASE WHEN factory IS NOT NULL THEN user_operations.transaction_hash END as creation_transaction_hash,
                                                  CASE WHEN factory IS NOT NULL THEN user_operations.hash END             as creation_op_hash,
                                                  CASE WHEN factory IS NOT NULL THEN blocks.timestamp END                 as creation_timestamp
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
SELECT accounts_cte.sender                    as address,
       accounts_total_cte.total_ops           as total_ops,
       accounts_cte.factory                   as factory,
       accounts_cte.creation_transaction_hash as creation_transaction_hash,
       accounts_cte.creation_op_hash          as creation_op_hash,
       accounts_cte.creation_timestamp        as creation_timestamp
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
    use keccak_hash::H256;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn find_account_by_address_ok() {
        let db = get_shared_db().await;

        let addr = Address::from_low_u64_be(0xffff);
        let item = find_account_by_address(&db, addr).await.unwrap();
        assert_eq!(item, None);

        let addr = Address::from_low_u64_be(0x0102);
        let item = find_account_by_address(&db, addr).await.unwrap();
        assert_eq!(
            item,
            Some(Account {
                address: addr,
                factory: None,
                creation_transaction_hash: None,
                creation_op_hash: None,
                creation_timestamp: None,
                total_ops: 100,
            })
        );

        let addr = Address::from_low_u64_be(0x3202);
        let item = find_account_by_address(&db, addr).await.unwrap();
        assert_eq!(
            item,
            Some(Account {
                address: addr,
                factory: Some(Address::from_low_u64_be(0xf1)),
                creation_transaction_hash: Some(H256::from_low_u64_be(0x3204)),
                creation_op_hash: Some(H256::from_low_u64_be(0x3201)),
                creation_timestamp: Some(1704067260),
                total_ops: 100,
            })
        );
    }

    #[tokio::test]
    async fn list_accounts_ok() {
        let db = get_shared_db().await;

        let (items, next_page_token) = list_accounts(&db, None, None, 60).await.unwrap();
        assert_eq!(items.len(), 60);
        assert_ne!(next_page_token, None);

        let (items, next_page_token) = list_accounts(&db, None, next_page_token, 60).await.unwrap();
        assert_eq!(items.len(), 40);
        assert_eq!(next_page_token, None);

        let factory = Some(Address::from_low_u64_be(0xf1));
        let (items, next_page_token) = list_accounts(&db, factory, None, 60).await.unwrap();
        assert_eq!(items.len(), 10);
        assert_eq!(next_page_token, None);
        assert!(items.iter().all(|a| a.factory == factory))
    }
}
