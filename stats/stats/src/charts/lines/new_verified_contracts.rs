use crate::{
    charts::db_interaction::types::DateValueInt,
    data_source::kinds::{adapter::parse::MapParseTo, updateable_chart::clone::CloneChartWrapper},
};

mod _inner {
    use std::ops::RangeInclusive;

    use crate::{
        data_source::kinds::{
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
            updateable_chart::clone::CloneChart,
        },
        utils::sql_with_range_filter_opt,
        Chart, DateValueString, Named,
    };
    use entity::sea_orm_active_enums::ChartType;
    use sea_orm::{prelude::*, DbBackend, Statement};

    pub struct NewVerifiedContractsStatement;

    impl StatementFromRange for NewVerifiedContractsStatement {
        fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(smart_contracts.inserted_at) as date,
                        COUNT(*)::TEXT as value
                    FROM smart_contracts
                    WHERE TRUE {filter}
                    GROUP BY DATE(smart_contracts.inserted_at)
                "#,
                [],
                "smart_contracts.inserted_at",
                range
            )
        }
    }

    pub type NewVerifiedContractsRemote =
        RemoteDatabaseSource<PullAllWithAndSort<NewVerifiedContractsStatement, DateValueString>>;

    pub struct NewVerifiedContractsInner;

    impl Named for NewVerifiedContractsInner {
        const NAME: &'static str = "newVerifiedContracts";
    }

    impl Chart for NewVerifiedContractsInner {
        fn chart_type() -> ChartType {
            ChartType::Line
        }
    }

    impl CloneChart for NewVerifiedContractsInner {
        type Dependency = NewVerifiedContractsRemote;
    }
}
pub type NewVerifiedContracts = CloneChartWrapper<_inner::NewVerifiedContractsInner>;

pub type NewVerifiedContractsInt = MapParseTo<NewVerifiedContracts, DateValueInt>;

#[cfg(test)]
mod tests {
    use super::NewVerifiedContracts;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_verified_contracts() {
        simple_test_chart::<NewVerifiedContracts>(
            "update_new_verified_contracts",
            vec![
                ("2022-11-14", "1"),
                ("2022-11-15", "1"),
                ("2022-11-16", "1"),
            ],
        )
        .await;
    }
}
