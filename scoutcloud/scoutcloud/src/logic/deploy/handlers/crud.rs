use crate::{
    logic::{
        deploy::{handlers::user_actions::user_action, instance::InstanceDeployment},
        users::UserToken,
        DeployError, GithubClient, Instance, UserConfig,
    },
    server::proto,
};
use sea_orm::{DatabaseConnection, TransactionTrait};
use serde_json::json;

pub async fn create_instance(
    db: &DatabaseConnection,
    github: &GithubClient,
    name: &str,
    config: &proto::DeployConfigInternal,
    creator: &UserToken,
) -> Result<proto::CreateInstanceResponseInternal, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let instance = Instance::try_create(&tx, name, config, creator).await?;
    instance.commit(github, "initial instance creation").await?;
    user_action(
        &tx,
        creator,
        "create_instance",
        Some(json!({
            "instance_id": instance.model.id,
            "instance_slug": instance.model.slug,
        })),
    )
    .await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

    Ok(proto::CreateInstanceResponseInternal {
        instance_id: instance.model.external_id.to_string(),
    })
}

pub async fn update_instance_config(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_id: &str,
    config: &proto::DeployConfigInternal,
    user_token: &UserToken,
) -> Result<UserConfig, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let mut instance = Instance::find(db, instance_id)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_id.to_string()))?;
    user_token.has_access_to_instance(&instance).await?;
    let updated_config = instance.update_config(&tx, config.clone()).await?;
    instance.commit(github, "full config update").await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

    Ok(updated_config)
}

pub async fn update_instance_config_partial(
    db: &DatabaseConnection,
    github: &GithubClient,
    instance_id: &str,
    config: &proto::DeployConfigPartialInternal,
    user_token: &UserToken,
) -> Result<UserConfig, DeployError> {
    let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
    let mut instance = Instance::find(db, instance_id)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_id.to_string()))?;
    user_token.has_access_to_instance(&instance).await?;
    let updated_config = instance.update_config_partial(&tx, config).await?;
    instance.commit(github, "partial config update").await?;
    tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

    Ok(updated_config)
}

pub async fn get_instance(
    db: &DatabaseConnection,
    instance_id: &str,
    user_token: &UserToken,
) -> Result<proto::InstanceInternal, DeployError> {
    let instance_deployment = InstanceDeployment::from_instance_id(db, instance_id).await?;
    user_token
        .has_access_to_instance(&instance_deployment.instance)
        .await?;
    proto::InstanceInternal::try_from(instance_deployment)
}

pub async fn list_instances(
    db: &DatabaseConnection,
    user_token: &UserToken,
) -> Result<Vec<proto::InstanceInternal>, DeployError> {
    let instances = InstanceDeployment::find_all(db, user_token).await?;
    instances
        .into_iter()
        .map(proto::InstanceInternal::try_from)
        .collect::<Result<Vec<_>, _>>()
}

pub async fn get_deployment(
    db: &DatabaseConnection,
    deployment_id: &str,
    user_token: &UserToken,
) -> Result<proto::DeploymentInternal, DeployError> {
    let result = InstanceDeployment::from_deployment_id(db, deployment_id).await?;
    user_token.has_access_to_instance(&result.instance).await?;
    proto::DeploymentInternal::try_from(result)
}

pub async fn get_current_deployment(
    db: &DatabaseConnection,
    instance_id: &str,
    user_token: &UserToken,
) -> Result<proto::DeploymentInternal, DeployError> {
    let result = InstanceDeployment::from_instance_id(db, instance_id).await?;
    user_token.has_access_to_instance(&result.instance).await?;
    proto::DeploymentInternal::try_from(result)
}

pub async fn list_deployments(
    db: &DatabaseConnection,
    instance_id: &str,
    user_token: &UserToken,
) -> Result<Vec<proto::DeploymentInternal>, DeployError> {
    let instance = Instance::find(db, instance_id)
        .await?
        .ok_or(DeployError::InstanceNotFound(instance_id.to_string()))?;
    user_token.has_access_to_instance(&instance).await?;
    let deployments = InstanceDeployment::find_all_for_instance(db, &instance).await?;
    deployments
        .into_iter()
        .map(proto::DeploymentInternal::try_from)
        .collect::<Result<Vec<_>, _>>()
}
