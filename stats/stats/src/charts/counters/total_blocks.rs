use std::sync::OnceLock;

use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        kinds::{
            local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        types::UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    utils::MarkedDbConnection,
    ChartProperties, MissingDatePolicy, Named, ChartError,
};

use blockscout_db::entity::blocks;
use cached::Cached;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, sea_query::Expr, FromQueryResult, QuerySelect};
use tokio::sync::Mutex;

#[derive(FromQueryResult)]
struct TotalBlocksData {
    number: i64,
    timestamp: NaiveDateTime,
}

pub struct TotalBlocksQueryBehaviour;

impl RemoteQueryBehaviour for TotalBlocksQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let data = blocks::Entity::find()
            .select_only()
            .column_as(Expr::col(blocks::Column::Number).count(), "number")
            .column_as(Expr::col(blocks::Column::Timestamp).max(), "timestamp")
            .filter(blocks::Column::Consensus.eq(true))
            .into_model::<TotalBlocksData>()
            .one(cx.blockscout.connection.as_ref())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .ok_or_else(|| ChartError::Internal("query returned nothing".into()))?;

        let data = DateValue::<String> {
            timespan: data.timestamp.date(),
            value: data.number.to_string(),
        };
        Ok(data)
    }
}

pub type TotalBlocksRemote = RemoteDatabaseSource<TotalBlocksQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalBlocks".into()
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

pub struct CachedBlocksEstimation;

const TOTAL_BLOCKS_ESTIMATION_CACHE_LIVENESS_SEC_DEFAULT: u64 = 10;
// so that it can be added to settings if necessary
pub static TOTAL_BLOCKS_ESTIMATION_CACHE_LIVENESS_SEC: OnceLock<u64> = OnceLock::new();
static CACHED_BLOCKS_ESTIMATION: OnceLock<Mutex<cached::TimedCache<String, i64>>> = OnceLock::new();

impl ValueEstimation for CachedBlocksEstimation {
    async fn estimate(blockscout: &MarkedDbConnection) -> Result<DateValue<String>, ChartError> {
        async fn cached_blocks_estimation(
            blockscout: &DatabaseConnection,
            db_id: &str,
        ) -> Result<Option<i64>, DbErr> {
            let mut cache = CACHED_BLOCKS_ESTIMATION
                .get_or_init(|| {
                    Mutex::new(cached::TimedCache::with_lifespan_and_refresh(
                        *TOTAL_BLOCKS_ESTIMATION_CACHE_LIVENESS_SEC
                            .get_or_init(|| TOTAL_BLOCKS_ESTIMATION_CACHE_LIVENESS_SEC_DEFAULT),
                        false,
                    ))
                })
                .lock()
                .await;
            if let Some(cached_estimate) = cache.cache_get(db_id) {
                return Ok(Some(*cached_estimate));
            }

            let query_result =
                query_estimated_table_rows(blockscout, blocks::Entity.table_name()).await;
            if let Ok(Some(estimate)) = &query_result {
                cache.cache_set(db_id.to_string(), *estimate);
            }
            query_result
        }

        let now = Utc::now();
        let value = cached_blocks_estimation(blockscout.connection.as_ref(), &blockscout.db_name)
            .await
            .map_err(ChartError::BlockscoutDB)?
            .map(|b| {
                let b = b as f64 * 0.9;
                b as i64
            })
            .unwrap_or(0);
        Ok(DateValue {
            timespan: now.date_naive(),
            value: value.to_string(),
        })
    }
}

pub type TotalBlocks = DirectPointLocalDbChartSourceWithEstimate<
    TotalBlocksRemote,
    CachedBlocksEstimation,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data_source::{types::BlockscoutMigrations, DataSource, UpdateContext, UpdateParameters},
        tests::{
            init_db::init_marked_db_all, mock_blockscout::fill_mock_blockscout_data,
            simple_test::get_counter,
        },
    };
    use chrono::NaiveDate;
    use entity::chart_data;
    use pretty_assertions::{assert_eq, assert_ne};
    use sea_orm::{DatabaseConnection, DbBackend, EntityTrait, Set, Statement};
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_marked_db_all("update_total_blocks_recurrent").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db.connection, &current_time)
            .await
            .unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db.connection as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout.connection, current_date).await;

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_counter::<TotalBlocks>(&cx).await;
        assert_eq!("13", data.value);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_marked_db_all("update_total_blocks_fresh").await;
        let current_time = chrono::DateTime::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db.connection, &current_time)
            .await
            .unwrap();

        fill_mock_blockscout_data(&blockscout.connection, current_date).await;

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_counter::<TotalBlocks>(&cx).await;
        assert_eq!("9", data.value);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_marked_db_all("update_total_blocks_last").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db.connection, &current_time)
            .await
            .unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db.connection as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout.connection, current_date).await;

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_counter::<TotalBlocks>(&cx).await;
        assert_eq!("13", data.value);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_blocks_fallback() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_marked_db_all("total_blocks_fallback").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db.connection, &current_time)
            .await
            .unwrap();

        fill_mock_blockscout_data(&blockscout.connection, current_date).await;

        // need to analyze or vacuum for `reltuples` to be updated.
        // source: https://www.postgresql.org/docs/9.3/planner-stats.html
        let _ = blockscout
            .connection
            .execute(Statement::from_string(DbBackend::Postgres, "ANALYZE;"))
            .await
            .unwrap();

        let parameters = UpdateParameters {
            db: &db,
            blockscout: &blockscout,
            blockscout_applied_migrations: BlockscoutMigrations::latest(),
            update_time_override: Some(current_time),
            force_full: false,
        };
        let cx: UpdateContext<'_> = UpdateContext::from_params_now_or_override(parameters.clone());
        let data = get_counter::<TotalBlocks>(&cx).await;
        assert_ne!("0", data.value);
    }
}
