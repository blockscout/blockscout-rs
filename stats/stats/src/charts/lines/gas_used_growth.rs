use crate::{
    charts::{
        insert::{DateValue, DateValueDecimal},
        updater::ChartPartialUpdater,
    },
    MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct GasUsedGrowth {}

#[async_trait]
impl ChartPartialUpdater for GasUsedGrowth {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let data = match last_row {
            Some(row) => {
                let last_value = Decimal::from_str_exact(&row.value).map_err(|e| {
                    UpdateError::Internal(format!("failed to parse previous value: {e}"))
                })?;
                let stmnt = Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT 
                        DATE(blocks.timestamp) as date, 
                        (sum(sum(blocks.gas_used)) OVER (ORDER BY date(blocks.timestamp))) AS value
                    FROM blocks
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND 
                        DATE(blocks.timestamp) > $1 AND 
                        blocks.consensus = true
                    GROUP BY date(blocks.timestamp)
                    ORDER BY date;
                    "#,
                    vec![row.date.into()],
                );
                DateValueDecimal::find_by_statement(stmnt)
                    .all(blockscout)
                    .await
                    .map_err(UpdateError::BlockscoutDB)?
                    .into_iter()
                    .map(|mut point| {
                        point.value += last_value;
                        point
                    })
                    .map(|point| point.into())
                    .collect()
            }
            None => {
                let stmnt = Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT 
                        DATE(blocks.timestamp) as date, 
                        (sum(sum(blocks.gas_used)) OVER (ORDER BY date(blocks.timestamp)))::TEXT AS value
                    FROM blocks
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND 
                        blocks.consensus = true
                    GROUP BY date(blocks.timestamp)
                    ORDER BY date;
                    "#,
                    vec![],
                );
                DateValue::find_by_statement(stmnt)
                    .all(blockscout)
                    .await
                    .map_err(UpdateError::BlockscoutDB)?
            }
        };

        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for GasUsedGrowth {
    fn name(&self) -> &str {
        "gasUsedGrowth"
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
    use super::GasUsedGrowth;
    use crate::tests::simple_test::{ranged_test_chart, simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_gas_used_growth() {
        let chart = GasUsedGrowth::default();
        simple_test_chart(
            "update_gas_used_growth",
            chart,
            vec![
                ("2022-11-09", "10000"),
                ("2022-11-10", "91780"),
                ("2022-11-11", "221640"),
                ("2022-11-12", "250680"),
                ("2022-12-01", "288350"),
                ("2023-01-01", "334650"),
                ("2023-02-01", "389580"),
                ("2023-03-01", "403140"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_gas_used_growth() {
        let chart = GasUsedGrowth::default();
        let value_2022_11_12 = "250680";
        ranged_test_chart(
            "ranged_update_gas_used_growth",
            chart,
            vec![
                ("2022-11-20", value_2022_11_12),
                ("2022-11-21", value_2022_11_12),
                ("2022-11-22", value_2022_11_12),
                ("2022-11-23", value_2022_11_12),
                ("2022-11-24", value_2022_11_12),
                ("2022-11-25", value_2022_11_12),
                ("2022-11-26", value_2022_11_12),
                ("2022-11-27", value_2022_11_12),
                ("2022-11-28", value_2022_11_12),
                ("2022-11-29", value_2022_11_12),
                ("2022-11-30", value_2022_11_12),
                ("2022-12-01", "288350"),
            ],
            "2022-11-20".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
        )
        .await;
    }
}
