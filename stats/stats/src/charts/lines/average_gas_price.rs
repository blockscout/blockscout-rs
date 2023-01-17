#![allow(unused_variables)]
use super::utils::OnlyDate;
use crate::{
    charts::insert::{insert_data_many, DateValue},
    UpdateError,
};
use async_trait::async_trait;
use entity::{chart_data, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, DbBackend, FromQueryResult, QueryOrder, QuerySelect, Statement};

#[derive(Default, Debug)]
pub struct AverageGasPrice {}

const GWEI: i64 = 1_000_000_000;

impl AverageGasPrice {
    async fn get_current_value(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        last_row: Option<OnlyDate>,
    ) -> Result<Vec<DateValue>, DbErr> {
        let stmnt = match last_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        TRIM_SCALE((AVG(gas_price) / $1))::TEXT as value
                    FROM transactions
                    JOIN blocks on transactions.block_number = blocks.number
                    WHERE date(blocks.timestamp) >= $2
                    GROUP BY date
                    "#,
                vec![GWEI.into(), row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT
                        blocks.timestamp::date as date,
                        TRIM_SCALE((AVG(gas_price) / $1))::TEXT as value
                    FROM transactions
                    JOIN blocks on transactions.block_number = blocks.number
                    GROUP BY date
                    "#,
                vec![GWEI.into()],
            ),
        };

        let data = DateValue::find_by_statement(stmnt).all(blockscout).await?;

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
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let last_row = if full {
            None
        } else {
            chart_data::Entity::find()
                .column(chart_data::Column::Date)
                .filter(chart_data::Column::ChartId.eq(id))
                .order_by_desc(chart_data::Column::Date)
                .into_model::<OnlyDate>()
                .one(db)
                .await?
        };

        let data = self
            .get_current_value(db, blockscout, last_row)
            .await?
            .into_iter()
            .map(|item| item.active_model(id));
        insert_data_many(db, data).await?;
        Ok(())
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
                value: "71.123456789".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "71.123456789".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "71.123456789".into(),
            },
        ];
        assert_eq!(expected, data);
    }
}
