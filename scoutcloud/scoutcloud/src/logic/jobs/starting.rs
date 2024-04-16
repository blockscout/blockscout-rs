use fang::{AsyncQueueable, AsyncRunnable, FangError, Scheduled, typetag};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use crate::logic::{Deployment, Instance, InstanceDeployment};
use super::global;


#[derive(fang::serde::Serialize, fang::serde::Deserialize)]
#[serde(crate = "fang::serde")]
pub struct StartingTask {
    deployment_id: i64,
}

impl StartingTask {
    pub fn new(deployment_id: i64) -> Self {
        Self { deployment_id }
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for StartingTask {
    async fn run(&self, _client: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::get_db_connection();
        let github = global::get_github_client();

        let deployment = Deployment::get(db.as_ref(), self.deployment_id).await.map_err(map_db_err)?;

        InstanceDeployment::from_deployment_id()
        let allowed_statuses = vec![
            DeploymentStatusType::Created,
            DeploymentStatusType::Stopped,
        ];
        if !allowed_statuses.contains(&deployment.model.status) {
            tracing::warn!("cannot start deployment '{}': not in created/stopped state", self.deployment_id);
            return Ok(());
        };



        Ok(())
    }

    fn cron(&self) -> Option<Scheduled> {
        None
    }
}


fn map_db_err(e: sea_orm::error::DbErr) -> FangError {
    FangError {
        description: e.to_string(),
    }
}