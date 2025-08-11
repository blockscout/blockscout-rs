use crate::{
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        UpdateContext,
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
    },
    indexing_status::IndexingStatusTrait,
    range::UniversalRange,
    types::timespans::DateValue,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use multichain_aggregator_entity::interop_messages;
use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect};

pub struct TotalInteropMessagesQueryBehaviour;

impl RemoteQueryBehaviour for TotalInteropMessagesQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let db = cx.indexer_db;
        let timespan = cx.time;

        let value = interop_messages::Entity::find()
            .select_only()
            .count(db)
            .await
            .map_err(ChartError::IndexerDB)?;

        let data = DateValue::<String> {
            timespan: timespan.date_naive(),
            value: value.to_string(),
        };
        Ok(data)
    }
}

pub type TotalInteropMessagesRemote = RemoteDatabaseSource<TotalInteropMessagesQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInteropMessages".into()
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
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type TotalInteropMessages =
    DirectPointLocalDbChartSource<TotalInteropMessagesRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interop_messages() {
        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "6",
            Some(dt("2022-08-06T00:00:00")),
        )
        .await;
    }
}
