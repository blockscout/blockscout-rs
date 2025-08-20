use crate::{
    ChartProperties, IndexingStatus, MissingDatePolicy, Named,
    data_source::{
        kinds::{
            data_manipulation::map::MapToString,
            local_db::DirectPointLocalDbChartSource,
            remote_db::{PullOneNowValue, RemoteDatabaseSource, StatementFromUpdateTime},
        },
        types::IndexerMigrations,
    },
    indexing_status::IndexingStatusTrait,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct TotalMultichainTxnsStatement;

impl StatementFromUpdateTime for TotalMultichainTxnsStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COALESCE(SUM(total_transactions_number), 0)::bigint AS value
            FROM (
                SELECT DISTINCT ON (chain_id) chain_id, total_transactions_number
                FROM counters_global_imported
                WHERE date <= $1
                ORDER BY chain_id, date DESC
            ) t
            "#
            .to_string(),
            vec![update_time.into()],
        )
    }
}

pub type TotalMultichainTxnsRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalMultichainTxnsStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalMultichainTxns".into()
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
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type TotalMultichainTxns =
    DirectPointLocalDbChartSource<MapToString<TotalMultichainTxnsRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_multichain_txns() {
        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "210",
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "101",
            Some(dt("2023-02-02T00:00:00")),
        )
        .await;
    }
}
