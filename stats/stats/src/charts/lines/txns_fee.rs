use crate::{
    charts::{
        insert::{DateValue, DateValueDouble},
        ChartUpdater,
    },
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct TxnsFee {}

const ETHER: i64 = i64::pow(10, 18);

#[async_trait]
impl ChartUpdater for TxnsFee {
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
                    DATE(b.timestamp) as date, 
                    (SUM(t.gas_used * t.gas_price) / $1)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    DATE(b.timestamp) >= $2 AND
                    b.consensus = true
                GROUP BY DATE(b.timestamp)
                "#,
                vec![ETHER.into(), row.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(b.timestamp) as date, 
                    (SUM(t.gas_used * t.gas_price) / $1)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE b.consensus = true
                GROUP BY DATE(b.timestamp)
                "#,
                vec![ETHER.into()],
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
impl crate::Chart for TxnsFee {
    fn name(&self) -> &str {
        "txnsFee"
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
    use super::TxnsFee;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_fee() {
        let chart = TxnsFee::default();
        simple_test_chart(
            "update_txns_fee",
            chart,
            vec![
                ("2022-11-09", "0"),
                ("2022-11-10", "0.00023592592569"),
                ("2022-11-11", "0.000519037036518"),
                ("2022-11-12", "0.000023592592569"),
            ],
        )
        .await;
    }
}
