use crate::logic::{blockscout::blockscout_health, jobs::global, DeployError, Deployment};
use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::{prelude::*, FromQueryResult, QuerySelect};

#[derive(fang::serde::Serialize, fang::serde::Deserialize, Debug)]
#[serde(crate = "fang::serde")]
pub struct SuperviseTask {
    schedule: Option<String>,
    #[cfg(test)]
    database_url: Option<String>,
}

impl Default for SuperviseTask {
    fn default() -> Self {
        Self {
            schedule: Some("0 * * * * *".to_string()),
            #[cfg(test)]
            database_url: None,
        }
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for SuperviseTask {
    async fn run(&self, client: &dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::DATABASE.get().await;
        let deployments = Deployment::find_running(db.as_ref())
            .await
            .map_err(DeployError::Db)?;
        for deployment in deployments {
            check_deployment_health(&deployment).await;
        }
        Ok(())
    }

    fn uniq(&self) -> bool {
        true
    }

    fn cron(&self) -> Option<Scheduled> {
        self.schedule
            .as_ref()
            .map(|s| Scheduled::CronPattern(s.clone()))
    }
}

async fn check_deployment_health(deployment: &Deployment) -> Result<(), anyhow::Error> {
    let instance_url = deployment.instance_config().parse_instance_url()?;
    let health_response = blockscout_health(&instance_url).await?;
    let new_status = if health_response.healthy {
        todo!()
    } else {
        todo!()
    };
    Ok(())
}
