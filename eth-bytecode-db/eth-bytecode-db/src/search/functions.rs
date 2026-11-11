// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy_primitives::Selector;
use anyhow::Context;
use entity::functions;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;

pub type FunctionDescription = functions::Model;

/// Returns the known function descriptions for each of the given selectors,
/// in the order of `selectors`. All selectors are looked up with a single query.
pub async fn find_function_descriptions<C>(
    db: &C,
    selectors: Vec<Selector>,
) -> Result<Vec<Vec<FunctionDescription>>, anyhow::Error>
where
    C: ConnectionTrait,
{
    if selectors.is_empty() {
        return Ok(vec![]);
    }

    let models = functions::Entity::find()
        .filter(
            functions::Column::Selector.is_in(selectors.iter().map(|selector| selector.to_vec())),
        )
        .order_by_asc(functions::Column::Id)
        .all(db)
        .await
        .context("extracting functions from the database")?;

    let mut grouped: HashMap<Vec<u8>, Vec<FunctionDescription>> = HashMap::new();
    for model in models {
        grouped
            .entry(model.selector.clone())
            .or_default()
            .push(model);
    }

    Ok(selectors
        .into_iter()
        .map(|selector| {
            grouped
                .get(selector.as_slice())
                .cloned()
                .unwrap_or_default()
        })
        .collect())
}
