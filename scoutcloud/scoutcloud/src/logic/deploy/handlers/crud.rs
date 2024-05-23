use crate::{
    logic::{
        users::{user_actions, UserToken},
        DeployError, GithubClient, Instance, InstanceDeployment, UserConfig,
    },
    server::proto,
};
use sea_orm::{DatabaseConnection, TransactionTrait};

pub async fn create_instance(
    db: &DatabaseConnection,
    github: &GithubClient,
    name: &str,
    config: &proto::DeployConfigInternal,
    creator: &UserToken,
) -> Result<proto::CreateInstanceResponseInternal, DeployError> {
    let tx = db.begin().await?;
    creator.allowed_to_create_instance(&tx).await?;
    let instance = Instance::try_create(&tx, name, config, creator).await?;
    let config = instance.user_config_raw().clone();
    user_actions::log_create_instance(&tx, creator, &instance, &config).await?;
    instance.commit(github, "initial instance creation").await?;
    tx.commit().await?;

    Ok(proto::CreateInstanceResponseInternal {
        instance_id: instance.model.external_id.to_string(),
    })
}

pub async fn update_instance_config(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_uuid: &str,
    config: &proto::DeployConfigInternal,
    user_token: &UserToken,
) -> Result<UserConfig, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let mut instance = Instance::find_by_uuid(db, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&instance)?;
    let old_config = instance.user_config_raw().clone();
    let updated_config = instance.update_config(&tx, config.clone()).await?;
    user_actions::log_update_config(
        &tx,
        user_token,
        &instance,
        &old_config,
        &updated_config.raw()?,
        false,
    )
    .await?;
    instance.commit(github, "config update").await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

    Ok(updated_config)
}

pub async fn update_instance_config_partial(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_uuid: &str,
    config: &proto::DeployConfigPartialInternal,
    user_token: &UserToken,
) -> Result<UserConfig, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let mut instance = Instance::find_by_uuid(db, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&instance)?;
    let old_config = instance.user_config_raw().clone();
    let updated_config = instance.update_config_partial(&tx, config).await?;
    user_actions::log_update_config(
        &tx,
        user_token,
        &instance,
        &old_config,
        &updated_config.raw()?,
        true,
    )
    .await?;
    instance.commit(github, "partial config update").await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

    Ok(updated_config)
}

pub async fn get_instance(
    db: &DatabaseConnection,
    instance_uuid: &str,
    user_token: &UserToken,
) -> Result<proto::InstanceInternal, DeployError> {
    let instance_deployment = InstanceDeployment::find_by_instance_uuid(db, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&instance_deployment.instance)?;
    proto::InstanceInternal::try_from(instance_deployment)
}

pub async fn list_instances(
    db: &DatabaseConnection,
    user_token: &UserToken,
) -> Result<Vec<proto::InstanceInternal>, DeployError> {
    let instances = InstanceDeployment::find_all_instances(db, user_token).await?;
    instances
        .into_iter()
        // find_all should return only instances that the user has access to,
        // but we filter them again just in case
        .filter(|i| user_token.has_access_to_instance(&i.instance).is_ok())
        .map(proto::InstanceInternal::try_from)
        .collect::<Result<Vec<_>, _>>()
}

pub async fn delete_instance(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_uuid: &str,
    user_token: &UserToken,
) -> Result<(), DeployError> {
    let tx = db.begin().await?;
    let mut instance_deployment = InstanceDeployment::find_by_instance_uuid(&tx, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&instance_deployment.instance)?;
    let status = instance_deployment.deployment_status();
    match status {
        proto::DeploymentStatus::NoStatus
        | proto::DeploymentStatus::Created
        | proto::DeploymentStatus::Stopped
        | proto::DeploymentStatus::Failed => {
            // Ok
        }
        _ => {
            return Err(DeployError::InvalidStateTransition(
                "delete".to_string(),
                serde_plain::to_string(&status).expect("enum should be serializable"),
            ));
        }
    }

    user_actions::log_delete_instance(&tx, user_token, &instance_deployment.instance).await?;
    instance_deployment.instance.mark_as_deleted(&tx).await?;
    // deleting a file cannot be reverted, so we do it last
    instance_deployment.instance.delete_file(github).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn get_deployment(
    db: &DatabaseConnection,
    deployment_uuid: &str,
    user_token: &UserToken,
) -> Result<proto::DeploymentInternal, DeployError> {
    let result = InstanceDeployment::find_by_deployment_uuid(db, deployment_uuid)
        .await?
        .ok_or(DeployError::DeploymentNotFound)?;
    user_token.has_access_to_instance(&result.instance)?;
    proto::DeploymentInternal::try_from(result)
}

pub async fn get_current_deployment(
    db: &DatabaseConnection,
    instance_uuid: &str,
    user_token: &UserToken,
) -> Result<proto::DeploymentInternal, DeployError> {
    let result = InstanceDeployment::find_by_instance_uuid(db, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&result.instance)?;
    proto::DeploymentInternal::try_from(result)
}

pub async fn list_deployments(
    db: &DatabaseConnection,
    instance_uuid: &str,
    user_token: &UserToken,
) -> Result<Vec<proto::DeploymentInternal>, DeployError> {
    let instance = Instance::find_by_uuid(db, instance_uuid)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_uuid.to_string()))?;
    user_token.has_access_to_instance(&instance)?;
    let deployments = InstanceDeployment::find_deployments_of_instance(db, &instance).await?;
    deployments
        .into_iter()
        .map(proto::DeploymentInternal::try_from)
        .collect::<Result<Vec<_>, _>>()
}
