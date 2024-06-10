use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use std::ops::RangeInclusive;

    use crate::{
        data_source::{
            kinds::{
                remote_db::{QueryBehaviour, RemoteDatabaseSource},
                updateable_chart::clone::point::ClonePointChart,
            },
            UpdateContext,
        },
        Chart, DateValueString, Named, UpdateError,
    };
    use blockscout_db::entity::addresses;
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::prelude::*;

    pub struct TotalContractsQueryBehaviour;

    impl QueryBehaviour for TotalContractsQueryBehaviour {
        type Output = DateValueString;

        async fn query_data(
            cx: &UpdateContext<'_>,
            _range: Option<RangeInclusive<DateTimeUtc>>,
        ) -> Result<Self::Output, UpdateError> {
            let value = addresses::Entity::find()
                .filter(addresses::Column::ContractCode.is_not_null())
                .filter(addresses::Column::InsertedAt.lte(cx.time))
                .count(cx.blockscout)
                .await
                .map_err(UpdateError::BlockscoutDB)?;
            let date = cx.time.date_naive();
            Ok(DateValueString {
                date,
                value: value.to_string(),
            })
        }
    }

    pub type TotalContractsRemote = RemoteDatabaseSource<TotalContractsQueryBehaviour>;

    pub struct TotalContractsInner;

    impl Named for TotalContractsInner {
        const NAME: &'static str = "totalContracts";
    }

    impl Chart for TotalContractsInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl ClonePointChart for TotalContractsInner {
        // todo: reconsider once #845 is solved
        // https://github.com/blockscout/blockscout-rs/issues/845
        // + change update group once changed back
        // i.e. set to LastPointChart<ContractsGrowth>
        type Dependency = TotalContractsRemote;
    }
}

pub type TotalContracts = ClonePointChartWrapper<_inner::TotalContractsInner>;

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
