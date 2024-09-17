//! Accounts that are active during the period + were active during some previous time

use std::{marker::PhantomData, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                filter_deducible::FilterDeducible,
                map::{MapParseTo, MapToString},
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{PullEachWith, RemoteDatabaseSource, StatementFromTimespan},
        },
        types::{BlockscoutMigrations, Get},
    },
    define_and_impl_resolution_properties, gettable_const,
    types::timespans::{Month, Week, Year},
    utils::produce_filter_and_values,
    ChartProperties, Named,
};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct ActiveRecurringAccountsStatement<Recurrance>(PhantomData<Recurrance>);

impl<Recurrance: RecurrancePeriod> StatementFromTimespan
    for ActiveRecurringAccountsStatement<Recurrance>
{
    fn get_statement(
        range: Range<DateTime<Utc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        let recurrance_range = Recurrance::generate(range.clone());
        if completed_migrations.denormalization {
            let (current_activity_range, mut args) =
                produce_filter_and_values(Some(range), "block_timestamp", 1);
            let (recurring_activity_range, new_args) =
                produce_filter_and_values(Some(recurrance_range), "block_timestamp", 3);
            args.extend(new_args);

            let sql = format!(
                r#"
                    SELECT COUNT(*)::TEXT as value
                    FROM (
                        SELECT
                            DISTINCT from_address_hash
                        FROM transactions
                        WHERE
                            block_timestamp != to_timestamp(0) AND
                            block_consensus = true {current_activity_range}
                        INTERSECT
                        SELECT
                            DISTINCT from_address_hash
                        FROM transactions
                        WHERE
                            block_timestamp != to_timestamp(0) AND
                            block_consensus = true {recurring_activity_range}
                    ) recurring_active_addresses;
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        } else {
            let (current_activity_range, mut args) =
                produce_filter_and_values(Some(range), "b.timestamp", 1);
            let (recurring_activity_range, new_args) =
                produce_filter_and_values(Some(recurrance_range), "b.timestamp", 3);
            args.extend(new_args);

            let sql = format!(
                r#"
                    SELECT COUNT(*)::TEXT as value
                    FROM (
                        SELECT
                            DISTINCT t.from_address_hash
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true {current_activity_range}
                        INTERSECT
                        SELECT
                            DISTINCT t.from_address_hash
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        WHERE
                            b.timestamp != to_timestamp(0) AND
                            b.consensus = true {recurring_activity_range}
                    ) recurring_active_addresses;
                "#,
            );
            Statement::from_sql_and_values(DbBackend::Postgres, sql, args)
        }
    }
}

trait RecurrancePeriod {
    fn generate(current_period: Range<DateTime<Utc>>) -> Range<DateTime<Utc>>;
}

impl<N: Get<Value = Duration>> RecurrancePeriod for N {
    fn generate(current_period: Range<DateTime<Utc>>) -> Range<DateTime<Utc>> {
        let start = current_period
            .start
            .checked_sub_signed(N::get())
            .unwrap_or(DateTime::<Utc>::MIN_UTC);
        start..current_period.start
    }
}

gettable_const!(Recurrance60Days: Duration = Duration::days(60));
gettable_const!(Recurrance90Days: Duration = Duration::days(90));
gettable_const!(Recurrance120Days: Duration = Duration::days(120));

pub type ActiveRecurringAccountsRemote<Recurrance, Resolution> = RemoteDatabaseSource<
    PullEachWith<
        ActiveRecurringAccountsStatement<Recurrance>,
        Resolution,
        String,
        QueryAllBlockTimestampRange,
    >,
>;

macro_rules! impl_all_properties_for_recurrance {
    (
        name_suffix: $name_suffix:expr,
        day: $day_type:ident,
        week: $week_type:ident,
        month: $month_type:ident,
        year: $year_type:ident $(,)?
    ) => {
        pub struct $day_type;

        impl Named for $day_type {
            fn name() -> String {
                format!("activeRecurringAccounts{}", $name_suffix)
            }
        }

        impl ChartProperties for $day_type {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Line
            }
        }

        define_and_impl_resolution_properties!(
            define_and_impl: {
                $week_type: Week,
                $month_type: Month,
                $year_type: Year,
            },
            base_impl: $day_type
        );
    };
}

impl_all_properties_for_recurrance!(
    name_suffix: "60days",
    day: DailyProperties60Days,
    week: WeeklyProperties60Days,
    month: MonthlyProperties60Days,
    year: YearlyProperties60Days,
);

impl_all_properties_for_recurrance!(
    name_suffix: "90days",
    day: DailyProperties90Days,
    week: WeeklyProperties90Days,
    month: MonthlyProperties90Days,
    year: YearlyProperties90Days,
);

