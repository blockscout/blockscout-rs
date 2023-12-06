use ethers::prelude::{Address, H256};
use sea_orm::prelude::DateTime;
use sea_orm::sea_query::{Expr, IntoCondition, OnConflict};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    FromQueryResult, IntoSimpleExpr, Iterable, JoinType, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait, RelationDef, Statement,
};

use entity::user_operations::{ActiveModel, Column, Entity, Model};

use crate::types::user_op::{ListUserOp, UserOp};

#[derive(FromQueryResult)]
struct TxHash {
    tx_hash: Vec<u8>,
}

#[derive(FromQueryResult, Clone)]
pub struct ListUserOpDB {
    pub op_hash: Vec<u8>,
    pub block_number: i32,
    pub sender: Vec<u8>,
    pub tx_hash: Vec<u8>,
    pub timestamp: DateTime,
}

pub fn user_ops_block_rel() -> RelationDef {
    entity::blocks::Entity::belongs_to(Entity)
        .from(entity::blocks::Column::Hash)
        .to(Column::BlockHash)
        .on_condition(|_, _| {
            entity::blocks::Column::Consensus
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
                .column(entity::blocks::Column::Consensus)
                .column(entity::blocks::Column::Timestamp)
                .join_rev(
                    JoinType::LeftJoin,
                    entity::blocks::Entity::belongs_to(Entity)
                        .from(entity::blocks::Column::Hash)
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
                .map(|t| t.timestamp() as u32);
            Some(user_op)
        }
    };

    Ok(user_op)
}

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
            Column::OpHash,
            Column::BlockNumber,
            Column::Sender,
            Column::TxHash,
        ])
        .column(entity::blocks::Column::Timestamp)
        .join_rev(JoinType::Join, user_ops_block_rel());
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
        q = q.filter(Column::TxHash.eq(tx_hash.as_bytes()));
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
                Column::OpHash.into_simple_expr(),
            ])
            .lte(Expr::tuple([
                page_token.0.into(),
                page_token.1.as_bytes().into(),
            ])),
        )
        .order_by_desc(Column::BlockNumber)
        .order_by_desc(Column::OpHash)
        .limit(limit + 1);

    let user_ops: Vec<ListUserOp> = q
        .into_model::<ListUserOpDB>()
        .all(db)
        .await?
        .iter()
        .map(|op| ListUserOp::from(op.clone()))
        .collect();

    match user_ops.get(limit as usize) {
        Some(a) => Ok((
            user_ops[0..limit as usize].to_vec(),
            Some((a.block_number, a.op_hash)),
        )),
        None => Ok((user_ops, None)),
    }
}

pub async fn upsert_many(
    db: &DatabaseConnection,
    user_ops: Vec<UserOp>,
) -> Result<(), anyhow::Error> {
    let user_ops = user_ops.iter().map(|user_op| {
        let model: Model = user_op.clone().into();
        let mut active: ActiveModel = model.into();
        active.created_at = ActiveValue::NotSet;
        active.updated_at = ActiveValue::NotSet;
        active
    });

    Entity::insert_many(user_ops)
        .on_conflict(
            OnConflict::column(Column::OpHash)
                .update_columns(Column::iter().filter(|col| match col {
                    Column::OpHash => false,
                    Column::CreatedAt => false,
                    Column::UpdatedAt => false,
                    _ => true,
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
SELECT DISTINCT logs.transaction_hash as tx_hash
FROM logs
         JOIN blocks ON logs.block_hash = blocks.hash AND blocks.consensus
         LEFT JOIN user_operations ON logs.second_topic = '0x' || ENCODE(user_operations.op_hash, 'hex')
WHERE logs.address_hash = $1
  AND logs.first_topic = '0x' || ENCODE($2, 'hex')
  AND logs.block_number >= $3
  AND logs.block_number <= $4
  AND user_operations.op_hash IS NULL"#,
        [addr.as_bytes().into(), topic.as_bytes().into(), from_block.into(), to_block.into()],
    ))
        .all(db)
        .await?
        .into_iter()
        .map(|tx| H256::from_slice(&tx.tx_hash))
        .collect();

    Ok(tx_hashes)
}
