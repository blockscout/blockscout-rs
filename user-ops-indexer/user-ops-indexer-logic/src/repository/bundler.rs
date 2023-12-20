use crate::types::bundler::Bundler;
use ethers::prelude::Address;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

#[derive(FromQueryResult, Clone)]
pub struct BundlerDB {
    pub bundler: Vec<u8>,
    pub total_bundles: i64,
    pub total_ops: i64,
}

pub async fn find_bundler_by_address(
    db: &DatabaseConnection,
    addr: Address,
) -> Result<Option<Bundler>, anyhow::Error> {
    let bundler = BundlerDB::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
WITH bundles_cte AS (SELECT bundler, count(*) as bundle_ops
                     FROM user_operations
                              JOIN blocks ON blocks.hash = user_operations.block_hash AND consensus
                     WHERE bundler = $1
                     GROUP BY bundler, transaction_hash, bundle_index)
SELECT bundler, count(*) as total_bundles, sum(bundle_ops)::int8 as total_ops
FROM bundles_cte
GROUP BY bundler"#,
        [addr.as_bytes().into()],
    ))
    .one(db)
    .await?
    .map(Bundler::from);

    Ok(bundler)
}

pub async fn list_bundlers(
    db: &DatabaseConnection,
    page_token: Option<(u64, Address)>,
    limit: u64,
) -> Result<(Vec<Bundler>, Option<(u64, Address)>), anyhow::Error> {
    let page_token = page_token.unwrap_or((i64::MAX as u64, Address::zero()));

    let bundlers: Vec<Bundler> = BundlerDB::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
WITH bundles_cte AS (SELECT bundler, count(*) as bundle_ops
                     FROM user_operations
                              JOIN blocks ON blocks.hash = user_operations.block_hash AND consensus
                     GROUP BY bundler, transaction_hash, bundle_index)
SELECT bundler, count(*) as total_bundles, sum(bundle_ops)::int8 as total_ops
FROM bundles_cte
GROUP BY bundler
HAVING (sum(bundle_ops), bundler) <= ($1, $2)
ORDER BY 2 DESC, 1 DESC
LIMIT $3"#,
        [
            page_token.0.into(),
            page_token.1.as_bytes().into(),
            (limit + 1).into(),
        ],
    ))
    .all(db)
    .await?
    .into_iter()
    .map(Bundler::from)
    .collect();

    match bundlers.get(limit as usize) {
        Some(a) => Ok((
            bundlers[0..limit as usize].to_vec(),
            Some((a.total_ops as u64, a.bundler)),
        )),
        None => Ok((bundlers, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn find_bundler_by_address_ok() {
        let db = get_shared_db().await;

        let addr = Address::from_low_u64_be(0xffff);
        let item = find_bundler_by_address(&db, addr).await.unwrap();
        assert_eq!(item, None);

        let addr = Address::from_low_u64_be(0x0105);
        let item = find_bundler_by_address(&db, addr).await.unwrap();
        assert_eq!(
            item,
            Some(Bundler {
                bundler: addr,
                total_ops: 100,
                total_bundles: 100,
            })
        );

        let addr = Address::from_low_u64_be(0x0505);
        let item = find_bundler_by_address(&db, addr).await.unwrap();
        assert_eq!(
            item,
            Some(Bundler {
                bundler: addr,
                total_ops: 100,
                total_bundles: 99,
            })
        );
    }

    #[tokio::test]
    async fn list_bundlers_ok() {
        let db = get_shared_db().await;

        let (items, next_page_token) = list_bundlers(&db, None, 60).await.unwrap();
        assert_eq!(items.len(), 60);
        assert_ne!(next_page_token, None);

        let (items, next_page_token) = list_bundlers(&db, next_page_token, 60).await.unwrap();
        assert_eq!(items.len(), 40);
        assert_eq!(next_page_token, None);
        assert!(items
            .iter()
            .all(|a| a.total_ops == 99 || a.total_ops == 100))
    }
}
