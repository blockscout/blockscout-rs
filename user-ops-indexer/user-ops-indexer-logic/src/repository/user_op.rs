use crate::types::user_op::{ListUserOp, UserOp};
use blockscout_db::entity::blocks;
use entity::user_operations::{ActiveModel, Column, Entity, Model};
use ethers::prelude::{Address, H256};
use sea_orm::{
    prelude::{BigDecimal, DateTime},
    sea_query::{Expr, IntoCondition, OnConflict},
    ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    FromQueryResult, IntoSimpleExpr, Iterable, JoinType, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, RelationDef, Statement,
};

#[derive(FromQueryResult)]
struct TxHash {
    transaction_hash: Vec<u8>,
}

#[derive(FromQueryResult, Clone)]
pub struct ListUserOpDB {
    pub hash: Vec<u8>,
    pub block_number: i32,
    pub sender: Vec<u8>,
    pub transaction_hash: Vec<u8>,
    pub timestamp: DateTime,
    pub status: bool,
    pub gas_price: BigDecimal,
    pub gas_used: BigDecimal,
}

pub fn user_ops_blocks_rel() -> RelationDef {
    blocks::Entity::belongs_to(Entity)
        .from(blocks::Column::Hash)
        .to(Column::BlockHash)
        .on_condition(|_, _| {
            blocks::Column::Consensus
                .into_simple_expr()
                .into_condition()
        })
        .into()
}

pub async fn find_user_op_by_op_hash(
    db: &DatabaseConnection,
    op_hash: H256,
) -> Result<Option<UserOp>, anyhow::Error> {
    let res = db
        .query_one(
            Entity::find_by_id(op_hash.as_bytes())
                .column(blocks::Column::Consensus)
                .column(blocks::Column::Timestamp)
                .join_rev(
                    JoinType::LeftJoin,
                    blocks::Entity::belongs_to(Entity)
                        .from(blocks::Column::Hash)
                        .to(Column::BlockHash)
                        .into(),
                )
                .build(db.get_database_backend()),
        )
        .await?;

    let user_op = match res {
        None => None,
        Some(res) => {
            let user_op = Model::from_query_result(&res, "")?;
            let mut user_op = UserOp::from(user_op);
            user_op.consensus = res.try_get("", "consensus")?;
            user_op.timestamp = res
                .try_get::<Option<DateTime>>("", "timestamp")?
                .map(|t| t.timestamp() as u64);
            Some(user_op)
        }
    };

    Ok(user_op)
}

#[allow(clippy::too_many_arguments)]
pub async fn list_user_ops(
    db: &DatabaseConnection,
    sender_filter: Option<Address>,
    bundler_filter: Option<Address>,
    paymaster_filter: Option<Address>,
    factory_filter: Option<Address>,
    tx_hash_filter: Option<H256>,
    entry_point_filter: Option<Address>,
    bundle_index_filter: Option<u64>,
    block_number_filter: Option<u64>,
    page_token: Option<(u64, H256)>,
    limit: u64,
) -> Result<(Vec<ListUserOp>, Option<(u64, H256)>), anyhow::Error> {
    let page_token = page_token.unwrap_or((i64::MAX as u64, H256::zero()));
    let mut q = Entity::find()
        .select_only()
        .columns([
            Column::Hash,
            Column::BlockNumber,
            Column::Sender,
            Column::TransactionHash,
            Column::Status,
            Column::GasPrice,
            Column::GasUsed,
        ])
        .column(blocks::Column::Timestamp)
        .join_rev(JoinType::Join, user_ops_blocks_rel());
    if let Some(sender) = sender_filter {
        q = q.filter(Column::Sender.eq(sender.as_bytes()));
    }
    if let Some(bundler) = bundler_filter {
        q = q.filter(Column::Bundler.eq(bundler.as_bytes()));
    }
    if let Some(paymaster) = paymaster_filter {
        q = q.filter(Column::Paymaster.eq(paymaster.as_bytes()));
    }
    if let Some(factory) = factory_filter {
        q = q.filter(Column::Factory.eq(factory.as_bytes()));
    }
    if let Some(tx_hash) = tx_hash_filter {
        q = q.filter(Column::TransactionHash.eq(tx_hash.as_bytes()));
        if let Some(bundle_index) = bundle_index_filter {
            q = q.filter(Column::BundleIndex.eq(bundle_index));
        }
    }
    if let Some(entry_point) = entry_point_filter {
        q = q.filter(Column::EntryPoint.eq(entry_point.as_bytes()));
    }
    if let Some(block_number) = block_number_filter {
        q = q.filter(Column::BlockNumber.eq(block_number));
    }
    q = q
        .filter(
            Expr::tuple([
                Column::BlockNumber.into_simple_expr(),
                Column::Hash.into_simple_expr(),
            ])
            .lte(Expr::tuple([
                page_token.0.into(),
                page_token.1.as_bytes().into(),
            ])),
        )
        .order_by_desc(Column::BlockNumber)
        .order_by_desc(Column::Hash)
        .limit(limit + 1);

    let user_ops: Vec<ListUserOp> = q
        .into_model::<ListUserOpDB>()
        .all(db)
        .await?
        .into_iter()
        .map(ListUserOp::from)
        .collect();

    match user_ops.get(limit as usize) {
        Some(a) => Ok((
            user_ops[0..limit as usize].to_vec(),
            Some((a.block_number, a.hash)),
        )),
        None => Ok((user_ops, None)),
    }
}

