use crate::{
    charts::{
        insert::{DateValue, DateValueDouble},
        updater::ChartPartialUpdater,
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct TxnsSuccessRate {}

#[async_trait]
impl ChartPartialUpdater for TxnsSuccessRate {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(b.timestamp) as date, 
                    COUNT(CASE WHEN t.error IS NULL THEN 1 END)::FLOAT
                        / COUNT(*)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE 
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    t.block_hash IS NOT NULL AND 
                    (t.error IS NULL OR t.error::text != 'dropped/replaced') AND
                    DATE(b.timestamp) > $1
                GROUP BY DATE(b.timestamp)
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(b.timestamp) as date, 
                    COUNT(CASE WHEN t.error IS NULL THEN 1 END)::FLOAT
                        / COUNT(*)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE 
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    t.block_hash IS NOT NULL AND
                    (t.error IS NULL OR t.error::text != 'dropped/replaced')
                GROUP BY DATE(b.timestamp)
                "#,
                vec![],
            ),
        };

        let data = DateValueDouble::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?
            .into_iter()
            .map(DateValue::from)
            .collect::<Vec<_>>();
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for TxnsSuccessRate {
    fn name(&self) -> &str {
        "txnsSuccessRate"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Line
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
    use super::TxnsSuccessRate;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate() {
        let chart = TxnsSuccessRate::default();
        simple_test_chart(
            "update_txns_success_rate",
            chart,
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "1"),
                ("2022-11-11", "1"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
