pub mod batching;
pub mod clear_and_query_all;
pub mod point;

pub use point::PassPoint;
use sea_orm::{ConnectionTrait, TransactionTrait};

use crate::{
    charts::db_interaction::write::insert_data_many,
    types::{Timespan, TimespanValue},
    ChartError,
};

pub(crate) async fn pass_vec<Resolution, C>(
    db: &C,
    chart_id: i32,
    min_blockscout_block: i64,
    main_data: Vec<TimespanValue<Resolution, String>>,
) -> Result<usize, ChartError>
where
    Resolution: Timespan + Clone + Send + Sync,
    C: ConnectionTrait + TransactionTrait,
{
    let found = main_data.len();
    // note: right away cloning another chart will not result in exact copy,
    // because if the other chart is `FillPrevious`, then omitted starting point
    // within the range is set to the last known before the range
    // i.e. some duplicate points might get added.
    //
    // however, it should not be a problem, since semantics remain the same +
    // cloning already stored chart is counter-productive/not effective.
    let values = main_data
        .into_iter()
        .map(|value| value.active_model(chart_id, Some(min_blockscout_block)));
    insert_data_many(db, values)
        .await
        .map_err(ChartError::StatsDB)?;
    Ok(found)
}
