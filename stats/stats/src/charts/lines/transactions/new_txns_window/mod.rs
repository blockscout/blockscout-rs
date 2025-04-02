use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::Statement;

use crate::{
    charts::db_interaction::read::cached::find_all_cached,
    data_source::{
        kinds::remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        types::BlockscoutMigrations,
        UpdateContext,
    },
    range::UniversalRange,
    types::{new_txns::NewTxnsCombinedPoint, Timespan, TimespanDuration},
    utils::day_start,
    ChartError, ChartKey,
};

pub mod all_new_txns_window;
pub mod op_stack_operational;

pub use all_new_txns_window::{NewTxnsWindow, NewTxnsWindowInt};

use super::new_txns::NewTxnsCombinedStatement;

pub const WINDOW: u64 = 30;

fn new_txns_window_combined_statement(
    update_day: NaiveDate,
    completed_migrations: &BlockscoutMigrations,
    enabled_update_charts_recursive: &HashSet<ChartKey>,
) -> Statement {
    // `update_day` is not included because the data would
    // be incomplete.
    let window =
        day_start(&update_day.saturating_sub(TimespanDuration::from_timespan_repeats(WINDOW)))
            ..day_start(&update_day);
    NewTxnsCombinedStatement::get_statement(
        Some(window),
        completed_migrations,
        enabled_update_charts_recursive,
    )
}

pub struct NewTxnsWindowCombinedQuery;

impl RemoteQueryBehaviour for NewTxnsWindowCombinedQuery {
    type Output = Vec<NewTxnsCombinedPoint>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Vec<NewTxnsCombinedPoint>, ChartError> {
        let update_day = cx.time.date_naive();
        let query = new_txns_window_combined_statement(
            update_day,
            &cx.blockscout_applied_migrations,
            &cx.enabled_update_charts_recursive,
        );
        let data = find_all_cached::<NewTxnsCombinedPoint>(cx, query).await?;
        Ok(data)
    }
}

pub type NewTxnsWindowCombinedRemote = RemoteDatabaseSource<NewTxnsWindowCombinedQuery>;
