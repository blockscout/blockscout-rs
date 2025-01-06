use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    range::UniversalRange,
    types::timespans::DateValue,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};

use blockscout_db::entity::addresses;
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;

pub struct TotalContractsQueryBehaviour;

impl RemoteQueryBehaviour for TotalContractsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let value = addresses::Entity::find()
            .filter(addresses::Column::ContractCode.is_not_null())
            .filter(addresses::Column::InsertedAt.lte(cx.time))
            .count(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?;
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
        simple_test_counter::<TotalContracts>("update_total_contracts", "23", None).await;
    }
}
