//! Accounts that are active during the period + were active during some previous time

use std::{marker::PhantomData, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            local_db::{
                parameters::update::batching::parameters::Batch30Days, DirectVecLocalDbChartSource,
            },
            remote_db::{
                PullAllWithAndSort, PullEachWith, RemoteDatabaseSource, StatementFromRange,
                StatementFromTimespan,
            },
        },
        types::{BlockscoutMigrations, Get},
    },
    gettable_const,
    types::Timespan,
    utils::{produce_filter_and_values, sql_with_range_filter_opt},
    ChartProperties, Named,
};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

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
                    SELECT COUNT(recurring_active_addresses.from_address_hash)
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
                    SELECT COUNT(recurring_active_addresses.from_address_hash)
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
                            bconsensus = true {recurring_activity_range}
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
        todo!()
    }
}

gettable_const!(ThirtyDays: Duration = Duration::days(30));

pub type ActiveRecurringAccountsRemote<Recurrance> = RemoteDatabaseSource<
    PullEachWith<
        ActiveRecurringAccountsStatement<Recurrance>,
        NaiveDate,
        String,
        QueryAllBlockTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "activeRecurringAccounts".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type ActiveRecurringAccounts =
    DirectVecLocalDbChartSource<ActiveRecurringAccountsRemote<ThirtyDays>, Batch30Days, Properties>;

#[cfg(test)]
mod tests {
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    use super::ActiveRecurringAccounts;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_recurring_accounts() {
        simple_test_chart_with_migration_variants::<ActiveRecurringAccounts>(
            "update_active_recurring_accounts",
            vec![
                ("2022-11-09", "0"),
                ("2022-11-10", "0"),
                ("2022-11-11", "0"),
                ("2022-11-12", "1"),
                ("2022-12-01", "0"),
                ("2023-01-01", "0"),
                ("2023-02-01", "0"),
                ("2023-03-01", "0"),
            ],
        )
        .await;
    }
}
