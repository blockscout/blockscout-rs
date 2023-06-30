use super::NewAccounts;
use crate::{
    charts::{
        cache::Cache,
        insert::{DateValue, DateValueInt},
        updater::ChartFullUpdater,
    },
    MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::prelude::*;
use tokio::sync::Mutex;

pub struct AccountsGrowth {
    cache: Mutex<Cache<Vec<DateValueInt>>>,
}

impl AccountsGrowth {
    pub fn new(cache: Cache<Vec<DateValueInt>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
    }

    pub fn sum_new<I: IntoIterator<Item = DateValueInt>>(
        values: I,
    ) -> impl IntoIterator<Item = DateValue> {
        values
            .into_iter()
            .scan(0i64, |acc, mut value| {
                *acc += value.value;
                value.value = *acc;
                Some(value)
            })
            .map(DateValue::from)
    }
}

#[async_trait]
impl ChartFullUpdater for AccountsGrowth {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let mut cache = self.cache.lock().await;
        let data = cache
            .get_or_update(async move { NewAccounts::read_values(blockscout).await })
            .await?;
        Ok(Self::sum_new(data).into_iter().collect())
    }
}

#[async_trait]
impl crate::Chart for AccountsGrowth {
    fn name(&self) -> &str {
        "accountsGrowth"
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
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_accounts_growth() {
        let chart = AccountsGrowth::new(Cache::default());
        simple_test_chart(
            "update_accounts_growth",
            chart,
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "4"),
                ("2022-11-11", "8"),
                ("2023-03-01", "9"),
            ],
        )
        .await;
    }
}
