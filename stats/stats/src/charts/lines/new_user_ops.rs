use std::ops::Range;

use crate::{
    charts::db_interaction::{read::QueryAllBlockTimestampRange, utils::datetime_range_filter},
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
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
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    ChartProperties, Named,
};

use blockscout_db::entity::{blocks, user_operations};
use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use migration::{Alias, Asterisk, Expr, Func, IntoColumnRef, IntoIden};
use sea_orm::{EntityTrait, IntoIdentity, IntoSimpleExpr, QuerySelect, QueryTrait, Statement};

pub struct NewUserOpsStatement;

impl StatementFromRange for NewUserOpsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        let date_intermediate_col = "date".into_identity();
        let mut query = user_operations::Entity::find()
            .select_only()
            .join(
                sea_orm::JoinType::InnerJoin,
                user_operations::Entity::belongs_to(blocks::Entity)
                    .from(user_operations::Column::BlockHash)
                    .to(blocks::Column::Hash)
                    .into(),
            )
            .expr_as(
                blocks::Column::Timestamp
                    .into_simple_expr()
                    .cast_as(Alias::new("date")),
                date_intermediate_col.clone(),
            )
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .group_by(Expr::col(date_intermediate_col.into_iden()));
        if let Some(range) = range {
            query = datetime_range_filter(query, blocks::Column::Timestamp, &range);
        }
        query.build(sea_orm::DatabaseBackend::Postgres)
    }
}

pub type NewUserOpsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<NewUserOpsStatement, NaiveDate, String, QueryAllBlockTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newUserOps".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewUserOps = DirectVecLocalDbChartSource<NewUserOpsRemote, Batch30Days, Properties>;
pub type NewUserOpsInt = MapParseTo<StripExt<NewUserOps>, i64>;
pub type NewUserOpsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewUserOpsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewUserOpsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewUserOpsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewUserOpsMonthlyInt = MapParseTo<StripExt<NewUserOpsMonthly>, i64>;
pub type NewUserOpsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewUserOpsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{
        ranged_test_chart_with_migration_variants, simple_test_chart_with_migration_variants,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_user_ops() {
        simple_test_chart_with_migration_variants::<NewUserOps>(
            "update_new_user_ops",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_user_ops_weekly() {
        simple_test_chart_with_migration_variants::<NewUserOpsWeekly>(
            "update_new_user_ops_weekly",
            vec![
                ("2022-11-07", "36"),
                ("2022-11-28", "5"),
                ("2022-12-26", "1"),
                ("2023-01-30", "4"),
                ("2023-02-27", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_user_ops_monthly() {
        simple_test_chart_with_migration_variants::<NewUserOpsMonthly>(
            "update_new_user_ops_monthly",
            vec![
                ("2022-11-01", "36"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_user_ops_yearly() {
        simple_test_chart_with_migration_variants::<NewUserOpsYearly>(
            "update_new_user_ops_yearly",
            vec![("2022-01-01", "41"), ("2023-01-01", "6")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_user_ops() {
        ranged_test_chart_with_migration_variants::<NewUserOps>(
            "ranged_update_new_user_ops",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ],
            "2022-11-08".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
