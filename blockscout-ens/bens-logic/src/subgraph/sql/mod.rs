mod additional_tables;
mod create;
mod domain;
mod schema_selector;
mod transaction_history;
mod update;
mod utils;

pub use additional_tables::*;
pub use create::*;
pub use domain::*;
pub use schema_selector::*;
pub use transaction_history::*;
pub use update::*;
#[derive(thiserror::Error, Debug)]
pub enum DbErr {
    #[error("db error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}
