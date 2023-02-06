use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct TotalTokens {}

#[async_trait]
impl ChartFullUpdater for TotalTokens {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let data = DateValue::find_by_statement(Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT 
                (
                    SELECT count(*)::text
                        FROM tokens
                ) AS "value",
                (
                    SELECT max(timestamp)::date as "date" 
                        FROM blocks
                        WHERE blocks.consensus = true
                ) AS "date"
            "#
            .into(),
        ))
        .one(blockscout)
        .await
        .map_err(UpdateError::BlockscoutDB)?
        .ok_or_else(|| UpdateError::Internal("query returned nothing".into()))?;

        Ok(vec![data])
    }
}

#[async_trait]
impl crate::Chart for TotalTokens {
    fn name(&self) -> &str {
        "totalTokens"
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
    async fn update_total_tokens() {
        let counter = TotalTokens::default();
        simple_test_counter("update_total_tokens", counter, "4").await;
    }
}
