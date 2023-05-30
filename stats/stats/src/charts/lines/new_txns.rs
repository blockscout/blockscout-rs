use crate::{
    charts::{insert::DateValue, updater::ChartPartialUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct NewTxns {}

#[async_trait]
impl ChartPartialUpdater for NewTxns {
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
                    date(b.timestamp) as date, 
                    COUNT(*)::TEXT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE 
                    date(b.timestamp) > $1 AND 
                    b.consensus = true
                GROUP BY date;
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    date(b.timestamp) as date, 
                    COUNT(*)::TEXT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE b.consensus = true
                GROUP BY date;
                "#,
                vec![],
            ),
        };

        let data = DateValue::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for NewTxns {
    fn name(&self) -> &str {
        "newTxns"
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
    use super::NewTxns;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns() {
        let chart = NewTxns::default();
        simple_test_chart(
            "update_new_txns",
            chart,
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }
}
