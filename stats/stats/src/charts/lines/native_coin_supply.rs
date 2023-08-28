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

const ETH: i64 = 1_000_000_000_000_000_000;

#[derive(Default, Debug)]
pub struct NativeCoinSupply {}

#[async_trait]
impl ChartPartialUpdater for NativeCoinSupply {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM 
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE 
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE day > $2 AND day != to_timestamp(0)
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into(), row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r"
                    SELECT date, value FROM 
                    (
                        SELECT
                            day as date,
                            (sum(
                                CASE 
                                    WHEN address_hash = '\x0000000000000000000000000000000000000000' THEN -value
                                    ELSE value
                                END
                            ) / $1)::float AS value
                        FROM address_coin_balances_daily
                        WHERE day != to_timestamp(0)
                        GROUP BY day
                    ) as intermediate
                    WHERE value is not NULL;
                ",
                vec![ETH.into()],
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
impl crate::Chart for NativeCoinSupply {
    fn name(&self) -> &str {
        "nativeCoinSupply"
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
    async fn update_native_coin_supply() {
        let chart = NativeCoinSupply::default();

        simple_test_chart(
            "update_native_coin_supply",
            chart,
            vec![
                ("2022-11-09", "6666.666666666667"),
                ("2022-11-10", "6000"),
                ("2022-11-11", "5000"),
            ],
        )
        .await;
    }
}
