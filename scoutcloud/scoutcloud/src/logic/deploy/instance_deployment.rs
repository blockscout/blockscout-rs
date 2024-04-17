use crate::{
    logic::{
        deploy::deployment::map_deployment_status, DeployError, Deployment, Instance, UserToken,
    },
    server::proto,
    uuid_eq,
};
use scoutcloud_entity as db;
use sea_orm::{ConnectionTrait, LoaderTrait, QueryFilter, QuerySelect};

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

    pub async fn from_deployment_id<C>(db: &C, deployment_id: &str) -> Result<Self, DeployError>
    where
        C: ConnectionTrait,
    {
        let (deployment, instance) = Deployment::default_select()
            .filter(uuid_eq!(db::deployments::Column::ExternalId, deployment_id))
            .find_also_related(db::instances::Entity)
            .one(db)
            .await?
            .ok_or(DeployError::DeploymentNotFound)?;
        let instance = instance.ok_or(anyhow::anyhow!("deployment without instance"))?;

        Ok(Self {
            instance: Instance::new(instance),
            deployment: Some(Deployment::new(deployment)),
        })
    }

    pub async fn find_all<C>(db: &C, owner: &UserToken) -> Result<Vec<Self>, DeployError>
    where
        C: ConnectionTrait,
    {
        let instances: Vec<db::instances::Model> = Instance::find_all(db, owner)
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
