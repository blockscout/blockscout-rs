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
pub struct AverageGasPrice {}

const GWEI: i64 = 1_000_000_000;

#[async_trait]
impl ChartUpdater for AverageGasPrice {
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
                        blocks.timestamp::date as date,
                        (AVG(gas_price) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_number = blocks.number
                    WHERE date(blocks.timestamp) >= $2 AND blocks.consensus = true
                    GROUP BY date
                    "#,
                vec![GWEI.into(), row.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        (AVG(gas_price) / $1)::float as value
                    FROM transactions
                    JOIN blocks ON transactions.block_number = blocks.number
                    WHERE blocks.consensus = true
                    GROUP BY date
                    "#,
                vec![GWEI.into()],
            ),
        };

        let data = DateValueDouble::find_by_statement(stmnt)
            .all(blockscout)
            .await?;
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
        full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, full).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        charts::Chart,
        get_chart_data,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Point,
    };
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_price() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_average_gas_price", None).await;
        let updater = AverageGasPrice::default();

        updater.create(&db).await.unwrap();
        fill_mock_blockscout_data(&blockscout, "2022-11-11").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = vec![
            Point {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "0".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "2.8086419725".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "6.1790123395".into(),
            },
        ];
        assert_eq!(expected, data);
    }
}