impl_all_properties_for_recurrance!(
    name_suffix: "120days",
    day: DailyProperties120Days,
    week: WeeklyProperties120Days,
    month: MonthlyProperties120Days,
    year: YearlyProperties120Days,
);

type ActiveRecurringAccounts<Recurrance, Resolution, BatchSize, Properties> =
    DirectVecLocalDbChartSource<
        MapToString<
            FilterDeducible<
                MapParseTo<ActiveRecurringAccountsRemote<Recurrance, Resolution>, i64>,
                Properties,
            >,
        >,
        BatchSize,
        Properties,
    >;

pub type ActiveRecurringAccountsDailyRecurrance60Days =
    ActiveRecurringAccounts<Recurrance60Days, NaiveDate, Batch30Days, DailyProperties60Days>;
pub type ActiveRecurringAccountsWeeklyRecurrance60Days =
    ActiveRecurringAccounts<Recurrance60Days, Week, Batch30Weeks, WeeklyProperties60Days>;
pub type ActiveRecurringAccountsMonthlyRecurrance60Days =
    ActiveRecurringAccounts<Recurrance60Days, Month, Batch36Months, MonthlyProperties60Days>;
pub type ActiveRecurringAccountsYearlyRecurrance60Days =
    ActiveRecurringAccounts<Recurrance60Days, Year, Batch30Years, YearlyProperties60Days>;

pub type ActiveRecurringAccountsDailyRecurrance90Days =
    ActiveRecurringAccounts<Recurrance90Days, NaiveDate, Batch30Days, DailyProperties90Days>;
pub type ActiveRecurringAccountsWeeklyRecurrance90Days =
    ActiveRecurringAccounts<Recurrance90Days, Week, Batch30Weeks, WeeklyProperties90Days>;
pub type ActiveRecurringAccountsMonthlyRecurrance90Days =
    ActiveRecurringAccounts<Recurrance90Days, Month, Batch36Months, MonthlyProperties90Days>;
pub type ActiveRecurringAccountsYearlyRecurrance90Days =
    ActiveRecurringAccounts<Recurrance90Days, Year, Batch30Years, YearlyProperties90Days>;

pub type ActiveRecurringAccountsDailyRecurrance120Days =
    ActiveRecurringAccounts<Recurrance120Days, NaiveDate, Batch30Days, DailyProperties120Days>;
pub type ActiveRecurringAccountsWeeklyRecurrance120Days =
    ActiveRecurringAccounts<Recurrance120Days, Week, Batch30Weeks, WeeklyProperties120Days>;
pub type ActiveRecurringAccountsMonthlyRecurrance120Days =
    ActiveRecurringAccounts<Recurrance120Days, Month, Batch36Months, MonthlyProperties120Days>;
pub type ActiveRecurringAccountsYearlyRecurrance120Days =
    ActiveRecurringAccounts<Recurrance120Days, Year, Batch30Years, YearlyProperties120Days>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::*;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_daily_60_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsDailyRecurrance60Days>(
            "update_active_recurring_accounts_daily_60_days",
            vec![
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_weekly_60_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsWeeklyRecurrance60Days>(
            "update_active_recurring_accounts_weekly_60_days",
            vec![("2022-11-28", "1"), ("2023-12-26", "1")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_monthly_60_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsMonthlyRecurrance60Days>(
            "update_active_recurring_accounts_monthly_60_days",
            vec![
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_yearly_60_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsYearlyRecurrance60Days>(
            "update_active_recurring_accounts_yearly_60_days",
            vec![("2023-01-01", "2")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_daily_90_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsDailyRecurrance90Days>(
            "update_active_recurring_accounts_daily_90_days",
            vec![
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_weekly_90_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsWeeklyRecurrance90Days>(
            "update_active_recurring_accounts_weekly_90_days",
            vec![
                ("2022-11-28", "1"),
                ("2023-12-26", "1"),
                ("2023-01-30", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_monthly_90_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsMonthlyRecurrance90Days>(
            "update_active_recurring_accounts_monthly_90_days",
            vec![
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_yearly_90_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsYearlyRecurrance90Days>(
            "update_active_recurring_accounts_yearly_90_days",
            vec![("2023-01-01", "2")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_daily_120_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsDailyRecurrance120Days>(
            "update_active_recurring_accounts_daily_120_days",
            vec![
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_weekly_120_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsWeeklyRecurrance120Days>(
            "update_active_recurring_accounts_weekly_120_days",
            vec![
                ("2022-11-28", "1"),
                ("2023-12-26", "1"),
                ("2023-01-30", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_monthly_120_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsMonthlyRecurrance120Days>(
            "update_active_recurring_accounts_monthly_120_days",
            vec![
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts_yearly_120_days() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsYearlyRecurrance120Days>(
            "update_active_recurring_accounts_yearly_120_days",
            vec![
                ("2023-01-01", "2"),
            ],
        )
        .await;
    }
}
