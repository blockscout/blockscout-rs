use ethers::prelude::Address;
use sea_orm::{ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

use crate::types::bundler::Bundler;

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
                     GROUP BY bundler, tx_hash, bundle_index)
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
                     GROUP BY bundler, tx_hash, bundle_index)
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
    .map(|b| Bundler::from(b))
    .collect();

    match bundlers.get(limit as usize) {
        Some(a) => Ok((
            bundlers[0..limit as usize].to_vec(),
            Some((a.total_ops as u64, a.bundler)),
        )),
        None => Ok((bundlers, None)),
    }
}
