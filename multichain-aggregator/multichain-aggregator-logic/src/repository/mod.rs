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
pub mod pagination;
pub mod tokens;

mod batch_update;

use regex::Regex;
use sea_orm::{
    ConnectionTrait, Cursor, DbErr, FromQueryResult, SelectorTrait,
    sea_query::{IntoValueTuple, SelectStatement},
};
use std::sync::OnceLock;

// TODO: replace with `paginate_query` in all applicable places
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

pub async fn paginate_query<E, P1, P2, F>(
    db: &impl ConnectionTrait,
    mut query: SelectStatement,
    page_size: u64,
    page_token: Option<P1>,
    key_specs: Vec<pagination::KeySpec>,
    into_page_token: F,
) -> Result<(Vec<E>, Option<P2>), DbErr>
where
    E: FromQueryResult + Clone,
    P1: IntoValueTuple,
    F: FnOnce(&E) -> P2,
{
    let cursor =
        pagination::Cursor::new(page_token, key_specs).map_err(|e| DbErr::Custom(e.to_string()))?;
    cursor.apply_pagination(
        &mut query,
        pagination::PageOptions {
            page_size: page_size + 1,
        },
    );

    let models = E::find_by_statement(db.get_database_backend().build(&query))
        .all(db)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    if models.len() as u64 > page_size {
        Ok((
            models[..page_size as usize].to_vec(),
            models.get(page_size as usize - 1).map(into_page_token),
        ))
    } else {
        Ok((models, None))
    }
}

fn words_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[a-zA-Z0-9]+").unwrap())
}

fn prepare_ts_query(query: &str) -> String {
    words_regex()
        .find_iter(query.trim())
        .map(|w| format!("{}:*", w.as_str()))
        .collect::<Vec<String>>()
        .join(" & ")
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
