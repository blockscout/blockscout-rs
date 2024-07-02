use da_indexer_entity::eigenda_batches::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QuerySelect, Statement,
};

use crate::common::types::gap::Gap;

pub async fn find_gaps(
    db: &DatabaseConnection,
    contract_creation_block: i64,
    to_block: i64,
) -> Result<Vec<Gap>, anyhow::Error> {
    let mut gaps = vec![];
    // add the gap between the contract creation block and the first saved batch
    match find_min_batch_id(db).await? {
        Some((min_batch_id, min_l1_block)) if min_batch_id > 0 => {
            gaps.push(Gap::new(contract_creation_block, min_l1_block - 1));
        }
        None => {
            gaps.push(Gap::new(contract_creation_block, to_block));
            return Ok(gaps);
        }
        _ => {}
    }

    gaps.append(
        &mut Gap::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
        SELECT l1_block + 1 as start, 
                next_l1_block - 1 as end
        FROM (
            SELECT batch_id, l1_block, lead(batch_id) OVER (ORDER BY batch_id) as next_batch_id, 
                lead(l1_block) OVER (ORDER BY batch_id) as next_l1_block
            FROM eigenda_batches WHERE l1_block <= $1
        ) nr
        WHERE nr.batch_id + 1 <> nr.next_batch_id ORDER BY nr.batch_id;"#,
            [to_block.into()],
        ))
        .all(db)
        .await?,
    );

    // add the gap between the last saved batch and the to_block
    let gaps_end = gaps
        .last()
        .map(|gap| gap.end)
        .unwrap_or(contract_creation_block);
    match find_max_l1_block_in_range(db, gaps_end, to_block).await? {
        Some(max_height) if max_height < to_block => {
            gaps.push(Gap {
                start: max_height + 1,
                end: to_block,
            });
        }
        _ => {}
    }

    Ok(gaps)
}

pub async fn find_max_l1_block_in_range(
    db: &DatabaseConnection,
    from: i64,
    to: i64,
) -> Result<Option<i64>, anyhow::Error> {
    let max_block: Option<Option<i64>> = Entity::find()
        .select_only()
        .column_as(Expr::col(Column::L1Block).max(), "l1_block")
        .filter(Column::L1Block.gte(from))
        .filter(Column::L1Block.lte(to))
        .into_tuple()
        .one(db)
        .await?;
    Ok(max_block.flatten())
}

pub async fn find_min_batch_id(
    db: &DatabaseConnection,
) -> Result<Option<(i64, i64)>, anyhow::Error> {
    let min_block: Option<(Option<i64>, Option<i64>)> = Entity::find()
        .select_only()
        .column_as(Expr::col(Column::BatchId).min(), "batch_id")
        .column_as(Expr::col(Column::L1Block).min(), "l1_block")
        .into_tuple()
        .one(db)
        .await?;
    match min_block {
        Some((Some(batch_id), Some(l1_block))) => Ok(Some((batch_id, l1_block))),
        _ => Ok(None),
    }
}

pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    batch_header_hash: &[u8],
    batch_id: i64,
    blobs_count: i32,
    l1_tx_hash: &[u8],
    l1_block: i64,
) -> Result<(), anyhow::Error> {
    let model = Model {
        batch_id,
        batch_header_hash: batch_header_hash.to_vec(),
        blobs_count,
        l1_tx_hash: l1_tx_hash.to_vec(),
        l1_block,
    };
    let active: ActiveModel = model.into();

    Entity::insert(active)
        .on_conflict(
            OnConflict::column(Column::BatchId)
                .update_columns([
                    Column::BatchHeaderHash,
                    Column::BlobsCount,
                    Column::L1TxHash,
                    Column::L1Block,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}
