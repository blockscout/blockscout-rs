use super::deployment::Deployment;
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

    pub async fn find<C>(db: &C, id: &str) -> Result<Option<Self>, DbErr>
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

    pub async fn find_all<C>(db: &C, user_token: &UserToken) -> Result<Vec<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
        let instances = Self::default_select()
            .filter(db::instances::Column::CreatorId.eq(user_token.user.id))
            .limit(MAX_LIMIT)
            .all(db)
            .await?
            .into_iter()
            .map(|model| Instance { model })
            .collect();
        Ok(instances)
    }

    pub async fn count<C>(db: &C, creator: &UserToken) -> Result<u64, DbErr>
    where
        C: ConnectionTrait,
    {
        let count = Self::default_select()
            .filter(db::instances::Column::CreatorId.eq(creator.user.id))
            .count(db)
            .await?;
        Ok(count)
    }

    // pub async fn get<C>(db: &C, id: i64) -> Result<Self, DbErr>
    // where
    //     C: ConnectionTrait,
    // {
    //     let model = db::instances::Entity::find()
    //         .filter(db::instances::Column::Id.eq(id))
    //         .one(db)
    //         .await?
    //         .ok_or(DbErr::Custom("no instance found".into()))?;
    //     Ok(Instance { model })
    // }

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
            creator_id: Set(creator.user.id),
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
            .ok_or(anyhow::anyhow!(
                "server size `{server_size}` not found in database"
            ))?;
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
            .run_and_get_latest_with_mutex(github, MAX_TRY_GITHUB)
            .await?
            .ok_or(anyhow::anyhow!("no instance workflow found after running"))?;
        tracing::info!(
            instance_id =? self.model.external_id,
            run_id =? instance_run.id,
            run_status =? instance_run.status,
            "triggered github deploy workflow for app=instance"
        );
        let postgres_run = DeployWorkflow::postgres(self.model.slug.clone())
            .run_and_get_latest_with_mutex(github, MAX_TRY_GITHUB)
            .await?
            .ok_or(anyhow::anyhow!("no postgres workflow found after running"))?;
        tracing::info!(
            instance_id =? self.model.external_id,
            run_id =? postgres_run.id,
            run_status =? postgres_run.status,
            "triggered github deploy workflow for app=postgres"
        );
        Ok(instance_run)
    }
}

fn get_filename(slug: &str) -> String {
    format!("values-{}.yaml", slug)
}
