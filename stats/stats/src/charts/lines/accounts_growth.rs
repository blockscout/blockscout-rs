use crate::{
    charts::{insert::DateValue, ChartUpdater},
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct AccountsGrowth {}

#[async_trait]
impl ChartUpdater for AccountsGrowth {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        _last_row: Option<NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
                SELECT 
                    first_tx.date as date,
                    (sum(count(*)) OVER (ORDER BY first_tx.date))::TEXT as value
                FROM (
                    SELECT DISTINCT ON (t.from_address_hash)
                        b.timestamp::date as date
                    FROM transactions  t
                    JOIN blocks        b ON t.block_hash = b.hash
                    WHERE b.consensus = true
                    ORDER BY t.from_address_hash, b.timestamp
                ) first_tx
                GROUP BY first_tx.date;
                "#,
            vec![],
        );

        let data = DateValue::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for AccountsGrowth {
    fn name(&self) -> &str {
        "accountsGrowth"
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
    use super::AccountsGrowth;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth() {
        let chart = AccountsGrowth::default();
        simple_test_chart(
            "update_accounts_growth",
            chart,
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "5"),
                ("2022-11-12", "6"),
            ],
        )
        .await;
    }
}
