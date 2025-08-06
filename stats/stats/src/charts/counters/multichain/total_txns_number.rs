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
use multichain_aggregator_entity::{hashes, sea_orm_active_enums::HashType};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect};

pub struct TotalTxnsNumberQueryBehaviour;

impl RemoteQueryBehaviour for TotalTxnsNumberQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let db = cx.indexer_db;
        let timespan = cx.time;

        let value = hashes::Entity::find()
            .select_only()
            .filter(hashes::Column::HashType.eq(HashType::Transaction))
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

pub type TotalTxnsNumberRemote = RemoteDatabaseSource<TotalTxnsNumberQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalTxnsNumber".into()
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

pub type TotalTxnsNumber =
    DirectPointLocalDbChartSource<TotalTxnsNumberRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_txns_number() {
        simple_test_counter_multichain::<TotalTxnsNumber>(
            "update_total_txns_number",
            "50",
            Some(dt("2022-08-06T00:00:00")),
        )
        .await;
    }
}
