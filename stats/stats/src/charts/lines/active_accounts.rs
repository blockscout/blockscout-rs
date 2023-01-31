use crate::{
    charts::{insert::DateValue, ChartUpdater},
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct ActiveAccounts {}

#[async_trait]
impl ChartUpdater for ActiveAccounts {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(blocks.timestamp) as date, 
                    COUNT(DISTINCT from_address_hash)::TEXT as value
                FROM transactions 
                JOIN blocks on transactions.block_hash = blocks.hash
                WHERE date(blocks.timestamp) >= $1 AND blocks.consensus = true
                GROUP BY date(blocks.timestamp);
                "#,
                vec![row.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(blocks.timestamp) as date, 
                    COUNT(DISTINCT from_address_hash)::TEXT as value
                FROM transactions 
                JOIN blocks on transactions.block_hash = blocks.hash
                WHERE blocks.consensus = true
                GROUP BY date(blocks.timestamp);
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
impl crate::Chart for ActiveAccounts {
    fn name(&self) -> &str {
        "activeAccounts"
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
    use crate::tests::simple_test::simple_test_chart;

    use super::ActiveAccounts;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_active_accounts() {
        let chart = ActiveAccounts::default();
        simple_test_chart(
            "update_active_accounts",
            chart,
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "2"),
                ("2022-11-11", "2"),
                ("2022-11-12", "1"),
            ],
        )
        .await;
    }
}
