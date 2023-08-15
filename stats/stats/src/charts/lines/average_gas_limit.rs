use crate::{
    charts::{insert::DateValue, updater::ChartPartialUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct AverageGasLimit {}

#[async_trait]
impl ChartPartialUpdater for AverageGasLimit {
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
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.gas_limit))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    DATE(blocks.timestamp) > $1 AND
                    blocks.consensus = true
                GROUP BY date
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.gas_limit))::TEXT as value
                FROM blocks 
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true
                GROUP BY date
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
impl crate::Chart for AverageGasLimit {
    fn name(&self) -> &str {
        "averageGasLimit"
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
    use super::AverageGasLimit;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit() {
        let chart = AverageGasLimit::default();
        simple_test_chart(
            "update_average_gas_limit",
            chart,
            vec![
                ("2022-11-09", "12500000"),
                ("2022-11-10", "12500000"),
                ("2022-11-11", "30000000"),
                ("2022-11-12", "30000000"),
                ("2022-12-01", "30000000"),
                ("2023-01-01", "30000000"),
                ("2023-02-01", "30000000"),
                ("2023-03-01", "30000000"),
            ],
        )
        .await;
    }
}
