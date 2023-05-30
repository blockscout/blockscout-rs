use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct CompletedTxns {}

#[async_trait]
impl ChartFullUpdater for CompletedTxns {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        // here we split query into 3 parts due to perfomance.
        //
        // joining transactions and blocks with filtering on (t.status = 1 and b.consensus = true) is super long.
        // so we count amount of success transactions without joinging,
        // and then subtract amount of dropped transactions.
        // since amount of dropped txns (b.consensus = false) is very small,
        // the second query will execute very quickly.
        // also we need last date of block, that's why 3rd query is needed
        let data = DateValue::find_by_statement(Statement::from_string(
            DbBackend::Postgres,
            r#"SELECT (all_success - all_success_dropped)::TEXT AS value, last_block_date AS date 
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
            ) AS sub"#
                .into(),
        ))
        .one(blockscout)
        .await
        .map_err(UpdateError::BlockscoutDB)?
        .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;

        Ok(vec![data])
    }
}

#[async_trait]
impl crate::Chart for CompletedTxns {
    fn name(&self) -> &str {
        "completedTxns"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_completed_txns() {
        let counter = CompletedTxns::default();
        simple_test_counter("update_completed_txns", counter, "46").await;
    }
}
