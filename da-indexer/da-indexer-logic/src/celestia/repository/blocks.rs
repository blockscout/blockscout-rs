use da_indexer_entity::celestia_blocks::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter,
    QuerySelect, Statement,
};

#[derive(FromQueryResult, Debug)]
pub struct Gap {
    pub gap_start: i64,
    pub gap_end: i64,
}

pub async fn find_gaps(
    db: &DatabaseConnection,
    up_to_height: u64,
) -> Result<Vec<Gap>, anyhow::Error> {
    let mut gaps = Gap::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
            SELECT height + 1 as gap_start, 
                   next_nr - 1 as gap_end
            FROM (
                SELECT height, lead(height) OVER (ORDER BY height) as next_nr
                FROM celestia_blocks WHERE height <= $1
            ) nr
            WHERE nr.height + 1 <> nr.next_nr ORDER BY nr.height;"#,
        [(up_to_height as i64).into()],
    ))
    .all(db)
    .await?;

    // if there is no row with height == up_to_height, we will miss the last gap
    let gaps_end = gaps.last().map(|gap| gap.gap_end).unwrap_or(0) as u64;
    let max_height = find_max_in_range(db, gaps_end, up_to_height)
        .await?
        .unwrap_or(0) as u64;
    if max_height < up_to_height {
        gaps.push(Gap {
            gap_start: (max_height + 1) as i64,
            gap_end: up_to_height as i64,
        })
    }

    Ok(gaps)
}

pub async fn find_max_in_range(
    db: &DatabaseConnection,
    from: u64,
    to: u64,
) -> Result<Option<i64>, anyhow::Error> {
    let max_height = Entity::find()
        .select_only()
        .column_as(Expr::col(Column::Height).max(), "height")
        .filter(Column::Height.gte(from as i64))
        .filter(Column::Height.lte(to as i64))
        .into_tuple()
        .one(db)
        .await?;
    Ok(max_height)
}

pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    height: u64,
    hash: &[u8],
    blobs_count: u32,
    timestamp: i64,
) -> Result<(), anyhow::Error> {
    let model = Model {
        height: height as i64,
        hash: hash.to_vec(),
        blobs_count: blobs_count as i32,
        timestamp,
    };
    let active: ActiveModel = model.into();

    Entity::insert(active)
        .on_conflict(
            OnConflict::column(Column::Height)
                .update_columns([Column::Hash, Column::BlobsCount, Column::Timestamp])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

pub async fn exists(db: &DatabaseConnection, height: u64) -> Result<bool, anyhow::Error> {
    Ok(Entity::find_by_id(height as i64).one(db).await?.is_some())
}
