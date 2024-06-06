use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use crate::{
        data_source::{
            kinds::{
                remote::point::{RemotePointSource, RemotePointSourceWrapper},
                updateable_chart::{clone::point::ClonePointChart, last_point::LastPointChart},
            },
            UpdateContext,
        },
        lines::ContractsGrowth,
        Chart, DateValueString, Named, UpdateError,
    };
    use blockscout_db::entity::addresses;
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::prelude::*;

    pub struct TotalContractsRemote;

    impl RemotePointSource for TotalContractsRemote {
        type Point = DateValueString;
        fn get_query() -> sea_orm::Statement {
            unreachable!("must not be called")
        }

        async fn query_data(cx: &UpdateContext<'_>) -> Result<Self::Point, UpdateError> {
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

    pub struct TotalContractsInnerFixed;

    impl Named for TotalContractsInnerFixed {
        const NAME: &'static str = "totalContracts";
    }

    impl Chart for TotalContractsInnerFixed {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl ClonePointChart for TotalContractsInnerFixed {
        type Dependency = RemotePointSourceWrapper<TotalContractsRemote>;
    }

    pub struct TotalContractsInner;

    impl LastPointChart for TotalContractsInner {
        type InnerSource = ContractsGrowth;
    }
}

// todo: reconsider once #845 is solved
// https://github.com/blockscout/blockscout-rs/issues/845
// + change update group once changed back
pub type TotalContracts = ClonePointChartWrapper<_inner::TotalContractsInnerFixed>;

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
