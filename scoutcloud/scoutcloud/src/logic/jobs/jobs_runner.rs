use crate::logic::{
    jobs::{balance::CheckBalanceTask, StartingTask, StoppingTask},
    DeployError, GithubClient,
};
use anyhow::Context;
use fang::{
    asynk::async_queue::AsyncQueue, AsyncQueueable, AsyncWorkerPool, FangError, NoTls, SleepParams,
};
use sea_orm::DatabaseConnection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct JobsRunner {
    queue: Mutex<AsyncQueue<NoTls>>,
}

impl JobsRunner {
    pub async fn start(
        scoutcloud_db: Arc<DatabaseConnection>,
        github: Arc<GithubClient>,
        fang_db_url: &str,
    ) -> Result<Self, anyhow::Error> {
        // it's important to init global values before starting the runner
        // because runner will use global variables since fang doesn't support context
        super::global::init_db_connection(scoutcloud_db);
        super::global::init_github_client(github);
        let runner = Self::start_pool(fang_db_url).await?;
        runner.schedule_tasks().await?;
        Ok(runner)
    }

    pub async fn start_pool(db_url: &str) -> Result<Self, anyhow::Error> {
        let max_pool_size: u32 = 10;

        let mut queue = AsyncQueue::builder()
            .uri(db_url)
            .max_pool_size(max_pool_size)
            .build();
        queue
            .connect(NoTls)
            .await
            .context("connecting to fang database")?;

        let mut pool: AsyncWorkerPool<AsyncQueue<NoTls>> = AsyncWorkerPool::builder()
            .number_of_workers(max_pool_size)
            .sleep_params(SleepParams {
                sleep_period: Duration::from_secs(1),
                max_sleep_period: Duration::from_secs(5),
                min_sleep_period: Duration::from_secs(1),
                sleep_step: Duration::from_secs(1),
            })
            .queue(queue.clone())
            .build();
        pool.start().await;

        let queue = Mutex::new(queue);

        Ok(Self { queue })
    }

    pub async fn schedule_tasks(&self) -> Result<(), anyhow::Error> {
        let mut queue = self.queue.lock().await;
        queue.schedule_task(&CheckBalanceTask::default()).await?;
        Ok(())
    }

    pub async fn insert_starting_task(&self, deployment_id: i32) -> Result<(), anyhow::Error> {
        let mut queue = self.queue.lock().await;
        queue.insert_task(&StartingTask::new(deployment_id)).await?;
        Ok(())
    }

    pub async fn insert_stopping_task(&self, deployment_id: i32) -> Result<(), anyhow::Error> {
        let mut queue = self.queue.lock().await;
        queue.insert_task(&StoppingTask::new(deployment_id)).await?;
        Ok(())
    }
}

impl From<DeployError> for FangError {
    fn from(value: DeployError) -> Self {
        FangError {
            description: value.to_string(),
        }
    }
}
