use crate::logic::{jobs::global, DeployError, Deployment, GithubClient, Instance};
use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::DatabaseConnection;
use std::time::Duration;

const WORKFLOW_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const WORKFLOW_CHECK_INTERVAL: Duration = Duration::from_secs(5);

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Debug)]
#[serde(crate = "fang::serde")]
pub struct StoppingTask {
    deployment_id: i32,
}

impl StoppingTask {
    pub fn new(deployment_id: i32) -> Self {
        Self { deployment_id }
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

        let allowed_statuses = [DeploymentStatusType::Running];
        if !allowed_statuses.contains(&deployment.model.status) {
            tracing::warn!(
                "cannot stop deployment '{}': not in running state",
                self.deployment_id
            );
            return Ok(());
        };

        if let Err(err) =
            github_stop_and_wait(db.as_ref(), github.as_ref(), &instance, &mut deployment).await
        {
            tracing::error!("failed to stop deployment: {}", err);
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

async fn github_stop_and_wait(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance: &Instance,
    deployment: &mut Deployment,
) -> Result<(), DeployError> {
    deployment
        .update_status(db, DeploymentStatusType::Stopping)
        .await?;
    let microservices_run = instance.cleanup_instance(github).await?;
    github
        .wait_for_success_workflow(
            "clean microservices",
            microservices_run.id,
            WORKFLOW_TIMEOUT,
            WORKFLOW_CHECK_INTERVAL,
        )
        .await?;
    // no need to wait before cleaning postgres
    let postgres_run = instance.cleanup_postgres(github).await?;
    github
        .wait_for_success_workflow(
            "clean postgres",
            postgres_run.id,
            WORKFLOW_TIMEOUT,
            WORKFLOW_CHECK_INTERVAL,
        )
        .await?;
    deployment.mark_as_finished(db).await?;
    Ok(())
}
