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
use multichain_aggregator_entity::addresses;
use sea_orm::{EntityTrait, PaginatorTrait, QuerySelect};

pub struct TotalAddressesNumberQueryBehaviour;

impl RemoteQueryBehaviour for TotalAddressesNumberQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let db = cx.indexer_db;
        let timespan = cx.time;

        let value = addresses::Entity::find()
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

pub type TotalAddressesNumberRemote = RemoteDatabaseSource<TotalAddressesNumberQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalAddressesNumber".into()
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

pub type TotalAddressesNumber =
    DirectPointLocalDbChartSource<TotalAddressesNumberRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_multichain;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_addresses_number() {
        simple_test_counter_multichain::<TotalAddressesNumber>(
            "update_total_addresses_number",
            "3",
            None,
        )
        .await;
    }
}
