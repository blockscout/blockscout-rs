pub mod address_coin_balances;
pub mod address_token_balances;
pub mod addresses;
pub mod api_keys;
pub mod block_ranges;
pub mod chains;
pub mod counters;
pub mod hashes;
pub mod interop_message_transfers;
pub mod interop_messages;

use sea_orm::{sea_query::IntoValueTuple, ConnectionTrait, Cursor, DbErr, SelectorTrait};

pub async fn paginate_cursor<S, E, R1, R2, F>(
    db: &impl ConnectionTrait,
    mut c: Cursor<S>,
    page_size: u64,
    page_token: Option<R1>,
    into_page_token: F,
) -> Result<(Vec<E>, Option<R2>), DbErr>
where
    E: Clone,
    S: SelectorTrait<Item = E>,
    R1: IntoValueTuple,
    F: FnOnce(&E) -> R2,
{
    if let Some(page_token) = page_token {
        c.after(page_token);
    };
    let results = c.first(page_size + 1).all(db).await?;
    if results.len() as u64 > page_size {
        Ok((
            results[..page_size as usize].to_vec(),
            results.get(page_size as usize - 1).map(into_page_token),
        ))
    } else {
        Ok((results, None))
    }
}
