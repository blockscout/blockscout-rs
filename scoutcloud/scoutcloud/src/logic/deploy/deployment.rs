use crate::{
    logic::{ConfigError, DeployError, Instance, InstanceConfig, UserConfig},
    server::proto,
    uuid_eq,
};

use db::sea_orm_active_enums::DeploymentStatusType;
use scoutcloud_entity as db;
use sea_orm::{
    prelude::*, ActiveValue::Set, Condition, ConnectionTrait, IntoActiveModel, NotSet, QueryOrder,
};

pub struct Deployment {
    pub model: db::deployments::Model,
}

// Build functions
impl Deployment {
    pub fn new(model: db::deployments::Model) -> Self {
        Deployment { model }
    }

    pub async fn try_create<C>(
        db: &C,
        instance: &Instance,
        maybe_status: Option<DeploymentStatusType>,
    ) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let server_spec = instance.find_server_spec(db).await?.ok_or(anyhow::anyhow!(
            "server spec of the instance was not found in database"
        ))?;
        let model = db::deployments::ActiveModel {
            instance_id: Set(instance.model.id),
            user_config: Set(instance.model.user_config.clone()),
            parsed_config: Set(instance.model.parsed_config.clone()),
            server_spec_id: Set(server_spec.id),
            status: maybe_status.map(Set).unwrap_or(NotSet),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(Deployment { model })
    }

    pub async fn update_from_instance<C: ConnectionTrait>(
        &mut self,
        db: &C,
        instance: &Instance,
    ) -> Result<&mut Self, DeployError> {
        let server_spec = instance.find_server_spec(db).await?.ok_or(anyhow::anyhow!(
            "server spec of the instance was not found in database"
        ))?;
        let mut model = self.model.clone().into_active_model();
        model.user_config = Set(instance.model.user_config.clone());
        model.parsed_config = Set(instance.model.parsed_config.clone());
        model.server_spec_id = Set(server_spec.id);
        self.model = model.update(db).await?;
        Ok(self)
    }

    pub async fn get<C>(db: &C, id: i32) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let model = Self::default_select()
            .filter(db::deployments::Column::Id.eq(id))
            .one(db)
            .await?
            .ok_or(DbErr::Custom("no deployment found".into()))?;
        Ok(Deployment { model })
    }

    pub async fn latest_of_instance<C>(db: &C, instance: &Instance) -> Result<Option<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
        let deployment = Self::default_select()
            .filter(db::deployments::Column::InstanceId.eq(instance.model.id))
            .one(db)
            .await?
            .map(|model| Deployment { model });
        Ok(deployment)
    }

    pub async fn find_by_uuid<C>(db: &C, uuid: impl Into<String>) -> Result<Option<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
        let deployment = Self::default_select()
            .filter(uuid_eq!(db::deployments::Column::ExternalId, uuid.into()))
            .one(db)
            .await?
            .map(|model| Deployment { model });
        Ok(deployment)
    }

    pub async fn find_active<C: ConnectionTrait>(db: &C) -> Result<Vec<Self>, DbErr> {
        let deployments = Self::default_select()
            .filter(
                Condition::any()
                    .add(
                        scoutcloud_entity::deployments::Column::Status
                            .eq(DeploymentStatusType::Running),
                    )
                    .add(
                        scoutcloud_entity::deployments::Column::Status
                            .eq(DeploymentStatusType::Unhealthy),
                    ),
            )
            .all(db)
            .await?
            .into_iter()
            .map(Self::new)
            .collect();
        Ok(deployments)
    }
}

impl Deployment {
    pub fn default_select() -> Select<db::deployments::Entity> {
        db::deployments::Entity::find().order_by_desc(db::deployments::Column::CreatedAt)
    }

    pub fn user_config(&self) -> Result<UserConfig, ConfigError> {
        UserConfig::from_raw(self.user_config_raw().clone())
    }

    pub fn user_config_raw(&self) -> &serde_json::Value {
        &self.model.user_config
    }

    pub fn instance_config(&self) -> InstanceConfig {
        InstanceConfig::from_raw(self.model.parsed_config.clone())
    }

    pub async fn get_instance<C>(&self, db: &C) -> Result<Instance, DbErr>
    where
        C: ConnectionTrait,
    {
        Instance::get(db, self.model.instance_id).await
    }

    pub async fn update_status<C>(
        &mut self,
        db: &C,
        status: DeploymentStatusType,
    ) -> Result<&mut Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut model = self.model.clone().into_active_model();
        model.status = Set(status);
        self.model = model.update(db).await?;
        Ok(self)
    }

    pub async fn mark_as_error<C>(
        &mut self,
        db: &C,
        error: impl Into<String>,
    ) -> Result<&mut Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut model = self.model.clone().into_active_model();
        model.error = Set(Some(error.into()));
        model.status = Set(DeploymentStatusType::Failed);
        self.model = model.update(db).await?;
        Ok(self)
    }

    pub async fn mark_as_unhealthy<C>(
        &mut self,
        db: &C,
        maybe_error: Option<impl Into<String>>,
    ) -> Result<&mut Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut model = self.model.clone().into_active_model();
        model.status = Set(DeploymentStatusType::Unhealthy);
        model.error = Set(maybe_error.map(Into::into));
        self.model = model.update(db).await?;
        Ok(self)
    }

    pub async fn mark_as_stopped<C>(&mut self, db: &C) -> Result<&mut Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut model = self.model.clone().into_active_model();
        model.status = Set(DeploymentStatusType::Stopped);
        model.finished_at = Set(Some(chrono::Utc::now().fixed_offset()));
        self.model = model.update(db).await?;
        Ok(self)
    }

    pub async fn mark_as_running<C>(&mut self, db: &C) -> Result<&mut Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let instance_url = self.instance_config().parse_instance_url()?;
        let mut model = self.model.clone().into_active_model();
        model.status = Set(DeploymentStatusType::Running);
        model.started_at = Set(Some(chrono::Utc::now().fixed_offset()));
        model.instance_url = Set(Some(instance_url.to_string()));
        self.model = model.update(db).await?;
        Ok(self)
    }
}

pub fn map_deployment_status(status: Option<&DeploymentStatusType>) -> proto::DeploymentStatus {
    match status {
        None => proto::DeploymentStatus::NoStatus,
        Some(DeploymentStatusType::Created) => proto::DeploymentStatus::Created,
        Some(DeploymentStatusType::Pending) => proto::DeploymentStatus::Pending,
        Some(DeploymentStatusType::Running) => proto::DeploymentStatus::Running,
        Some(DeploymentStatusType::Failed) => proto::DeploymentStatus::Failed,
        Some(DeploymentStatusType::Stopping) => proto::DeploymentStatus::Stopping,
        Some(DeploymentStatusType::Stopped) => proto::DeploymentStatus::Stopped,
        Some(DeploymentStatusType::Unhealthy) => proto::DeploymentStatus::Unhealthy,
    }
}
