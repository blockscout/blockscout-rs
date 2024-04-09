use crate::{
    logic::{ConfigError, DeployError, Instance, UserConfig},
    server::proto,
    uuid_eq,
};
use scoutcloud_entity as db;
use scoutcloud_entity::sea_orm_active_enums::DeploymentStatusType;
use sea_orm::{prelude::*, ConnectionTrait, QueryOrder};

pub struct Deployment {
    pub model: db::deployments::Model,
}

impl Deployment {
    pub fn new(model: db::deployments::Model) -> Self {
        Deployment { model }
    }

    pub fn default_loader() -> Select<db::deployments::Entity> {
        db::deployments::Entity::find().order_by_desc(db::deployments::Column::CreatedAt)
    }

    pub async fn latest_of_instance<C>(
        db: &C,
        instance: &Instance,
    ) -> Result<Option<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let deployment = Self::default_loader()
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
        let deployment = Self::default_loader()
            .filter(uuid_eq!(db::deployments::Column::ExternalId, id.into()))
            .one(db)
            .await?
            .map(|model| Deployment { model });
        Ok(deployment)
    }

    pub fn user_config(&self) -> Result<UserConfig, ConfigError> {
        UserConfig::parse(self.user_config_raw().clone())
    }

    pub fn user_config_raw(&self) -> &serde_json::Value {
        &self.model.user_config
    }
}

pub fn map_deployment_status(status: Option<&DeploymentStatusType>) -> proto::DeploymentStatus {
    match status {
        Some(DeploymentStatusType::Created) => proto::DeploymentStatus::Created,
        Some(DeploymentStatusType::Pending) => proto::DeploymentStatus::Pending,
        Some(DeploymentStatusType::Running) => proto::DeploymentStatus::Running,
        Some(DeploymentStatusType::Failed) => proto::DeploymentStatus::Failed,
        Some(DeploymentStatusType::Stopping) => proto::DeploymentStatus::Stopping,
        Some(DeploymentStatusType::Stopped) => proto::DeploymentStatus::Stopped,
        None => proto::DeploymentStatus::NoStatus,
    }
}
