use sea_orm::prelude::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;

pub async fn wait_until_some_with_timeout<F, Fut, T>(
    database: Arc<DatabaseConnection>,
    timeout: Duration,
    sleep_between: Duration,
    mut closure: F,
) -> Result<T, anyhow::Error>
where
    F: FnMut(Arc<DatabaseConnection>) -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start_time = Instant::now();
    while start_time.elapsed() < timeout {
        time::sleep(sleep_between).await;
        if let Some(data) = closure(database.clone()).await {
            return Ok(data);
        }
    }
    Err(anyhow::anyhow!("task didn't finish in time"))
}

pub async fn wait_for_empty_fang_tasks(db: Arc<DatabaseConnection>) -> Result<(), anyhow::Error> {
    wait_until_some_with_timeout(
        db,
        Duration::from_secs(20),
        Duration::from_millis(500),
        |conn| async move {
            let tasks = scoutcloud_entity::fang_tasks::Entity::find()
                .count(conn.as_ref())
                .await
                .unwrap();
            if tasks == 0 {
                Some(())
            } else {
                None
            }
        },
    )
    .await
}
