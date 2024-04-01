use crate::logic::{
    deploy::{types::config_to_json, DeployError},
    github::GithubClient,
    users::UserToken,
    ConfigError, GeneratedInstanceConfig,
};
use scoutcloud_entity as db;
use scoutcloud_proto::blockscout::scoutcloud::v1::CreateInstanceRequestInternal;
use sea_orm::{
    prelude::*, ActiveModelTrait, ActiveValue::Set, DatabaseConnection, TransactionTrait,
};

pub struct Instance {
    pub model: db::instances::Model,
}

impl Instance {
    pub async fn find(db: &DatabaseConnection, id: &str) -> Result<Option<Self>, DeployError> {
        let this = db::instances::Entity::find()
            .filter(db::instances::Column::ExternalId.eq(id))
            .one(db)
            .await?
            .map(|model| Instance { model });
        Ok(this)
    }

    pub async fn try_create(
        db: &DatabaseConnection,
        r: &CreateInstanceRequestInternal,
        creator: &UserToken,
    ) -> Result<Self, DeployError> {
        let tx = db.begin().await.map_err(|e| anyhow::anyhow!(e))?;
        if let Some(instance) = db::instances::Entity::find()
            .filter(db::instances::Column::Slug.eq(&r.name))
            .one(&tx)
            .await?
        {
            return Err(DeployError::InstanceExists(instance.slug));
        }

        let config = r
            .config
            .as_ref()
            .ok_or(DeployError::Config(ConfigError::Validation(
                "missing config".into(),
            )))?
            .to_owned();
        let user_config = config_to_json(&config);
        let slug = slug::slugify(&r.name);
        let parsed_config = GeneratedInstanceConfig::try_from_config(config.clone(), &slug)
            .await?
            .merged_with_defaults()
            .to_owned();
        let model = db::instances::ActiveModel {
            creator_token_id: Set(creator.token.id),
            slug: Set(slug),
            user_config: Set(user_config),
            parsed_config: Set(parsed_config.raw),
            ..Default::default()
        }
        .insert(&tx)
        .await?;
        tx.commit().await.map_err(|e| anyhow::anyhow!(e))?;

        Ok(Instance { model })
    }

    pub fn config(&self) -> GeneratedInstanceConfig {
        GeneratedInstanceConfig::new(self.model.parsed_config.clone())
    }

    pub async fn commit(&self, github: &GithubClient) -> Result<(), DeployError> {
        let file_name = get_filename(&self.model.slug);
        let content = self.config().to_yaml()?;
        github.create_or_update_file(&file_name, &content).await?;

        Ok(())
    }
}

fn get_filename(slug: &str) -> String {
    format!("values-{}.yaml", slug)
}
