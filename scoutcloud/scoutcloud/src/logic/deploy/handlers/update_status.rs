use crate::{
    logic::{
        deploy::{deployment::map_deployment_status, handlers::user_actions},
        jobs::JobsRunner,
        users::UserToken,
        DeployError, Deployment, GithubClient, Instance, InstanceDeployment,
    },
    server::proto,
};

use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::DatabaseConnection;

const MIN_HOURS_DEPLOY: u64 = 12;

pub async fn update_instance_status(
    db: &DatabaseConnection,
    github: &GithubClient,
    runner: &JobsRunner,
    instance_id: &str,
    action: &proto::UpdateInstanceAction,
    user_token: &UserToken,
) -> Result<proto::UpdateInstanceStatusResponseInternal, DeployError> {
    let instance = InstanceDeployment::find_by_instance_uuid(db, instance_id)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_id.to_string()))?;
    user_token.has_access_to_instance(&instance.instance)?;
    let result = handle_instance_action(db, github, runner, instance, action, user_token).await?;
    Ok(result)
}

async fn handle_instance_action(
    db: &DatabaseConnection,
    _github: &GithubClient,
    runner: &JobsRunner,
    instance: InstanceDeployment,
    action: &proto::UpdateInstanceAction,
    user_token: &UserToken,
) -> Result<proto::UpdateInstanceStatusResponseInternal, DeployError> {
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
            start_instance(db, runner, &instance.instance, user_token).await?
        }
        proto::UpdateInstanceAction::Finish => {
            stop_instance(db, runner, &instance.instance, user_token).await?
        }
        proto::UpdateInstanceAction::Restart => Err(anyhow::anyhow!(
            "restart not implemented yet, use start and finish instead"
        ))?,
    };

    Ok(proto::UpdateInstanceStatusResponseInternal {
        status: map_deployment_status(Some(&deployment.model.status)),
        deployment_id: deployment.model.external_id.to_string(),
    })
}

async fn start_instance(
    db: &DatabaseConnection,
    runner: &JobsRunner,
    instance: &Instance,
    user_token: &UserToken,
) -> Result<Deployment, DeployError> {
    let spec = instance.find_server_spec(db).await?.ok_or(anyhow::anyhow!(
        "server size of instance not found in database"
    ))?;
    user_token
        .allowed_to_deploy_for_hours(MIN_HOURS_DEPLOY, &spec)
        .await?;
    let deployment =
        Deployment::try_create(db, instance, Some(DeploymentStatusType::Created)).await?;
    user_actions::log_start_instance(db, user_token, instance, &deployment).await?;
    runner.insert_starting_task(deployment.model.id).await?;
    Ok(deployment)
}

async fn stop_instance(
    db: &DatabaseConnection,
    runner: &JobsRunner,
    instance: &Instance,
    user_token: &UserToken,
) -> Result<Deployment, DeployError> {
    let deployment = Deployment::latest_of_instance(db, instance)
        .await?
        .ok_or(DeployError::DeploymentNotFound)?;
    user_actions::log_stop_instance(db, user_token, instance, &deployment).await?;
    runner.insert_stopping_task(deployment.model.id).await?;
    Ok(deployment)
}
