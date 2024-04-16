use std::sync::Arc;
use std::time::Duration;
use anyhow::Context;
use fang::asynk::async_queue::AsyncQueue;
use fang::{AsyncQueueable, AsyncRunnable, AsyncWorkerPool, NoTls, SleepParams};
use sea_orm::DatabaseConnection;
use crate::logic::GithubClient;


pub struct FangRunner {
    queue: AsyncQueue<NoTls>
}


impl FangRunner {
    pub async fn start(
        scoutcloud_db: Arc<DatabaseConnection>,
        github: Arc<GithubClient>,
        fang_db_url: &str
    ) -> Result<Self, anyhow::Error> {
        // it's important to init global values before starting the runner
        // because runner will use global variables since fang doesn't support context
        super::global::init_db_connection(scoutcloud_db);
        super::global::init_github_client(github);
        let mut runner = Self::start_pool(fang_db_url).await?;
        runner.schedule_tasks().await?;
        Ok(runner)
    }

    pub async fn start_pool(db_url: &str) -> Result<Self, anyhow::Error> {
        let max_pool_size: u32 = 10;

        let mut queue = AsyncQueue::builder()
            .uri(db_url)
            .max_pool_size(max_pool_size)
            .build();
        queue.connect(NoTls).await.context("connecting to fang database")?;

        let mut pool: AsyncWorkerPool<AsyncQueue<NoTls>> = AsyncWorkerPool::builder()
            .number_of_workers(max_pool_size)
            .sleep_params(SleepParams {
                sleep_period: Duration::from_secs(1),
                max_sleep_period: Duration::from_secs(5),
                min_sleep_period: Duration::from_secs(1),
                sleep_step: Duration::from_secs(1),
            }
            )
            .queue(queue.clone())
            .build();
        pool.start().await;

        Ok(Self {
            queue
        })
    }

    pub async fn schedule_tasks(&mut self) -> Result<(), anyhow::Error>{
        self.queue.schedule_task(&super::balance::CheckBalanceTask::default()).await?;
        self.queue.insert_task(&super::starting::StartingTask::new("test".to_string())).await?;
        Ok(())
    }
}