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
pub struct AverageBlockRewards {}

const ETH: i64 = 1_000_000_000_000_000_000;

#[async_trait]
impl ChartPartialUpdater for AverageBlockRewards {
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
                    (AVG(block_rewards.reward) / $1)::FLOAT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    date(blocks.timestamp) > $2 AND 
                    blocks.consensus = true
                GROUP BY date
                "#,
                vec![ETH.into(), row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    (AVG(block_rewards.reward) / $1)::FLOAT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE 
                    blocks.timestamp != to_timestamp(0) AND 
                    blocks.consensus = true
                GROUP BY date
                "#,
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
impl crate::Chart for AverageBlockRewards {
    fn name(&self) -> &str {
        "averageBlockRewards"
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
    use super::AverageBlockRewards;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_rewards() {
        let chart = AverageBlockRewards::default();
        simple_test_chart(
            "update_average_block_rewards",
            chart,
            vec![
                ("2022-11-09", "0"),
                ("2022-11-10", "2"),
                ("2022-11-11", "1.75"),
                ("2022-11-12", "3"),
                ("2022-12-01", "4"),
                ("2023-01-01", "0"),
                ("2023-02-01", "1"),
                ("2023-03-01", "2"),
            ],
        )
        .await;
    }
}
