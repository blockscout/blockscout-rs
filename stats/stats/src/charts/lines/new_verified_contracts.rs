use std::ops::Range;

use crate::{
    data_source::kinds::{
        data_manipulation::{
            map::{MapParseTo, MapToString},
            resolutions::sum::SumLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    delegated_properties_with_resolutions,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct NewVerifiedContractsStatement;

impl StatementFromRange for NewVerifiedContractsStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
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
    RemoteDatabaseSource<PullAllWithAndSort<NewVerifiedContractsStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newVerifiedContracts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

delegated_properties_with_resolutions!(
    delegate: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    }
    ..Properties
);

pub type NewVerifiedContracts =
    DirectVecLocalDbChartSource<NewVerifiedContractsRemote, Batch30Days, Properties>;
pub type NewVerifiedContractsInt = MapParseTo<NewVerifiedContracts, i64>;
pub type NewVerifiedContractsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewVerifiedContractsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewVerifiedContractsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewVerifiedContractsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewVerifiedContractsMonthlyInt = MapParseTo<NewVerifiedContractsMonthly, i64>;
pub type NewVerifiedContractsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewVerifiedContractsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
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

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_verified_contracts_weekly() {
        simple_test_chart::<NewVerifiedContractsWeekly>(
            "update_new_verified_contracts_weekly",
            vec![("2022-11-14", "3")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_verified_contracts_monthly() {
        simple_test_chart::<NewVerifiedContractsMonthly>(
            "update_new_verified_contracts_monthly",
            vec![("2022-11-01", "3")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_verified_contracts_yearly() {
        simple_test_chart::<NewVerifiedContractsYearly>(
            "update_new_verified_contracts_yearly",
            vec![("2022-01-01", "3")],
        )
        .await;
    }
}
