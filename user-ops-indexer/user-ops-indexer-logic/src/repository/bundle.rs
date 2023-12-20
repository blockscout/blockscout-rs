use crate::types::bundle::Bundle;
use ethers::prelude::{Address, H256};
use sea_orm::{prelude::DateTime, ConnectionTrait, DatabaseConnection, FromQueryResult, Statement};

#[derive(FromQueryResult)]
pub struct BundleDB {
    pub transaction_hash: Vec<u8>,
    pub bundle_index: i32,
    pub block_number: i32,
    pub bundler: Vec<u8>,
    pub timestamp: DateTime,
    pub total_ops: i64,
}

pub async fn list_bundles(
    db: &DatabaseConnection,
    bundler_filter: Option<Address>,
    entry_point_filter: Option<Address>,
    page_token: Option<(u64, H256, u64)>,
    limit: u64,
) -> Result<(Vec<Bundle>, Option<(u64, H256, u64)>), anyhow::Error> {
    let page_token = page_token.unwrap_or((i64::MAX as u64, H256::zero(), 0));
    let bundles: Vec<Bundle> = BundleDB::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
SELECT transaction_hash, bundle_index, block_number, bundler, blocks.timestamp as timestamp, count(*) as total_ops
FROM user_operations
         JOIN blocks ON blocks.hash = user_operations.block_hash AND consensus
WHERE (block_number, transaction_hash, bundle_index) <=
      ($3, $4, $5)
      AND ($1 IS NULL OR bundler = $1)
      AND ($2 IS NULL OR entry_point = $2)
GROUP BY transaction_hash, bundle_index, block_number, bundler, blocks.timestamp
ORDER BY block_number DESC, transaction_hash DESC, bundle_index DESC
LIMIT $6"#,
        [
            bundler_filter.map(|f| f.as_bytes().to_vec()).into(),
            entry_point_filter.map(|f| f.as_bytes().to_vec()).into(),
            page_token.0.into(),
            page_token.1.as_bytes().into(),
            page_token.2.into(),
            (limit + 1).into(),
        ],
    ))
        .all(db)
        .await?
        .into_iter()
        .map(Bundle::from)
        .collect();

    match bundles.get(limit as usize) {
        Some(a) => Ok((
            bundles[0..limit as usize].to_vec(),
            Some((a.block_number, a.transaction_hash, a.bundle_index)),
        )),
        None => Ok((bundles, None)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[tokio::test]
    async fn list_bundles_ok() {
        let db = get_shared_db().await;

        let (items, next_page_token) = list_bundles(&db, None, None, None, 100).await.unwrap();
        assert_eq!(items.len(), 100);
        assert_ne!(next_page_token, None);

        let entrypoint =
            Some(Address::from_str("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789").unwrap());
        let bundler = Some(Address::from_low_u64_be(0x0105));

        let (items, next_page_token) = list_bundles(&db, None, entrypoint, None, 100)
            .await
            .unwrap();
        assert_eq!(items.len(), 100);
        assert_ne!(next_page_token, None);
        let (items, next_page_token) = list_bundles(&db, bundler, entrypoint, None, 100)
            .await
            .unwrap();
        assert_eq!(items.len(), 100);
        assert_eq!(next_page_token, None);
        assert!(items.iter().all(|a| Some(a.bundler) == bundler));
        let (items, next_page_token) = list_bundles(&db, bundler, None, None, 100).await.unwrap();
        assert_eq!(items.len(), 100);
        assert_eq!(next_page_token, None);
        let (items, next_page_token) = list_bundles(&db, bundler, bundler, None, 100)
            .await
            .unwrap();
        assert_eq!(items.len(), 0);
        assert_eq!(next_page_token, None);
    }
}
