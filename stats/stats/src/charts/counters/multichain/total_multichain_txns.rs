use crate::chart_prelude::*;

pub struct TotalMultichainTxnsStatement;
impl_db_choice!(TotalMultichainTxnsStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalMultichainTxnsStatement {
    fn get_statement(
        _update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        Statement::from_string(DbBackend::Postgres, "SELECT 0")
    }

    fn get_statement_with_context(
        cx: &UpdateContext<'_>,
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        let mut sql = String::from(
            r#"
            SELECT COALESCE(SUM(total_transactions_number), 0)::bigint AS value
            FROM (
                SELECT DISTINCT ON (chain_id) chain_id, total_transactions_number
                FROM counters_global_imported
                WHERE date <= $1
            "#
        );
    
        let mut params: Vec<Value> = vec![update_time.into()];
    
        if let Some(filter) = &cx.multichain_filter {
            if !filter.is_empty() {
                let placeholders: Vec<String> = (0..filter.len())
                    .map(|i| format!("${}", i + 2))
                    .collect();
                sql.push_str(&format!(" AND chain_id IN ({})", placeholders.join(", ")));
                for chain_id in filter {
                    params.push(Value::BigInt(Some(*chain_id as i64)));
                }
            }
        }
        
        sql.push_str(
            r#"
                ORDER BY chain_id, date DESC
            ) t
            "#
        );
    
        Statement::from_sql_and_values(DbBackend::Postgres, sql, params)
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
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "101",
            Some(dt("2023-02-02T00:00:00")),
            None
        )
        .await;

        simple_test_counter_multichain::<TotalMultichainTxns>(
            "update_total_multichain_txns",
            "101",
            None,   
            Some(vec![1, 2]),
        )
        .await;
    }
}
