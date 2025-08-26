use crate::chart_prelude::*;
pub struct TotalMultichainAddressesStatement;
impl_db_choice!(TotalMultichainAddressesStatement, UsePrimaryDB);

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
