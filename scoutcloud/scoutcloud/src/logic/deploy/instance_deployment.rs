use crate::{
    logic::{
        deploy::deployment::map_deployment_status, DeployError, Deployment, Instance, UserToken,
    },
    server::proto,
    uuid_eq,
};
use scoutcloud_entity as db;
use sea_orm::{prelude::*, ConnectionTrait, DbErr, LoaderTrait, QueryFilter, QuerySelect};

pub struct InstanceDeployment {
    pub instance: Instance,
    pub deployment: Option<Deployment>,
}

impl InstanceDeployment {
    pub async fn from_instance<C>(db: &C, instance: Instance) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let deployment = Deployment::latest_of_instance(db, &instance).await?;
        Ok(InstanceDeployment {
            instance,
            deployment,
        })
    }

    pub async fn find_by_instance_uuid<C>(
        db: &C,
        instance_uuid: &str,
    ) -> Result<Option<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
        let instance = match Instance::find_by_uuid(db, instance_uuid).await? {
            Some(instance) => instance,
            None => return Ok(None),
        };
        Self::from_instance(db, instance).await.map(Some)
    }

    pub async fn find_by_deployment_uuid<C>(
        db: &C,
        deployment_uuid: &str,
    ) -> Result<Option<Self>, DbErr>
    where
        C: ConnectionTrait,
    {
        let (deployment, instance) = match Deployment::default_select()
            .filter(uuid_eq!(
                db::deployments::Column::ExternalId,
                deployment_uuid
            ))
            .find_also_related(db::instances::Entity)
            .one(db)
            .await?
        {
            Some((deployment, instance)) => (deployment, instance),
            None => return Ok(None),
        };
        let instance = instance.ok_or(DbErr::Custom("deployment without instance".into()))?;

        Ok(Some(Self {
            instance: Instance::new(instance),
            deployment: Some(Deployment::new(deployment)),
        }))
    }

    pub async fn find_all_instances<C>(db: &C, owner: &UserToken) -> Result<Vec<Self>, DeployError>
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

    pub async fn find_deployments_of_instance<C>(
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
            name: instance.model.name.clone(),
            slug: instance.model.slug.clone(),
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
            started_at: deployment.model.started_at.map(|t| t.to_string()),
            finished_at: deployment.model.finished_at.map(|t| t.to_string()),
            config: Some(config.internal),
            blockscout_url: deployment.model.instance_url,
            total_cost: deployment.model.total_cost.to_string(),
        })
    }
}
