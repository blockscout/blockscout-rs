use super::deployment::{map_deployment_status, Deployment};
use crate::{
    logic::{
        github::{DeployWorkflow, Workflow},
        ConfigError, DeployError, GithubClient, InstanceConfig, UserConfig, UserToken,
    },
    server::proto,
    uuid_eq,
};
use scoutcloud_entity as db;
use sea_orm::{
    prelude::*, ActiveModelTrait, ActiveValue::Set, IntoActiveModel, QueryOrder, QuerySelect,
};

const MAX_LIMIT: u64 = 50;
const MAX_TRY_GITHUB: u8 = 10;

#[derive(Clone)]
pub struct Instance {
    pub model: db::instances::Model,
}

// Build functions
impl Instance {
    pub fn new(model: db::instances::Model) -> Self {
        Instance { model }
    }

    pub async fn find<C>(db: &C, id: &str) -> Result<Option<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let this = db::instances::Entity::find()
            .filter(uuid_eq!(db::instances::Column::ExternalId, id))
            .one(db)
            .await?
            .map(|model| Instance { model });
        Ok(this)
    }

    pub async fn find_all<C>(db: &C, creator_token_id: i32) -> Result<Vec<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let instances = Self::default_select()
            .filter(db::instances::Column::CreatorTokenId.eq(creator_token_id))
            .limit(MAX_LIMIT)
            .all(db)
            .await?
            .into_iter()
            .map(|model| Instance { model })
            .collect();
        Ok(instances)
    }

    pub async fn try_create<C>(
        db: &C,
        name: &str,
        config: &proto::DeployConfigInternal,
        creator: &UserToken,
    ) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let slug = slug::slugify(name);
        if let Some(instance) = db::instances::Entity::find()
            .filter(db::instances::Column::Slug.eq(&slug))
            .one(db)
            .await?
        {
            return Err(DeployError::InstanceExists(instance.slug));
        }

        let user_config = UserConfig::new(config.clone());
        let parsed_config =
            InstanceConfig::try_from_user_with_defaults(user_config.clone(), &slug).await?;

        let model = db::instances::ActiveModel {
            creator_token_id: Set(creator.token.id),
            slug: Set(slug),
            user_config: Set(user_config.raw()?),
            parsed_config: Set(parsed_config.raw().to_owned()),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(Instance { model })
    }
}

impl Instance {
    pub fn user_config_raw(&self) -> &serde_json::Value {
        &self.model.user_config
    }

    pub fn user_config(&self) -> Result<UserConfig, ConfigError> {
        UserConfig::from_raw(self.user_config_raw().clone())
    }

    pub fn parsed_config(&self) -> InstanceConfig {
        InstanceConfig::from_raw(self.model.parsed_config.clone())
    }

    pub fn default_select() -> Select<db::instances::Entity> {
        db::instances::Entity::find().order_by_desc(db::instances::Column::CreatedAt)
    }

    pub async fn commit(
        &self,
        github: &GithubClient,
        action_name: &str,
    ) -> Result<(), DeployError> {
        let file_name = get_filename(&self.model.slug);
        let content = self.parsed_config().to_yaml()?;
        github
            .create_or_update_file(&file_name, &content, action_name)
            .await?;
        Ok(())
    }

    pub async fn update_config<C>(
        &mut self,
        db: &C,
        config: impl Into<UserConfig>,
    ) -> Result<UserConfig, DeployError>
    where
        C: ConnectionTrait,
    {
        let config = config.into();
        let parsed_config =
            InstanceConfig::try_from_user_with_defaults(config.clone(), &self.model.slug).await?;
        let user_config_raw = config.raw()?;
        let parsed_config_raw = parsed_config.raw().to_owned();
        let model = self
            ._update_configs(db, user_config_raw, parsed_config_raw)
            .await?;
        self.model = model;
        Ok(config)
    }

    pub async fn update_config_partial<C>(
        &mut self,
        db: &C,
        config: &proto::DeployConfigPartialInternal,
    ) -> Result<UserConfig, DeployError>
    where
        C: ConnectionTrait,
    {
        let config = self.user_config()?.with_merged_partial(config)?;
        Self::update_config(self, db, config.clone()).await?;
        Ok(config)
    }

    async fn _update_configs<C>(
        &self,
        db: &C,
        user_config: serde_json::Value,
        parsed_config: serde_json::Value,
    ) -> Result<db::instances::Model, DeployError>
    where
        C: ConnectionTrait,
    {
        let mut active = self.model.clone().into_active_model();
        active.user_config = Set(user_config);
        active.parsed_config = Set(parsed_config);
        let model = active.update(db).await?;
        Ok(model)
    }

    pub async fn deployments<C>(&self, db: &C) -> Result<Vec<Deployment>, DeployError>
    where
        C: ConnectionTrait,
    {
        let deployments = Deployment::default_select()
            .filter(db::deployments::Column::InstanceId.eq(self.model.id))
            .limit(MAX_LIMIT)
            .all(db)
            .await?
            .into_iter()
            .map(|model| Deployment { model })
            .collect();
        Ok(deployments)
    }

