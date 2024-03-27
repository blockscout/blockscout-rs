use da_indexer_entity::celestia_blocks::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    sea_query::OnConflict, ConnectionTrait, DatabaseConnection, EntityTrait, FromQueryResult,
    Statement,
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
    let gaps = Gap::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
            SELECT height + 1 as gap_start, 
                   next_nr - 1 as gap_end
            FROM (
                SELECT height, lead(height) OVER (ORDER BY height) as next_nr
                FROM celestia_blocks WHERE height <= $1
            ) nr
            WHERE nr.height + 1 <> nr.next_nr;"#,
        [(up_to_height as i64).into()],
    ))
    .all(db)
    .await?;

    Ok(gaps)
}

pub async fn upsert(
    db: &DatabaseConnection,
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

pub async fn remove(db: &DatabaseConnection, height: u64) -> Result<(), anyhow::Error> {
    Entity::delete_by_id(height as i64).exec(db).await?;
    Ok(())
}

pub async fn exists(db: &DatabaseConnection, height: u64) -> Result<bool, anyhow::Error> {
    Ok(Entity::find_by_id(height as i64).one(db).await?.is_some())
}
