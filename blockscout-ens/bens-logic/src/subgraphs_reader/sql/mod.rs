mod cache_views;
mod domain;
mod schema_selector;
mod transaction_history;
mod utils;

pub use cache_views::*;
pub use domain::*;
pub use schema_selector::*;
pub use transaction_history::*;

#[derive(thiserror::Error, Debug)]
pub enum DbErr {
    #[error("db error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
