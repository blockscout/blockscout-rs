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

pub struct TotalMultichainAddressesStatement;

impl StatementFromUpdateTime for TotalMultichainAddressesStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT COALESCE(SUM(total_addresses_number), 0)::bigint AS value
            FROM (
                SELECT DISTINCT ON (chain_id) chain_id, total_addresses_number
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

pub type TotalMultichainAddressesRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalMultichainAddressesStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalMultichainAddresses".into()
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

pub type TotalMultichainAddresses =
    DirectPointLocalDbChartSource<MapToString<TotalMultichainAddressesRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_multichain_addresses() {
        simple_test_counter_multichain::<TotalMultichainAddresses>(
            "update_total_multichain_addresses",
            "920",
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainAddresses>(
            "update_total_multichain_addresses",
            "825",
            Some(dt("2023-02-02T00:00:00")),
        )
        .await;
    }
}
