use crate::logic::{jobs::global, DeployError, Deployment, GithubClient, Instance};
use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::DatabaseConnection;
use std::time::Duration;

const DEFAULT_WORKFLOW_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DEFAULT_WORKFLOW_CHECK_INTERVAL: Duration = Duration::from_secs(5);

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Debug)]
#[serde(crate = "fang::serde")]
pub struct StoppingTask {
    deployment_id: i32,
    workflow_timeout: Duration,
    workflow_check_interval: Duration,
}

impl StoppingTask {
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
impl AsyncRunnable for StoppingTask {
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

        // todo: save run_id to database and if deployment in stopping state, watch for it
        let result = match deployment.model.status {
            DeploymentStatusType::Running => {
                self.github_stop_and_wait(db.as_ref(), github.as_ref(), &instance, &mut deployment)
                    .await
            }
            DeploymentStatusType::Created
            | DeploymentStatusType::Failed
            | DeploymentStatusType::Pending
            | DeploymentStatusType::Stopped
            | DeploymentStatusType::Stopping => {
                tracing::warn!(
                    "cannot stop deployment '{}': invalid state '{:?}'",
                    self.deployment_id,
                    deployment.model.status,
                );
                return Ok(());
            }
        };

        if let Err(err) = result {
            tracing::error!("failed to stop deployment: {:?}", err);
            deployment
                .mark_as_error(db.as_ref(), format!("failed to stop deployment: {}", err))
                .await
                .map_err(DeployError::Db)?;
        };

        Ok(())
    }

    fn cron(&self) -> Option<Scheduled> {
        None
    }
}

impl StoppingTask {
    async fn github_stop_and_wait(
        &self,
        db: &DatabaseConnection,
        github: &GithubClient,
        instance: &Instance,
        deployment: &mut Deployment,
    ) -> Result<(), DeployError> {
        deployment
            .update_status(db, DeploymentStatusType::Stopping)
            .await?;
        let run = instance.cleanup_github(github).await?;
        github
            .wait_for_success_workflow(&run, self.workflow_timeout, self.workflow_check_interval)
            .await?;
        deployment.mark_as_finished(db).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_utils;

    #[tokio::test]
    async fn stopping_task_works() {
        let (db, _github, repo, runner) =
            tests_utils::init::jobs_runner_test_case("stopping_task_works").await;
        let conn = db.client();
        let handles = repo.build_handles();

        let running_deployment_id = 1;
        let task = StoppingTask::new(
            running_deployment_id,
            Duration::from_millis(100),
            Duration::from_millis(100),
        );
        runner.insert_task(&task).await.unwrap();
        // no way to block on task, so wait for task to be executed
        tokio::time::sleep(Duration::from_secs(10)).await;

        let deployment = Deployment::get(conn.as_ref(), running_deployment_id)
            .await
            .unwrap();
        assert_eq!(
            deployment.model.status,
            DeploymentStatusType::Stopped,
            "deployment is not stopped. error: {:?}",
            deployment.model.error
        );

        handles.assert_hits("dispatch_cleanup_yaml", 1);
        handles.assert_hits("runs_cleanup_yaml", 1);
        handles.assert_hits("single_run_cleanup_yaml", 1);
    }
}
