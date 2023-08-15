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
pub struct AverageGasPrice {}

const GWEI: i64 = 1_000_000_000;

#[async_trait]
impl ChartPartialUpdater for AverageGasPrice {
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
                        blocks.timestamp::date as date,
                        (AVG(gas_price) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_hash = blocks.hash
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND
                        date(blocks.timestamp) > $2 AND
                        blocks.consensus = true
                    GROUP BY date
                    "#,
                vec![GWEI.into(), row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        (AVG(gas_price) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_hash = blocks.hash
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND
                        blocks.consensus = true
                    GROUP BY date
                    "#,
                vec![GWEI.into()],
            ),
        };

        let data = DateValueDouble::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let data = data.into_iter().map(DateValue::from).collect();
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for AverageGasPrice {
    fn name(&self) -> &str {
        "averageGasPrice"
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
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price() {
        let chart = AverageGasPrice::default();

        simple_test_chart(
            "update_average_gas_price",
            chart,
            vec![
                ("2022-11-09", "0.4493827156"),
                ("2022-11-10", "1.96604938075"),
                ("2022-11-11", "3.2901234535"),
                ("2022-11-12", "5.8419753028"),
                ("2022-12-01", "6.5160493762"),
                ("2023-01-01", "1.123456789"),
                ("2023-02-01", "9.5493827065"),
                ("2023-03-01", "1.123456789"),
            ],
        )
        .await;
    }
}