    pub async fn find_server_spec<C>(&self, db: &C) -> Result<db::server_specs::Model, DeployError>
    where
        C: ConnectionTrait,
    {
        let server_size = self.user_config()?.internal.server_size;
        let server_spec = db::server_specs::Entity::find()
            .filter(db::server_specs::Column::Slug.eq(&server_size))
            .one(db)
            .await?
            .ok_or(DeployError::Internal(anyhow::anyhow!(
                "server size `{server_size}` not found in database"
            )))?;
        Ok(server_spec)
    }
}

// Starting and stopping instance using github api
impl Instance {
    pub async fn deploy_via_github(
        &self,
        github: &GithubClient,
    ) -> Result<octocrab::models::workflows::Run, DeployError> {
        let instance_run = DeployWorkflow::instance(self.model.slug.clone())
            .run_and_get_latest(github, MAX_TRY_GITHUB)
            .await?
            .ok_or(DeployError::Internal(anyhow::anyhow!(
                "no instance workflow found after running"
            )))?;
        tracing::info!(
            instance_id =? self.model.external_id,
            run_id =? instance_run.id,
            run_status =? instance_run.status,
            "triggered github deploy workflow for app=instance"
        );
        let postgres_run = DeployWorkflow::postgres(self.model.slug.clone())
            .run_and_get_latest(github, MAX_TRY_GITHUB)
            .await?
            .ok_or(DeployError::Internal(anyhow::anyhow!(
                "no postgres workflow found after running"
            )))?;
        tracing::info!(
            instance_id =? self.model.external_id,
            run_id =? postgres_run.id,
            run_status =? postgres_run.status,
            "triggered github deploy workflow for app=postgres"
        );
        Ok(instance_run)
    }
}

pub struct InstanceDeployment {
    pub instance: Instance,
    pub deployment: Option<Deployment>,
}

impl InstanceDeployment {
    pub async fn from_instance<C>(db: &C, instance: Instance) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let deployment = Deployment::latest_of_instance(db, &instance).await?;
        Ok(InstanceDeployment {
            instance,
            deployment,
        })
    }

    pub async fn from_instance_id<C>(db: &C, instance_id: &str) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let instance = Instance::find(db, instance_id)
            .await?
            .ok_or(DeployError::InstanceNotFound(instance_id.to_string()))?;
        Self::from_instance(db, instance).await
    }

    pub async fn from_deployment_id<C>(db: &C, _deployment_id: &str) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let (deployment, instance) = Deployment::default_select()
            .find_also_related(db::instances::Entity)
            .one(db)
            .await?
            .ok_or(DeployError::DeploymentNotFound)?;
        let instance = instance.ok_or(DeployError::Internal(anyhow::anyhow!(
            "deployment without instance"
        )))?;

        Ok(Self {
            instance: Instance::new(instance),
            deployment: Some(Deployment::new(deployment)),
        })
    }

    pub async fn find_all<C>(db: &C, owner: &UserToken) -> Result<Vec<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let instances: Vec<db::instances::Model> = Instance::find_all(db, owner.token.id)
            .await?
            .into_iter()
            .map(|i| i.model)
            .collect();
        let deployments = instances
            .load_many(Deployment::default_select().limit(1), db)
            .await?;

        instances
            .into_iter()
            .zip(deployments.into_iter())
            .map(|(instance, mut deployments)| {
                Ok(InstanceDeployment {
                    instance: Instance { model: instance },
                    deployment: deployments.pop().map(|model| Deployment { model }),
                })
            })
            .collect()
    }

    pub async fn find_all_for_instance<C>(
        db: &C,
        instance: &Instance,
    ) -> Result<Vec<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let deployments = instance.deployments(db).await?;
        Ok(deployments
            .into_iter()
            .map(|d| InstanceDeployment {
                instance: instance.clone(),
                deployment: Some(d),
            })
            .collect())
    }
}

impl TryFrom<InstanceDeployment> for proto::InstanceInternal {
    type Error = DeployError;

    fn try_from(value: InstanceDeployment) -> Result<Self, Self::Error> {
        let instance = value.instance;
        let deployment = value.deployment;
        let user_config = instance.user_config()?;
        let proto_instance = proto::InstanceInternal {
            instance_id: instance.model.external_id.to_string(),
            name: instance.model.slug.clone(),
            created_at: instance.model.created_at.to_string(),
            config: Some(user_config.internal),
            deployment_id: deployment.as_ref().map(|d| d.model.external_id.to_string()),
            deployment_status: map_deployment_status(deployment.as_ref().map(|d| &d.model.status)),
        };
        Ok(proto_instance)
    }
}

impl TryFrom<InstanceDeployment> for proto::DeploymentInternal {
    type Error = DeployError;

    fn try_from(value: InstanceDeployment) -> Result<Self, Self::Error> {
        let instance = value.instance;
        let deployment = value.deployment.ok_or(DeployError::DeploymentNotFound)?;
        let config = deployment.user_config()?;
        Ok(Self {
            deployment_id: deployment.model.external_id.to_string(),
            instance_id: instance.model.external_id.to_string(),
            status: map_deployment_status(Some(&deployment.model.status)),
            error: deployment.model.error,
            created_at: deployment.model.created_at.to_string(),
            finished_at: deployment.model.finished_at.map(|t| t.to_string()),
            config: Some(config.internal),
        })
    }
}

fn get_filename(slug: &str) -> String {
    format!("values-{}.yaml", slug)
}
