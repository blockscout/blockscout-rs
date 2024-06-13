#![allow(clippy::blocks_in_conditions)]

use crate::logic::{jobs::global, DeployError, Deployment};
use blockscout_client::{apis::health_api, Configuration};
use fang::{typetag, AsyncQueueable, AsyncRunnable, FangError, Scheduled};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::prelude::*;
use tracing::instrument;

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
            schedule: Some("30 * * * * *".to_string()),
            #[cfg(test)]
            database_url: None,
        }
    }
}

#[typetag::serde]
#[fang::async_trait]
impl AsyncRunnable for SuperviseTask {
    #[instrument(err(Debug), skip_all, level = "debug")]
    async fn run(&self, _client: &dyn AsyncQueueable) -> Result<(), FangError> {
        let db = global::DATABASE.get().await;
        let deployments = Deployment::find_active(db.as_ref())
            .await
            .map_err(DeployError::Db)?;
        for mut deployment in deployments {
            check_deployment_health(db.as_ref(), &mut deployment).await?;
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

#[instrument(err, skip(db, deployment), level = "debug", fields(deployment_id = deployment.model.id))]
pub async fn check_deployment_health<C: ConnectionTrait>(
    db: &C,
    deployment: &mut Deployment,
) -> Result<(), DeployError> {
    let instance_url = deployment.instance_config().parse_instance_url()?;
    let health_response = health_api::health(&Configuration::new(instance_url.clone())).await;
    match health_response {
        Ok(response) if response.healthy.unwrap_or_default() => {
            if !deployment.is_started() {
                deployment.mark_as_started(db).await?;
            }
            if deployment.model.status != DeploymentStatusType::Running {
                deployment
                    .update_status(db, DeploymentStatusType::Running)
                    .await?;
            }
        }
        Ok(response) => {
            if deployment.model.status != DeploymentStatusType::Unhealthy {
                tracing::warn!("instance {} is unhealthy: {:?}", instance_url, response);
            }
            deployment
                .mark_as_unhealthy(
                    db,
                    Some(format!(
                        "blockscout '{instance_url}' responded with unhealthy status"
                    )),
                )
                .await?;
        }
        Err(err) => {
            if deployment.model.status != DeploymentStatusType::Unhealthy {
                tracing::error!(
                    "failed to check health of instance {}: {:?}",
                    instance_url,
                    err
                );
            }
            deployment
                .mark_as_unhealthy(db, Some(format!("failed to check health: {}", err)))
                .await?;
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_utils::{
        blockscout::{start_blockscout_and_set_url, update_blockscout_url_of_all_instances},
        db::wait_for_empty_fang_tasks,
        init::jobs_runner_test_case,
    };

    #[tokio::test]
    #[serial_test::serial]
    async fn supervisor_works() {
        let (db, _github, _repo, runner) = jobs_runner_test_case("supervisor_works").await;
        let conn = db.client();
        let task = SuperviseTask {
            schedule: None,
            database_url: Some(db.db_url().to_string()),
        };

        {
            let _blockscout = start_blockscout_and_set_url(conn.as_ref(), true, true).await;
            runner.insert_task(&task).await.unwrap();
            wait_for_empty_fang_tasks(conn.clone()).await.unwrap();

            Deployment::find_active(conn.as_ref())
                .await
                .unwrap()
                .iter()
                .for_each(|deployment| {
                    assert_eq!(deployment.model.status, DeploymentStatusType::Running);
                });
        }

        {
            let _blockscout = start_blockscout_and_set_url(conn.as_ref(), false, true).await;
            runner.insert_task(&task).await.unwrap();
            wait_for_empty_fang_tasks(conn.clone()).await.unwrap();

            Deployment::find_active(conn.as_ref())
                .await
                .unwrap()
                .iter()
                .for_each(|deployment| {
                    assert_eq!(deployment.model.status, DeploymentStatusType::Unhealthy);
                    assert!(
                        deployment
                            .model
                            .error
                            .as_ref()
                            .unwrap()
                            .contains("responded with unhealthy status"),
                        "invalid error: {:?}",
                        deployment.model.error
                    );
                });
        }

        {
            let _blockscout = start_blockscout_and_set_url(conn.as_ref(), true, true).await;
            runner.insert_task(&task).await.unwrap();
            wait_for_empty_fang_tasks(conn.clone()).await.unwrap();

            // set blockscout url for deployment to random value
            update_blockscout_url_of_all_instances(conn.as_ref(), "http://localhost:1234").await;
            runner.insert_task(&task).await.unwrap();
            wait_for_empty_fang_tasks(conn.clone()).await.unwrap();
            Deployment::find_active(conn.as_ref())
                .await
                .unwrap()
                .iter()
                .for_each(|deployment| {
                    assert_eq!(deployment.model.status, DeploymentStatusType::Unhealthy);
                    assert!(
                        deployment
                            .model
                            .error
                            .as_ref()
                            .unwrap()
                            .contains("failed to check health"),
                        "invalid error: {:?}",
                        deployment.model.error
                    );
                });
        }
    }
}
