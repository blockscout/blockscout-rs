use sea_orm::DatabaseConnection;

use crate::{ChartError, data_source::UpdateContext};

trait DBChoiceInner {
    fn select_db<'a>(cx: &'a UpdateContext<'_>) -> Result<&'a DatabaseConnection, ChartError>;
}

pub struct UseBlockscoutDB;

impl DBChoiceInner for UseBlockscoutDB {
    fn select_db<'a>(cx: &'a UpdateContext<'_>) -> Result<&'a DatabaseConnection, ChartError> {
        Ok(cx.indexer_db)
    }
}

pub struct UseZetachainCctxDB;

impl DBChoiceInner for UseZetachainCctxDB {
    fn select_db<'a>(cx: &'a UpdateContext<'_>) -> Result<&'a DatabaseConnection, ChartError> {
        cx.second_indexer_db.ok_or(ChartError::Internal(
            "Zetachain CCTX DB is not available".into(),
        ))
    }
}

/// Use `impl_db_choice!` macro to implement this trait for your type concisely.
pub trait DatabaseChoice {
    type DB: DBChoiceInner;

    fn get_db<'a>(cx: &'a UpdateContext<'_>) -> Result<&'a DatabaseConnection, ChartError> {
        Self::DB::select_db(cx)
    }
}

macro_rules! impl_db_choice {
    ($name:ident, $choice:path) => {
        impl $crate::data_source::kinds::remote_db::db_choice::DatabaseChoice for $name {
            type DB = $choice;
        }
    };
}

pub(crate) use impl_db_choice;
