use super::global;
use crate::logic::{
    github::types::{RunStatus, RunStatusShort},
    DeployError, Deployment, GithubClient, Instance,
};

use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use octocrab::models::RunId;
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::DatabaseConnection;
use std::time::Duration;

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Debug)]
#[serde(crate = "fang::serde")]
pub struct StartingTask {
    deployment_id: i32,
}

impl StartingTask {
    pub fn new(deployment_id: i32) -> Self {
        Self { deployment_id }
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for StartingTask {
    #[tracing::instrument(skip(_client), level = "info")]
    async fn run(&self, _client: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::get_db_connection();
        let github = global::get_github_client();

        let mut deployment = Deployment::get(db.as_ref(), self.deployment_id)
            .await
            .map_err(DeployError::Db)?;
        let instance = deployment
            .get_instance(db.as_ref())
            .await
            .map_err(DeployError::Db)?;

        let allowed_statuses = [DeploymentStatusType::Created, DeploymentStatusType::Stopped];
        if !allowed_statuses.contains(&deployment.model.status) {
            tracing::warn!(
                "cannot start deployment '{}': not in created/stopped state",
                self.deployment_id
            );
            return Ok(());
        };

        if let Err(err) =
            github_deploy_and_wait(db.as_ref(), github.as_ref(), &instance, &mut deployment).await
        {
            tracing::error!("failed to start deployment: {}", err);
            deployment
                .update_error(db.as_ref(), format!("failed to start deployment: {}", err))
                .await
                .map_err(DeployError::Db)?;
        };

        Ok(())
    }

    fn cron(&self) -> Option<Scheduled> {
        None
    }
}

async fn github_deploy_and_wait(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance: &Instance,
    deployment: &mut Deployment,
) -> Result<(), DeployError> {
    let postgres_run = instance.deploy_postgres(github).await?;
    deployment
        .update_status(db, DeploymentStatusType::Pending)
        .await?;
    let max_sleep = Duration::from_secs(150);
    let sleep_between = Duration::from_secs(5);
    let status =
        wait_for_github_workflow(github, postgres_run.id, max_sleep, sleep_between).await?;
    match status.short() {
        RunStatusShort::Failure => {
            return Err(DeployError::GithubWorkflow(anyhow::anyhow!(
                "failed to start postgres. status={status:?}"
            )))
        }
        RunStatusShort::Pending => {
            return Err(DeployError::GithubWorkflow(anyhow::anyhow!(
                "timed out waiting for postgres deploy"
            )))
        }
        RunStatusShort::Completed => {
            tracing::info!("postgres deploy completed");
        }
    }

    let microservices_run = instance.deploy_microservices(github).await?;
    let status =
        wait_for_github_workflow(github, microservices_run.id, max_sleep, sleep_between).await?;
    match status.short() {
        RunStatusShort::Failure => {
            return Err(DeployError::GithubWorkflow(anyhow::anyhow!(
                "failed to start microservices. status={status:?}"
            )))
        }
        RunStatusShort::Pending => {
            return Err(DeployError::GithubWorkflow(anyhow::anyhow!(
                "timed out waiting for microservices deploy"
            )))
        }
        RunStatusShort::Completed => {
            tracing::info!("microservices deploy completed");
        }
    }
    deployment
        .update_status(db, DeploymentStatusType::Running)
        .await?;
    Ok(())
}

async fn wait_for_github_workflow(
    github: &GithubClient,
    run_id: RunId,
    timeout: Duration,
    sleep_between: Duration,
) -> Result<RunStatus, DeployError> {
    tracing::info!("waiting for github workflow run {}", run_id);

    let run = github.get_workflow_run(run_id).await?;
    let status = RunStatus::try_from_str(&run.status)?;
    match status.short() {
        RunStatusShort::Completed | RunStatusShort::Failure => return Ok(status),
        RunStatusShort::Pending => {}
    }
    tokio::time::timeout(timeout, async move {
        loop {
            tokio::time::sleep(sleep_between).await;
            let run = github.get_workflow_run(run_id).await?;
            let status = RunStatus::try_from_str(&run.status)?;
            match status.short() {
                RunStatusShort::Completed | RunStatusShort::Failure => return Ok(status),
                RunStatusShort::Pending => {}
            }
        }
    })
    .await
    .unwrap_or(Ok(status))
}
