use anyhow::Context;
use entity::events;
use ethers_core::types::H256;
use futures::StreamExt;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

pub type EventDescription = events::Model;

pub async fn find_event_descriptions<C>(
    db: &C,
    selectors: Vec<H256>,
) -> Vec<Result<Vec<EventDescription>, anyhow::Error>>
where
    C: ConnectionTrait + TransactionTrait,
{
    tokio_stream::iter(selectors.into_iter().map(|selector| async move {
        events::Entity::find()
            .filter(events::Column::Selector.eq(selector.as_bytes().to_vec()))
            .all(db)
            .await
            .context(format!(
                "extracting events from the database for {selector:x}"
            ))
    }))
    .buffered(20)
    .collect()
    .await
}
