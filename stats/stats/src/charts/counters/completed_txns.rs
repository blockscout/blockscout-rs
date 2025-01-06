use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
        },
        types::BlockscoutMigrations,
    },
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct CompletedTxnsStatement;

impl StatementForOne for CompletedTxnsStatement {
    fn get_statement(completed_migrations: &BlockscoutMigrations) -> Statement {
        if completed_migrations.denormalization {
            Statement::from_string(
                DbBackend::Postgres,
                r#"
                    SELECT
                        (all_success - all_success_dropped)::TEXT AS value,
                        last_block_date AS date 
                    FROM (
                        SELECT (
                            SELECT COUNT(*) AS all_success
                            FROM transactions t
                            WHERE t.status = 1
                        ), (
                            SELECT COUNT(*) as all_success_dropped
                            FROM transactions t
                            WHERE t.status = 1 AND t.block_consensus = false
                        ), (
                            SELECT MAX(b.timestamp)::DATE AS last_block_date
                            FROM blocks b
                            WHERE b.consensus = true
                        )
                    ) AS sub
                "#,
            )
        } else {
            Statement::from_string(
                DbBackend::Postgres,
                r#"
                    SELECT
                        (all_success - all_success_dropped)::TEXT AS value,
                        last_block_date AS date 
                    FROM (
                        SELECT (
                            SELECT COUNT(*) AS all_success
                            FROM transactions t
                            WHERE t.status = 1
                        ), (
                            SELECT COUNT(*) as all_success_dropped
                            FROM transactions t
                            JOIN blocks b ON t.block_hash = b.hash
                            WHERE t.status = 1 AND b.consensus = false
                        ), (
                            SELECT MAX(b.timestamp)::DATE AS last_block_date
                            FROM blocks b
                            WHERE b.consensus = true
                        )
                    ) AS sub
                "#,
            )
        }
    }
}

pub type CompletedTxnsRemote =
    RemoteDatabaseSource<PullOne<CompletedTxnsStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "completedTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type CompletedTxns = DirectPointLocalDbChartSource<CompletedTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_with_migration_variants;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_completed_txns() {
        simple_test_counter_with_migration_variants::<CompletedTxns>(
            "update_completed_txns",
            "46",
            None,
        )
        .await;
    }
}
