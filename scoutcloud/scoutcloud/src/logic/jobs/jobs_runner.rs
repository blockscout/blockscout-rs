use crate::logic::{
    jobs::{balance::CheckBalanceTask, StartingTask, StoppingTask},
    DeployError, GithubClient,
};
use anyhow::Context;
use fang::{
    asynk::async_queue::AsyncQueue, AsyncQueueable, AsyncRunnable, AsyncWorkerPool, FangError,
    NoTls, SleepParams,
};
use sea_orm::DatabaseConnection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct JobsRunner {
    queue: Mutex<AsyncQueue<NoTls>>,
}

impl JobsRunner {
    pub async fn default_start(
        scoutcloud_db: Arc<DatabaseConnection>,
        github: Arc<GithubClient>,
        fang_db_url: &str,
    ) -> Result<Self, anyhow::Error> {
        // it's important to init global values before starting the runner
        // because runner will use global variables since fang doesn't support context
        super::global::init_db_connection(scoutcloud_db).expect("database already initialized");
        super::global::init_github_client(github).expect("github client already initialized");

        let sleep_params = SleepParams {
            sleep_period: Duration::from_secs(1),
            max_sleep_period: Duration::from_secs(5),
            min_sleep_period: Duration::from_secs(1),
            sleep_step: Duration::from_secs(1),
        };
        let runner = Self::start_pool(fang_db_url, sleep_params).await?;
        runner.schedule_tasks().await?;
        Ok(runner)
    }

    pub async fn start_pool(
        db_url: &str,
        sleep_params: SleepParams,
    ) -> Result<Self, anyhow::Error> {
        tracing::info!("start jobs runners pool");

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
            .sleep_params(sleep_params)
            .retention_mode(fang::RetentionMode::RemoveFinished)
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
        self.insert_task(&StartingTask::from_deployment_id(deployment_id))
            .await
    }

    pub async fn insert_stopping_task(&self, deployment_id: i32) -> Result<(), anyhow::Error> {
        self.insert_task(&StoppingTask::from_deployment_id(deployment_id))
            .await
    }

    pub async fn insert_task(&self, task: &dyn AsyncRunnable) -> Result<(), anyhow::Error> {
        let mut queue = self.queue.lock().await;
        queue.insert_task(task).await?;
        Ok(())
    }

    pub fn queue(&self) -> &Mutex<AsyncQueue<NoTls>> {
        &self.queue
    }
}

impl From<DeployError> for FangError {
    fn from(value: DeployError) -> Self {
        FangError {
            description: value.to_string(),
        }
    }
}
