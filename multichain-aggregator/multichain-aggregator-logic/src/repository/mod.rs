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
pub mod tokens;

mod batch_update;

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

pub mod macros {
    macro_rules! update_if_not_null {
        ($column:expr) => {
            (
                $column,
                Expr::cust_with_exprs(
                    "COALESCE($1, $2)",
                    [
                        Expr::cust(format!("EXCLUDED.{}", $column.as_str())),
                        $column.into_expr().into(),
                    ],
                ),
            )
        };
    }

    macro_rules! is_distinct_from {
        ($( $column:expr ),+) => {
            {
                let excluded_exprs = Vec::from([$( Expr::cust(format!("EXCLUDED.{}", $column.as_str())) ),*]);
                let column_exprs = Vec::from([$( $column.into_expr().into() ),*]);

                let left_tuple = Expr::tuple(excluded_exprs);
                let right_tuple = Expr::tuple(column_exprs);

                Expr::cust_with_exprs("$1 IS DISTINCT FROM $2", [left_tuple.into(), right_tuple.into()])
            }
        };
    }

    pub(crate) use is_distinct_from;
    pub(crate) use update_if_not_null;
}
