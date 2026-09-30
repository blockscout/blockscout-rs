// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy_primitives::B256;
use anyhow::Context;
use entity::events;
use futures::StreamExt;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait};

pub type EventDescription = events::Model;

pub async fn find_event_descriptions<C>(
    db: &C,
    selectors: Vec<B256>,
) -> Vec<Result<Vec<EventDescription>, anyhow::Error>>
where
    C: ConnectionTrait + TransactionTrait,
{
    tokio_stream::iter(selectors.into_iter().map(|selector| async move {
        events::Entity::find()
            .filter(events::Column::Selector.eq(selector.to_vec()))
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
