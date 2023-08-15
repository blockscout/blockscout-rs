use crate::{
    charts::{
        insert::{DateValue, DateValueDouble},
        updater::ChartFullUpdater,
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct AverageBlockTime {}

#[async_trait]
impl ChartFullUpdater for AverageBlockTime {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let item = DateValueDouble::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT
                max(timestamp)::date as date, 
                (CASE WHEN avg(diff) IS NULL THEN 0 ELSE avg(diff) END)::float as value
            FROM
            (
                SELECT
                    timestamp,
                    EXTRACT(
                        EPOCH FROM timestamp - lag(timestamp) OVER (ORDER BY timestamp)
                    ) as diff
                FROM blocks b
                WHERE b.timestamp != to_timestamp(0) AND consensus = true
            ) t
            "#,
            vec![],
        ))
        .one(blockscout)
        .await
        .map_err(UpdateError::BlockscoutDB)?
        .map(DateValue::from)
        .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;

        Ok(vec![item])
    }
}

#[async_trait]
impl crate::Chart for AverageBlockTime {
    fn name(&self) -> &str {
        "averageBlockTime"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
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
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time() {
        let counter = AverageBlockTime::default();
        simple_test_counter("update_average_block_time", counter, "802200.0833333334").await;
    }
}
