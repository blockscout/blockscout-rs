/// DB read routines for Interchain mode.
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    DatabaseConnection, DbErr, FromQueryResult, Statement,
};

#[derive(FromQueryResult, Debug)]
struct MinTimestamp {
    min_timestamp: Option<NaiveDateTime>,
}

/// Earliest date available in the crosschain_messages table.
pub async fn get_min_date_interchain(
    interchain: &DatabaseConnection,
) -> Result<NaiveDateTime, DbErr> {
    let query = r#"
        SELECT MIN(init_timestamp::timestamp) AS min_timestamp
        FROM crosschain_messages
    "#;

    let result = MinTimestamp::find_by_statement(Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        query.to_owned(),
    ))
    .one(interchain)
    .await?
    .and_then(|r| r.min_timestamp)
    .unwrap_or_else(|| Utc::now().naive_utc());

    Ok(result)
}

/// InterchainIndexer does not have block information; we return i64::MAX so that
/// last_accurate_point logic does not trigger reupdates based on block.
pub async fn get_min_block_interchain(_interchain: &DatabaseConnection) -> Result<i64, DbErr> {
    Ok(i64::MAX)
}
