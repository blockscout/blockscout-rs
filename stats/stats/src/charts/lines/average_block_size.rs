use crate::{
    charts::{insert::DateValue, updater::ChartPartialUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct AverageBlockSize {}

#[async_trait]
impl ChartPartialUpdater for AverageBlockSize {
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
                    ROUND(AVG(blocks.size))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    DATE(blocks.timestamp) > $1 AND 
                    consensus = true
                GROUP BY date
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.size))::TEXT as value
                FROM blocks
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    consensus = true
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
impl crate::Chart for AverageBlockSize {
    fn name(&self) -> &str {
        "averageBlockSize"
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
    use super::AverageBlockSize;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_size() {
        let chart = AverageBlockSize::default();
        simple_test_chart(
            "update_average_block_size",
            chart,
            vec![
                ("2022-11-09", "1000"),
                ("2022-11-10", "2726"),
                ("2022-11-11", "3247"),
                ("2022-11-12", "2904"),
                ("2022-12-01", "3767"),
                ("2023-01-01", "4630"),
                ("2023-02-01", "5493"),
                ("2023-03-01", "1356"),
            ],
        )
        .await;
    }
}
