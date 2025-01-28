pub mod addresses;
pub mod api_keys;
pub mod block_ranges;
pub mod chains;
pub mod hashes;

use sea_orm::{ConnectionTrait, Cursor, DbErr, FromQueryResult, ModelTrait, SelectModel};

pub async fn paginate_cursor<E, R, F>(
    db: &impl ConnectionTrait,
    mut c: Cursor<SelectModel<E>>,
    page_size: u64,
    page_token: F,
) -> Result<(Vec<E>, Option<R>), DbErr>
where
    E: ModelTrait + FromQueryResult,
    F: FnOnce(&E) -> R,
{
    let results = c.first(page_size + 1).all(db).await?;
    dbg!(&results);
    if results.len() as u64 > page_size {
        Ok((
            results[..page_size as usize].to_vec(),
            results.get(page_size as usize - 1).map(page_token),
        ))
    } else {
        Ok((results, None))
    }
}
