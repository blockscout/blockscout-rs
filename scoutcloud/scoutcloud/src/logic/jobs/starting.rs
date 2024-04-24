#![allow(clippy::blocks_in_conditions)]

use super::global;
use crate::logic::{DeployError, Deployment, GithubClient, Instance};

use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};

use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::DatabaseConnection;
use std::time::Duration;

const DEFAULT_WORKFLOW_TIMEOUT: Duration = Duration::from_secs(20 * 60);
const DEFAULT_WORKFLOW_CHECK_INTERVAL: Duration = Duration::from_secs(5);

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Debug)]
#[serde(crate = "fang::serde")]
pub struct StartingTask {
    deployment_id: i32,
    workflow_timeout: Duration,
    workflow_check_interval: Duration,
}

impl StartingTask {
    pub fn new(
        deployment_id: i32,
        workflow_timeout: Duration,
        workflow_check_interval: Duration,
    ) -> Self {
        Self {
            deployment_id,
            workflow_timeout,
            workflow_check_interval,
        }
    }

    pub fn from_deployment_id(deployment_id: i32) -> Self {
        Self::new(
            deployment_id,
            DEFAULT_WORKFLOW_TIMEOUT,
            DEFAULT_WORKFLOW_CHECK_INTERVAL,
        )
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for StartingTask {
    #[tracing::instrument(err(Debug), skip(_client), level = "info")]
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

        // todo: save run_id to database and if deployment in pending state, watch for it
        let result = match &deployment.model.status {
            DeploymentStatusType::Created | DeploymentStatusType::Stopped => {
                self.github_deploy_and_wait(
                    db.as_ref(),
                    github.as_ref(),
                    &instance,
                    &mut deployment,
                )
                .await
            }
            DeploymentStatusType::Running
            | DeploymentStatusType::Pending
            | DeploymentStatusType::Stopping
            | DeploymentStatusType::Failed => {
                tracing::warn!(
                    "cannot start deployment '{}': state '{:?}' is invalid",
                    self.deployment_id,
                    deployment.model.status,
                );
                Ok(())
            }
        };

        if let Err(err) = result {
            tracing::error!("failed to start deployment: {:?}", err);
            deployment
                .mark_as_error(db.as_ref(), format!("failed to start deployment: {}", err))
                .await
                .map_err(DeployError::Db)?;
        };

        Ok(())
    }

    fn cron(&self) -> Option<Scheduled> {
        None
    }
}

impl StartingTask {
    async fn github_deploy_and_wait(
        &self,
        db: &DatabaseConnection,
        github: &GithubClient,
        instance: &Instance,
        deployment: &mut Deployment,
    ) -> Result<(), DeployError> {
        deployment
            .update_status(db, DeploymentStatusType::Pending)
            .await?;
        let run = instance.deploy_via_github(github).await?;
        github
            .wait_for_success_workflow(&run, self.workflow_timeout, self.workflow_check_interval)
            .await?;

        deployment.mark_as_running(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_utils;

    #[tokio::test]
    async fn starting_task_works() {
        let (db, _github, repo, runner) =
            tests_utils::init::jobs_runner_test_case("starting_task_works").await;
        let conn = db.client();
        let handles = repo.build_handles();

        let not_started_deployment_id = 4;
        let task = StartingTask::new(
            not_started_deployment_id,
            Duration::from_millis(100),
            Duration::from_millis(100),
        );
        runner.insert_task(&task).await.unwrap();
        // no way to block on task, so wait for task to be executed
        tokio::time::sleep(Duration::from_secs(10)).await;

        let deployment = Deployment::get(conn.as_ref(), not_started_deployment_id)
            .await
            .unwrap();
        assert_eq!(
            deployment.model.status,
            DeploymentStatusType::Running,
            "deployment is not running. error: {:?}",
            deployment.model.error
        );

        handles.assert_hits("dispatch_deploy_yaml", 1);
        handles.assert_hits("runs_deploy_yaml", 1);
        handles.assert_hits("single_run_deploy_yaml", 1);
    }
}
