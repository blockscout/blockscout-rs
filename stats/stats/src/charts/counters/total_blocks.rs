use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{QueryBehaviour, RemoteDatabaseSource},
        },
        types::UpdateContext,
    },
    ChartProperties, DateValueString, MissingDatePolicy, Named, UpdateError,
};

use blockscout_db::entity::blocks;
use chrono::NaiveDateTime;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, sea_query::Expr, FromQueryResult, QuerySelect};

#[derive(FromQueryResult)]
struct TotalBlocksData {
    number: i64,
    timestamp: NaiveDateTime,
}

pub struct TotalBlocksQueryBehaviour;

impl QueryBehaviour for TotalBlocksQueryBehaviour {
    type Output = DateValueString;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        let data = blocks::Entity::find()
            .select_only()
            .column_as(Expr::col(blocks::Column::Number).count(), "number")
            .column_as(Expr::col(blocks::Column::Timestamp).max(), "timestamp")
            .filter(blocks::Column::Consensus.eq(true))
            .into_model::<TotalBlocksData>()
            .one(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?
            .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;

        let data = DateValueString {
            date: data.timestamp.date(),
            value: data.number.to_string(),
        };
        Ok(data)
    }
}

pub type TotalBlocksRemote = RemoteDatabaseSource<TotalBlocksQueryBehaviour>;

pub struct TotalBlocksProperties;

impl Named for TotalBlocksProperties {
    const NAME: &'static str = "totalBlocks";
}

impl ChartProperties for TotalBlocksProperties {
    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalBlocks = DirectPointLocalDbChartSource<TotalBlocksRemote, TotalBlocksProperties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data_source::{DataSource, UpdateContext, UpdateParameters},
        get_raw_counters,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Named,
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
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_raw_counters(&db).await.unwrap();
        assert_eq!("13", data[TotalBlocks::NAME].value);
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
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_raw_counters(&db).await.unwrap();
        assert_eq!("9", data[TotalBlocks::NAME].value);
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
            update_time_override: Some(current_time),
            force_full: true,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalBlocks::update_recursively(&cx).await.unwrap();
        let data = get_raw_counters(&db).await.unwrap();
        assert_eq!("13", data[TotalBlocks::NAME].value);
    }
}
