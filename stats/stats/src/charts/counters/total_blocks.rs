use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        kinds::{
            data_manipulation::map::MapParseTo,
            local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        types::UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};

use blockscout_db::entity::blocks;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, sea_query::Expr, FromQueryResult, QuerySelect};

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
            .one(cx.blockscout)
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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::NoneIndexed
    }
}

pub struct TotalBlocksEstimation;

impl ValueEstimation for TotalBlocksEstimation {
    async fn estimate(blockscout: &DatabaseConnection) -> Result<DateValue<String>, ChartError> {
        // `now()` is more relevant when taken right before the query rather than
        // `cx.time` measured a bit earlier.
        let now = Utc::now();
        let value = query_estimated_table_rows(blockscout, blocks::Entity.table_name())
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

pub type TotalBlocks =
    DirectPointLocalDbChartSourceWithEstimate<TotalBlocksRemote, TotalBlocksEstimation, Properties>;
pub type TotalBlocksInt = MapParseTo<TotalBlocks, i64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data_source::{types::BlockscoutMigrations, DataSource, UpdateContext, UpdateParameters},
        tests::{
            init_db::init_db_all,
            mock_blockscout::fill_mock_blockscout_data,
            simple_test::{get_counter, test_counter_fallback},
        },
    };
    use chrono::NaiveDate;
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseConnection, EntityTrait, Set};
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_total_blocks_recurrent").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, current_date).await;

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
        let (db, blockscout) = init_db_all("update_total_blocks_fresh").await;
        let current_time = chrono::DateTime::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

        fill_mock_blockscout_data(&blockscout, current_date).await;

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
        let (db, blockscout) = init_db_all("update_total_blocks_last").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();

        TotalBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
            value: Set(1.to_string()),
            ..Default::default()
        })
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, current_date).await;

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
        test_counter_fallback::<TotalBlocks>("total_blocks_fallback").await;
    }
}
