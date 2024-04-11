use crate::{
    logic::{
        deploy::{deployment::map_deployment_status, instance::InstanceDeployment},
        users::UserToken,
        DeployError, Deployment, GithubClient,
    },
    server::proto,
};
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

const MIN_HOURS_DEPLOY: u64 = 12;

pub async fn update_instance_status(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_id: &str,
    action: &proto::UpdateInstanceAction,
    user_token: &UserToken,
) -> Result<proto::UpdateInstanceStatusResponseInternal, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let instance = InstanceDeployment::from_instance_id(db, instance_id).await?;
    user_token
        .has_access_to_instance(&instance.instance)
        .await?;
    let result = handle_instance_action(db, github, instance, action, user_token).await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(result)
}

async fn handle_instance_action<C>(
    db: &C,
    github: &GithubClient,
    instance: InstanceDeployment,
    action: &proto::UpdateInstanceAction,
    user_token: &UserToken,
) -> Result<proto::UpdateInstanceStatusResponseInternal, DeployError>
where
    C: ConnectionTrait,
{
    let current_status =
        map_deployment_status(instance.deployment.as_ref().map(|d| &d.model.status));
    let allowed_statuses = match &action {
        proto::UpdateInstanceAction::Start => vec![
            proto::DeploymentStatus::NoStatus,
            proto::DeploymentStatus::Stopped,
            proto::DeploymentStatus::Failed,
        ],
        proto::UpdateInstanceAction::Finish | proto::UpdateInstanceAction::Restart => {
            vec![proto::DeploymentStatus::Running]
        }
    };

    if !allowed_statuses.contains(&current_status) {
        return Err(DeployError::InvalidStateTransition(
            serde_plain::to_string(action).expect("enum should be serializable"),
            serde_plain::to_string(&current_status).expect("enum should be serializable"),
        ));
    }

    let deployment = match action {
        proto::UpdateInstanceAction::Start => {
            start_instance(db, github, instance, user_token).await?
        }
        proto::UpdateInstanceAction::Finish => {
            todo!("finish instance")
        }
        proto::UpdateInstanceAction::Restart => {
            todo!("restart instance")
        }
    };

    Ok(proto::UpdateInstanceStatusResponseInternal {
        status: map_deployment_status(Some(&deployment.model.status)),
        deployment_id: deployment.model.external_id.to_string(),
    })
}

async fn start_instance<C>(
    db: &C,
    github: &GithubClient,
    instance: InstanceDeployment,
    user_token: &UserToken,
) -> Result<Deployment, DeployError>
where
    C: ConnectionTrait,
{
    let spec = instance.instance.find_server_spec(db).await?;
    user_token
        .allowed_to_deploy_for_hours(MIN_HOURS_DEPLOY, &spec)
        .await?;
    let deployment =
        Deployment::try_create(db, &instance.instance, Some(DeploymentStatusType::Pending)).await?;
    let _run = instance.instance.deploy_via_github(github).await?;
    Ok(deployment)
}
