use crate::{
    charts::{insert::DateValue, updater::ChartPartialUpdater},
    MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct NativeCoinHoldersGrowth {}

#[async_trait]
impl ChartPartialUpdater for NativeCoinHoldersGrowth {
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
                        day as date,
                        count(*)::TEXT as value
                    FROM address_coin_balances_daily
                    WHERE value != 0 AND day > $1 AND day != to_timestamp(0)
                    GROUP BY day;
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT 
                        day as date,
                        count(*)::TEXT as value
                    FROM address_coin_balances_daily
                    WHERE value != 0 AND day != to_timestamp(0)
                    GROUP BY day;
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
impl crate::Chart for NativeCoinHoldersGrowth {
    fn name(&self) -> &str {
        "nativeCoinHoldersGrowth"
    }
    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy(&self) -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn drop_last_point(&self) -> bool {
        false
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
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth() {
        let chart = NativeCoinHoldersGrowth::default();

        simple_test_chart(
            "update_native_coin_holders_growth",
            chart,
            vec![
                ("2022-11-09", "8"),
                ("2022-11-10", "8"),
                ("2022-11-11", "7"),
            ],
        )
        .await;
    }
}
