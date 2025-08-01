use crate::{
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        UpdateContext,
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
    },
    indexing_status::{BlockscoutIndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus},
    range::UniversalRange,
    types::timespans::DateValue,
};

use blockscout_db::entity::addresses;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{QuerySelect, prelude::*};

pub struct TotalContractsQueryBehaviour;

impl RemoteQueryBehaviour for TotalContractsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let value = addresses::Entity::find()
            .select_only()
            .filter(addresses::Column::ContractCode.is_not_null())
            // seems to not introduce a significant performance penalty
            .filter(addresses::Column::InsertedAt.lte(cx.time))
            .count(cx.indexer_db)
            .await
            .map_err(ChartError::IndexerDB)?;
        let timespan = cx.time.date_naive();
        Ok(DateValue::<String> {
            timespan,
            value: value.to_string(),
        })
    }
}

pub type TotalContractsRemote = RemoteDatabaseSource<TotalContractsQueryBehaviour>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalContracts".into()
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
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::NoneIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        }
    }
}

// todo: reconsider once #845 is solved
// https://github.com/blockscout/blockscout-rs/issues/845
// i.e. set dependency to LastPointChart<ContractsGrowth>
pub type TotalContracts = DirectPointLocalDbChartSource<TotalContractsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_contracts() {
        simple_test_counter::<TotalContracts>("update_total_contracts", "25", None).await;
    }
}
