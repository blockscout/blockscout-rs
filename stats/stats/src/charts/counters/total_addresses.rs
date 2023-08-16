use crate::{
    charts::{insert::DateValue, updater::ChartFullUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct TotalAddresses {}

#[async_trait]
impl ChartFullUpdater for TotalAddresses {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let data = DateValue::find_by_statement(Statement::from_string(
            DbBackend::Postgres,
            r#"SELECT date, value FROM ( 
                SELECT (
                    SELECT COUNT(*)::TEXT as value FROM addresses
                ), (
                    SELECT MAX(b.timestamp)::DATE AS date
                    FROM blocks b
                    WHERE b.consensus = true
                )
            ) as sub"#
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
impl crate::Chart for TotalAddresses {
    fn name(&self) -> &str {
        "totalAddresses"
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
    async fn update_total_addresses() {
        let counter = TotalAddresses::default();
        simple_test_counter("update_total_addresses", counter, "33").await;
    }
}
