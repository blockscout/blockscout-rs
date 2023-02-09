use crate::{
    cache::Cache,
    charts::{insert::DateValue, updater::ChartUpdater},
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct NewNativeCoinTransfers {
    cache: Mutex<Cache<Vec<DateValue>>>,
}

impl NewNativeCoinTransfers {
    pub fn new(cache: Cache<Vec<DateValue>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
    }

    pub async fn read_values(
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(b.timestamp) as date,
                    COUNT(*)::TEXT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    DATE(b.timestamp) > $1 AND
                    b.consensus = true AND
                    LENGTH(t.input) = 0 AND
                    t.value >= 0
                GROUP BY date
                "#,
                vec![row.date.into()],
            ),
            None => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT 
                    DATE(b.timestamp) as date,
                    COUNT(*)::TEXT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.consensus = true AND
                    LENGTH(t.input) = 0 AND
                    t.value >= 0
                GROUP BY date
                "#,
                vec![],
            ),
        };

        let data = DateValue::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(data)
    }
}

#[async_trait]
impl ChartUpdater for NewNativeCoinTransfers {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let mut cache = self.cache.lock().await;
        cache
            .get_or_update(async move { Self::read_values(blockscout, last_row).await })
            .await
    }
}

#[async_trait]
impl crate::Chart for NewNativeCoinTransfers {
    fn name(&self) -> &str {
        "newNativeCoinTransfers"
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
        self.update_with_values(db, blockscout, full).await
    }
}

#[cfg(test)]
mod tests {
    use super::NewNativeCoinTransfers;
    use crate::{cache::Cache, tests::simple_test::simple_test_chart};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers() {
        let chart = NewNativeCoinTransfers::new(Cache::default());
        simple_test_chart(
            "update_native_coins_transfers",
            chart,
            vec![
                ("2022-11-09", "2"),
                ("2022-11-10", "4"),
                ("2022-11-11", "4"),
                ("2022-11-12", "1"),
            ],
        )
        .await;
    }
}