pub async fn upsert_many(
    db: &DatabaseConnection,
    user_ops: Vec<UserOp>,
) -> Result<(), anyhow::Error> {
    let user_ops = user_ops.into_iter().map(|user_op| {
        let model: Model = user_op.into();
        let mut active: ActiveModel = model.into();
        active.inserted_at = ActiveValue::NotSet;
        active.updated_at = ActiveValue::NotSet;
        active
    });

    Entity::insert_many(user_ops)
        .on_conflict(
            OnConflict::column(Column::Hash)
                .update_columns(Column::iter().filter(|col| {
                    !matches!(col, Column::Hash | Column::InsertedAt | Column::UpdatedAt)
                }))
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

pub async fn find_unprocessed_logs_tx_hashes(
    db: &DatabaseConnection,
    addr: Address,
    topic: H256,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<H256>, anyhow::Error> {
    let tx_hashes = TxHash::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
SELECT DISTINCT logs.transaction_hash as transaction_hash
FROM logs
         JOIN blocks ON logs.block_hash = blocks.hash AND blocks.consensus
         LEFT JOIN user_operations ON logs.second_topic = '0x' || ENCODE(user_operations.hash, 'hex')
WHERE logs.address_hash    = $1
  AND logs.first_topic     = '0x' || ENCODE($2, 'hex')
  AND logs.block_number    >= $3
  AND logs.block_number    <= $4
  AND user_operations.hash IS NULL"#,
        [addr.as_bytes().into(), topic.as_bytes().into(), from_block.into(), to_block.into()],
    ))
        .all(db)
        .await?
        .into_iter()
        .map(|tx| H256::from_slice(&tx.transaction_hash))
        .collect();

    Ok(tx_hashes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
    use ethers::prelude::U256;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[tokio::test]
    async fn find_user_op_by_op_hash_ok() {
        let db = get_shared_db().await;

        let hash = H256::from_low_u64_be(0x0102);
        let item = find_user_op_by_op_hash(&db, hash).await.unwrap();
        assert_eq!(item, None);

        let hash = H256::from_low_u64_be(0x0101);
        let item = find_user_op_by_op_hash(&db, hash).await.unwrap();
        assert_ne!(item, None);
        let item = item.unwrap();
        assert_eq!(item.hash, hash);
        assert_eq!(item.consensus, Some(true));

        let hash = H256::from_low_u64_be(0x1a0401);
        let item = find_user_op_by_op_hash(&db, hash).await.unwrap();
        assert_ne!(item, None);
        let item = item.unwrap();
        assert_eq!(item.hash, hash);
        assert_eq!(item.consensus, Some(false));

        let hash = H256::from_low_u64_be(0x1a0e01);
        let item = find_user_op_by_op_hash(&db, hash).await.unwrap();
        assert_ne!(item, None);
        let item = item.unwrap();
        assert_eq!(item.hash, hash);
        assert_eq!(item.consensus, None);
    }

    #[tokio::test]
    async fn list_user_ops_ok() {
        let db = get_shared_db().await;

        let (items, next_page_token) = list_user_ops(
            &db, None, None, None, None, None, None, None, None, None, 5000,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 5000);
        assert_ne!(next_page_token, None);
        assert!(items
            .iter()
            .all(|a| a.block_number != 666 && a.block_number != 667));

        let (items, next_page_token) = list_user_ops(
            &db,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            next_page_token,
            5000,
        )
        .await
        .unwrap();
        assert_eq!(items.len(), 4980);
        assert_eq!(next_page_token, None);
        assert!(items
            .iter()
            .all(|a| a.block_number != 666 && a.block_number != 667));

        let (items, next_page_token) = list_user_ops(
            &db,
            Some(Address::from_low_u64_be(0x0502)),
            None,
            None,
            None,
            Some(H256::from_low_u64_be(0x0504)),
            None,
            Some(0),
            Some(0),
            None,
            10,
        )
        .await
        .unwrap();
        assert_eq!(next_page_token, None);
        assert_eq!(
            items,
            [
                ListUserOp {
                    hash: H256::from_low_u64_be(0x6901),
                    block_number: 0,
                    sender: Address::from_low_u64_be(0x0502),
                    transaction_hash: H256::from_low_u64_be(0x0504),
                    timestamp: 1704067200,
                    status: true,
                    fee: U256::from(56001575011025u64),
                },
                ListUserOp {
                    hash: H256::from_low_u64_be(0x0501),
                    block_number: 0,
                    sender: Address::from_low_u64_be(0x0502),
                    transaction_hash: H256::from_low_u64_be(0x0504),
                    timestamp: 1704067200,
                    status: true,
                    fee: U256::from(56000075000025u64),
                }
            ]
        );
    }

    #[tokio::test]
    async fn find_unprocessed_logs_tx_hashes_ok() {
        let db = get_shared_db().await;

        let entrypoint = Address::from_str("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789").unwrap();
        let topic =
            H256::from_str("0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f")
                .unwrap();
        let items = find_unprocessed_logs_tx_hashes(&db, entrypoint, topic, 100, 150)
            .await
            .unwrap();
        assert_eq!(items, [H256::from_low_u64_be(0xffff)]);
    }
}
