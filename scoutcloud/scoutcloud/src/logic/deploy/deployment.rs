use crate::{
    logic::{ConfigError, DeployError, Instance, UserConfig},
    server::proto,
    uuid_eq,
};
use db::sea_orm_active_enums::DeploymentStatusType;
use scoutcloud_entity as db;
use sea_orm::{prelude::*, ActiveValue::Set, ConnectionTrait, IntoActiveModel, NotSet, QueryOrder};

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
        let model = db::deployments::ActiveModel {
            instance_id: Set(instance.model.id),
            user_config: Set(instance.model.user_config.clone()),
            parsed_config: Set(instance.model.parsed_config.clone()),
            server_spec_id: Set(instance.find_server_spec(db).await?.id),
            status: maybe_status.map(Set).unwrap_or(NotSet),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(Deployment { model })
    }

    pub async fn latest_of_instance<C>(
        db: &C,
        instance: &Instance,
    ) -> Result<Option<Self>, DeployError>
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

    pub async fn find<C>(db: &C, id: impl Into<String>) -> Result<Option<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let deployment = Self::default_select()
            .filter(uuid_eq!(db::deployments::Column::ExternalId, id.into()))
            .one(db)
            .await?
            .map(|model| Deployment { model });
        Ok(deployment)
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
        let updated = model.insert(db).await?;
        self.model = updated;
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
    }
}
