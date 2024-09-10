use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
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
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::DateTimeUtc, DbBackend, Statement};

pub struct NewContractsStatement;

impl StatementFromRange for NewContractsStatement {
    fn get_statement(
        range: Option<Range<DateTimeUtc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            let (tx_filter, mut args) =
                produce_filter_and_values(range.clone(), "t.block_timestamp", 1);
            let (block_filter, new_args) =
                produce_filter_and_values(range.clone(), "b.timestamp", 3);
            args.extend(new_args);
            let sql = format!(
                r#"
                    SELECT day AS date, COUNT(*)::text AS value
                    FROM (
                        SELECT 
                            DISTINCT ON (txns_plus_internal_txns.hash)
                            txns_plus_internal_txns.day
                        FROM (
                            SELECT
                                t.created_contract_address_hash AS hash,
                                t.block_timestamp::date AS day
                            FROM transactions t
                            WHERE
                                t.created_contract_address_hash NOTNULL AND
                                t.block_consensus = TRUE AND
                                t.block_timestamp != to_timestamp(0) {tx_filter}
                            UNION
                            SELECT
                                it.created_contract_address_hash AS hash,
                                b.timestamp::date AS day
                            FROM internal_transactions it
                                JOIN blocks b ON b.hash = it.block_hash
                            WHERE
                                it.created_contract_address_hash NOTNULL AND
                                b.consensus = TRUE AND
                                b.timestamp != to_timestamp(0) {block_filter}
                        ) txns_plus_internal_txns
                    ) sub
                    GROUP BY sub.day;
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT day AS date, COUNT(*)::text AS value
                    FROM (
                        SELECT 
                            DISTINCT ON (txns_plus_internal_txns.hash)
                            txns_plus_internal_txns.day
                        FROM (
                            SELECT
                                t.created_contract_address_hash AS hash,
                                b.timestamp::date AS day
                            FROM transactions t
                                JOIN blocks b ON b.hash = t.block_hash
                            WHERE
                                t.created_contract_address_hash NOTNULL AND
                                b.consensus = TRUE AND
                                b.timestamp != to_timestamp(0) {filter}
                            UNION
                            SELECT
                                it.created_contract_address_hash AS hash,
                                b.timestamp::date AS day
                            FROM internal_transactions it
                                JOIN blocks b ON b.hash = it.block_hash
                            WHERE
                                it.created_contract_address_hash NOTNULL AND
                                b.consensus = TRUE AND
                                b.timestamp != to_timestamp(0) {filter}
                        ) txns_plus_internal_txns
                    ) sub
                    GROUP BY sub.day;
                "#,
                [],
                "b.timestamp",
                range,
            )
        }
    }
}

pub type NewContractsRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewContractsStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newContracts".into()
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

pub type NewContracts = DirectVecLocalDbChartSource<NewContractsRemote, Batch30Days, Properties>;
pub type NewContractsInt = MapParseTo<NewContracts, i64>;
pub type NewContractsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewContractsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewContractsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewContractsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewContractsMonthlyInt = MapParseTo<NewContractsMonthly, i64>;
pub type NewContractsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewContractsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{
        point_construction::{d, dt},
        simple_test::{
            ranged_test_chart_with_migration_variants, simple_test_chart_with_migration_variants,
        },
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts() {
        simple_test_chart_with_migration_variants::<NewContracts>(
            "update_new_contracts",
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "6"),
                ("2022-11-11", "8"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts_weekly() {
        simple_test_chart_with_migration_variants::<NewContractsWeekly>(
            "update_new_contracts_weekly",
            vec![
                ("2022-11-07", "19"),
                ("2022-11-28", "2"),
                ("2022-12-26", "1"),
                ("2023-01-30", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts_monthly() {
        simple_test_chart_with_migration_variants::<NewContractsMonthly>(
            "update_new_contracts_monthly",
            vec![
                ("2022-11-01", "19"),
                ("2022-12-01", "2"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts_yearly() {
        simple_test_chart_with_migration_variants::<NewContractsYearly>(
            "update_new_contracts_yearly",
            vec![("2022-01-01", "21"), ("2023-01-01", "2")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_contracts() {
        ranged_test_chart_with_migration_variants::<NewContracts>(
            "ranged_update_new_contracts",
            vec![
                ("2022-11-11", "8"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
            ],
            d("2022-11-11"),
            d("2022-12-01"),
            Some(dt("2022-12-01T12:00:00")),
        )
        .await;
    }
}
