use crate::{
    charts::insert::{insert_data, DateValue},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct AverageBlockTime {}

impl AverageBlockTime {
    async fn get_current_value(&self, blockscout: &DatabaseConnection) -> Result<DateValue, DbErr> {
        let item = DateValue::find_by_statement(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT 
                max(timestamp)::date as date, 
                TRIM_SCALE((CASE WHEN avg(diff) IS NULL THEN 0 ELSE avg(diff) END))::TEXT as value
            FROM
            (
                SELECT
                    timestamp,
                    EXTRACT(
                        EPOCH FROM timestamp - lag(timestamp) OVER (ORDER BY timestamp)
                    ) as diff
                FROM "blocks"
            ) t
            "#,
            vec![],
        ))
        .one(blockscout)
        .await?
        .ok_or_else(|| DbErr::Custom("internal error: query returned nothing".into()))?;

        Ok(item)
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
        _full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let item = self.get_current_value(blockscout).await?;
        insert_data(db, chart_id, item).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        get_counters,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart,
    };
    use pretty_assertions::assert_eq;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_time() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_average_block_time", None).await;
        let updater = AverageBlockTime::default();

        updater.create(&db).await.unwrap();
        fill_mock_blockscout_data(&blockscout, "2022-11-11").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_counters(&db).await.unwrap();
        assert_eq!("24685.714285714286", data[updater.name()]);
    }
}
