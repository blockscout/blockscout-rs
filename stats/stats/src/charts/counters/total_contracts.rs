use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    types::DateValue,
    ChartProperties, MissingDatePolicy, Named, UpdateError,
};
use blockscout_db::entity::addresses;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;

pub struct TotalContractsQueryBehaviour;

impl RemoteQueryBehaviour for TotalContractsQueryBehaviour {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: Option<Range<DateTimeUtc>>,
    ) -> Result<Self::Output, UpdateError> {
        let value = addresses::Entity::find()
            .filter(addresses::Column::ContractCode.is_not_null())
            .filter(addresses::Column::InsertedAt.lte(cx.time))
            .count(cx.blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let timespan = cx.time.date_naive();
        Ok(DateValue::<String> {
            timespan,
            value: value.to_string(),
        })
    }
}

pub type TotalContractsRemote = RemoteDatabaseSource<TotalContractsQueryBehaviour>;

pub struct TotalContractsProperties;

impl Named for TotalContractsProperties {
    const NAME: &'static str = "totalContracts";
}

impl ChartProperties for TotalContractsProperties {
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
pub type TotalContracts =
    DirectPointLocalDbChartSource<TotalContractsRemote, TotalContractsProperties>;

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
