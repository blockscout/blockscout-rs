use crate::{
    charts::{
        cache::Cache,
        db_interaction::{
            chart_updaters::{ChartFullUpdater, ChartUpdater},
            types::{DateValue, DateValueInt},
        },
    },
    lines::{AccountsGrowth, NewAccounts},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use tokio::sync::Mutex;

pub struct TotalAccounts {
    cache: Mutex<Cache<Vec<DateValueInt>>>,
}

impl TotalAccounts {
    pub fn new(cache: Cache<Vec<DateValueInt>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
    }
}

#[async_trait]
impl ChartFullUpdater for TotalAccounts {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let mut cache = self.cache.lock().await;
        let data = cache
            .get_or_update(async move { NewAccounts::read_values(blockscout).await })
            .await?;
        let data = AccountsGrowth::sum_new(data)
            .into_iter()
            .max()
            .into_iter()
            .collect();
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for TotalAccounts {
    fn name(&self) -> &str {
        "totalAccounts"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Counter
    }
}

#[async_trait]
impl ChartUpdater for TotalAccounts {
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<chrono::Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, current_time, force_full)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_accounts() {
        let counter = TotalAccounts::new(Cache::default());
        simple_test_counter("update_total_accounts", counter, "9").await;
    }
}
