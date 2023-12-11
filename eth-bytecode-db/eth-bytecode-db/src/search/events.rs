use anyhow::Context;
use entity::events;
use ethers_core::types::H256;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

pub type EventDescription = events::Model;

pub async fn find_event_descriptions<C>(
    db: &C,
    selector: H256,
) -> Result<Vec<EventDescription>, anyhow::Error>
where
    C: ConnectionTrait + TransactionTrait,
{
    Ok(events::Entity::find()
        .filter(events::Column::Selector.eq(selector.as_bytes().to_vec()))
        .all(db)
        .await
        .context("extracting events from the database")?)
}
