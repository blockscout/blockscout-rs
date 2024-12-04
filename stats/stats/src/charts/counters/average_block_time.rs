use std::cmp::Reverse;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    range::UniversalRange,
    types::TimespanValue,
    utils::NANOS_PER_SEC,
    ChartProperties, MissingDatePolicy, Named, ChartError,
};

use blockscout_db::entity::blocks;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use itertools::Itertools;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, QueryOrder, QuerySelect, Statement};

pub const LIMIT_BLOCKS: u64 = 100;
pub const OFFSET_BLOCKS: u64 = 100;

fn average_block_time_statement(offset: u64) -> Statement {
    blocks::Entity::find()
        .select_only()
        .column_as(Expr::col(blocks::Column::Timestamp), "timestamp")
        // Do not count genesis block because it results in weird block time.
        // We assume that genesis block number is 1 or 0 (just to be safe).
        // If it's not, weird block time will be present only for
        // `LIMIT_BLOCKS` blocks, so it's not a big deal.
        .filter(blocks::Column::Number.gt(1))
        .limit(LIMIT_BLOCKS)
        // top state is considered quite unstable which is not great for computing
        // the metric
        .offset(offset)
        .order_by_desc(blocks::Column::Number)
        // Not configurable because `false` seems to be completely unused
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<BlockTimestamp>()
        .into_statement(DbBackend::Postgres)
}

#[derive(FromQueryResult, Debug)]
struct BlockTimestamp {
    timestamp: chrono::NaiveDateTime,
}

async fn query_average_block_time(
    cx: &UpdateContext<'_>,
    offset: u64,
) -> Result<Option<TimespanValue<NaiveDate, f64>>, ChartError> {
    let query = average_block_time_statement(offset);
    let block_timestamps = BlockTimestamp::find_by_statement(query)
        .all(cx.blockscout.connection.as_ref())
        .await
        .map_err(ChartError::BlockscoutDB)?;
    Ok(calculate_average_block_time(block_timestamps))
}

pub struct AverageBlockTimeQuery;

impl RemoteQueryBehaviour for AverageBlockTimeQuery {
    type Output = TimespanValue<NaiveDate, f64>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<TimespanValue<NaiveDate, f64>, ChartError> {
        match query_average_block_time(cx, OFFSET_BLOCKS).await? {
            Some(avg_block_time) => Ok(avg_block_time),
            None => query_average_block_time(cx, 0)
                .await?
                .ok_or(ChartError::Internal(
                    "No blocks were returned to calculate average block time".into(),
                )),
        }
    }
}

// Time in seconds. `None` if the vector is empty.
fn calculate_average_block_time(
    mut timestamps: Vec<BlockTimestamp>,
) -> Option<TimespanValue<NaiveDate, f64>> {
    // data is expected to be already sorted; in this case
    // the time complexity is linear
    timestamps.sort_unstable_by_key(|x| Reverse(x.timestamp));
    let last_block_date = timestamps.first()?.timestamp.date();
    // ensure it's sorted somehow
    let block_times_s = timestamps
        .iter()
        .tuple_windows::<(_, _)>()
        .map(|(cur, prev)| {
            let time_diff = cur.timestamp - prev.timestamp;
            // formula from `subsec_nanos()` docs
            let diff_ns =
                time_diff.subsec_nanos() as i64 + time_diff.num_seconds() * NANOS_PER_SEC as i64;
            diff_ns as f64 / NANOS_PER_SEC as f64
        })
        .collect_vec();
    let count = block_times_s.len();
    if count == 0 {
        return None;
    }
    let average_block_time_seconds = block_times_s.iter().sum::<f64>() / count as f64;
    Some(TimespanValue {
        timespan: last_block_date,
        value: average_block_time_seconds,
    })
}

pub type AverageBlockTimeRemote = RemoteDatabaseSource<AverageBlockTimeQuery>;
pub type AverageBlockTimeRemoteString = MapToString<AverageBlockTimeRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageBlockTime".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type AverageBlockTime = DirectPointLocalDbChartSource<AverageBlockTimeRemoteString, Properties>;

#[cfg(test)]
mod tests {
    use std::iter::repeat;

    use chrono::TimeDelta;

    use super::*;
    use crate::{
        data_source::{types::BlockscoutMigrations, DataSource, UpdateParameters},
        tests::{
            mock_blockscout::fill_many_blocks,
            simple_test::{get_counter, prepare_chart_test, simple_test_counter},
        },
        utils::MarkedDbConnection,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time() {
        // needs at least `OFFSET_BLOCKS` blocks to calculate
        // and at least `OFFSET_BLOCKS + LIMIT_BLOCKS` blocks to test the limit

        let (current_time, db, blockscout) =
            prepare_chart_test::<AverageBlockTime>("update_average_block_time", None).await;

        let times_generator = [100u64, 200, 300];
        let block_times = repeat(1)
            // genesis is not counted
            // (2 because we consider block 1 as genesis just in case)
            .take(2)
            .chain(
                times_generator
                    .into_iter()
                    .cycle()
                    // -1 since for `N` blocks there are `N - 1` time deltas
                    .take((LIMIT_BLOCKS - 1) as usize),
            )
            // will be skipped
            .chain(repeat(1).take(OFFSET_BLOCKS as usize))
            .map(|x| TimeDelta::seconds(x as i64))
            .collect_vec();
        let expected_avg = {
            let limit_block_times = LIMIT_BLOCKS - 1;
            let generator_len = u64::try_from(times_generator.len()).unwrap();
            // how many times the full `times_generator` sequence is repeated within considered blocks
            let full_generator_repeats = limit_block_times / generator_len;
            let full_repeats_sum = times_generator.iter().sum::<u64>() * full_generator_repeats;
            // how many elements of `times_generator` are taken for the last repeat
            let partial_repeat_elements_taken = limit_block_times % generator_len;
            let partial_repeat_sum = times_generator
                .iter()
                .take(partial_repeat_elements_taken as usize)
                .sum::<u64>();
            assert_eq!(
                partial_repeat_elements_taken + full_generator_repeats * generator_len,
                limit_block_times
            );
            let total_sum = full_repeats_sum + partial_repeat_sum;
            total_sum as f64 / limit_block_times as f64
        };
        fill_many_blocks(&blockscout, current_time.naive_utc(), &block_times).await;
        let mut parameters = UpdateParameters {
            db: &MarkedDbConnection::from_test_db(&db).unwrap(),
            blockscout: &MarkedDbConnection::from_test_db(&blockscout).unwrap(),
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        AverageBlockTime::update_recursively(&cx).await.unwrap();
        assert_eq!(
            expected_avg.to_string(),
            get_counter::<AverageBlockTime>(&cx).await.value
        );
        parameters.force_full = false;
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        AverageBlockTime::update_recursively(&cx).await.unwrap();
        assert_eq!(
            expected_avg.to_string(),
            get_counter::<AverageBlockTime>(&cx).await.value
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time_fallback() {
        // if there are not enough blocks to use offset, calculate from available data
        simple_test_counter::<AverageBlockTime>(
            "update_average_block_time_fallback",
            // first 2 blocks are excluded
            "958320",
            None,
        )
        .await;
    }
}
