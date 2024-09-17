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
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
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

gettable_const!(Recurrance30Days: Duration = Duration::days(30));

pub type ActiveRecurringAccountsRemote<Recurrance, Resolution> = RemoteDatabaseSource<
    PullEachWith<
        ActiveRecurringAccountsStatement<Recurrance>,
        Resolution,
        String,
        QueryAllBlockTimestampRange,
    >,
>;

pub struct DailyProperties;

impl Named for DailyProperties {
    fn name() -> String {
        "activeRecurringAccounts".into()
    }
}

impl ChartProperties for DailyProperties {
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
    base_impl: DailyProperties
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

pub type ActiveRecurringAccountsDailyRecurrance30Days =
    ActiveRecurringAccounts<Recurrance30Days, NaiveDate, Batch30Days, DailyProperties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::ActiveRecurringAccountsDailyRecurrance30Days;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccountsDailyRecurrance30Days>(
            "update_active_recurring_accounts",
            vec![("2022-11-12", "1"), ("2022-12-01", "1")],
        )
        .await;
    }
}
