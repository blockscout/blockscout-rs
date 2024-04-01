use crate::logic::{users::UserToken, DeployError, GithubClient, Instance};
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    CreateInstanceRequestInternal, CreateInstanceResponseInternal,
};
use sea_orm::DatabaseConnection;

pub async fn create_instance(
    db: &DatabaseConnection,
    github: &GithubClient,
    request: &CreateInstanceRequestInternal,
    creator: &UserToken,
) -> Result<CreateInstanceResponseInternal, DeployError> {
    let instance = Instance::try_create(db, request, creator).await?;
    instance.commit(github).await?;

    Ok(CreateInstanceResponseInternal {
        instance_id: instance.model.external_id.to_string(),
    })
}
