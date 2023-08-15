use crate::{
    charts::{
        cache::Cache,
        insert::{DateValue, DateValueInt},
        updater::ChartFullUpdater,
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};
use tokio::sync::Mutex;

pub struct NewAccounts {
    cache: Mutex<Cache<Vec<DateValueInt>>>,
}

impl NewAccounts {
    pub fn new(cache: Cache<Vec<DateValueInt>>) -> Self {
        Self {
            cache: Mutex::new(cache),
        }
    }

    pub async fn read_values(
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValueInt>, UpdateError> {
        let stmnt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
                SELECT 
                    first_tx.date as date,
                    count(*) as value
                FROM (
                    SELECT DISTINCT ON (t.from_address_hash)
                        b.timestamp::date as date
                    FROM transactions  t
                    JOIN blocks        b ON t.block_hash = b.hash
                    WHERE 
                        b.timestamp != to_timestamp(0) AND 
                        b.consensus = true
                    ORDER BY t.from_address_hash, b.timestamp
                ) first_tx
                GROUP BY first_tx.date;
                "#,
            vec![],
        );

        let mut data = DateValueInt::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        data.sort_by_key(|v| v.date);
        Ok(data)
    }
}

#[async_trait]
impl ChartFullUpdater for NewAccounts {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let mut cache = self.cache.lock().await;
        Ok(cache
            .get_or_update(async move { Self::read_values(blockscout).await })
            .await?
            .into_iter()
            .map(DateValue::from)
            .collect())
    }
}

#[async_trait]
impl crate::Chart for NewAccounts {
    fn name(&self) -> &str {
        "newAccounts"
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
    async fn update_new_accounts() {
        let chart = NewAccounts::new(Cache::default());
        simple_test_chart(
            "update_new_accounts",
            chart,
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "3"),
                ("2022-11-11", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }
}
