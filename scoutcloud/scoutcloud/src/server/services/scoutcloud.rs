use crate::{
    logic,
    logic::{
        jobs::JobsRunner,
        users::{AuthError, UserToken},
        ConfigError, DeployError, GithubClient,
    },
    server::proto::{scoutcloud_server::Scoutcloud, *},
};
use convert_trait::TryConvert;

use sea_orm::{ConnectionTrait, DatabaseConnection};
use std::sync::Arc;

use tonic::{Request, Response, Status};

pub struct ScoutcloudService {
    db: Arc<DatabaseConnection>,
    github: Arc<GithubClient>,
    jobs: Arc<JobsRunner>,
}

impl ScoutcloudService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        github: Arc<GithubClient>,
        jobs: Arc<JobsRunner>,
    ) -> Self {
        Self { db, github, jobs }
    }
}

macro_rules! get_config {
    ($request:expr) => {
        $request
            .config
            .as_ref()
            .ok_or(ConfigError::MissingConfig)
            .map_err(DeployError::Config)
            .map_err(map_deploy_error)
    };
}

#[async_trait::async_trait]
impl Scoutcloud for ScoutcloudService {
    async fn create_instance(
        &self,
        request: Request<CreateInstanceRequest>,
    ) -> Result<Response<CreateInstanceResponse>, Status> {
        let (request, user_token): (CreateInstanceRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let config = get_config!(&request)?;
        let result = logic::deploy::create_instance(
            self.db.as_ref(),
            self.github.as_ref(),
            &request.name,
            config,
            &user_token,
        )
        .await
        .map_err(map_deploy_error)?;
        Ok(Response::new(
            CreateInstanceResponse::try_convert(result).map_err(map_convert_error)?,
        ))
    }

    async fn update_config(
        &self,
        request: Request<UpdateConfigRequest>,
    ) -> Result<Response<UpdateConfigResponse>, Status> {
        let (request, user_token): (UpdateConfigRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let config = get_config!(&request)?;
        let updated_config = logic::deploy::update_instance_config(
            self.db.as_ref(),
            self.github.as_ref(),
            &request.instance_id,
            config,
            &user_token,
        )
        .await
        .map_err(map_deploy_error)?;
        let result = UpdateConfigResponseInternal {
            config: Some(updated_config.internal),
        };
        Ok(Response::new(
            UpdateConfigResponse::try_convert(result).map_err(map_convert_error)?,
        ))
    }

    async fn update_config_partial(
        &self,
        request: Request<UpdateConfigPartialRequest>,
    ) -> Result<Response<UpdateConfigResponse>, Status> {
        let (request, user_token): (UpdateConfigPartialRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let config = get_config!(&request)?;
        let updated_config = logic::deploy::update_instance_config_partial(
            self.db.as_ref(),
            self.github.as_ref(),
            &request.instance_id,
            config,
            &user_token,
        )
        .await
        .map_err(map_deploy_error)?;
        let result = UpdateConfigResponseInternal {
            config: Some(updated_config.internal),
        };
        Ok(Response::new(
            UpdateConfigResponse::try_convert(result).map_err(map_convert_error)?,
        ))
    }

    async fn update_instance_status(
        &self,
        request: Request<UpdateInstanceStatusRequest>,
    ) -> Result<Response<UpdateInstanceStatusResponse>, Status> {
        let (request, user_token): (UpdateInstanceStatusRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;

        let result = logic::deploy::update_instance_status(
            self.db.as_ref(),
            self.github.as_ref(),
            self.jobs.as_ref(),
            &request.instance_id,
            &request.action,
            &user_token,
        )
        .await
        .map_err(map_deploy_error)?;

        Ok(Response::new(
            UpdateInstanceStatusResponse::try_convert(result).map_err(map_convert_error)?,
        ))
    }

    async fn get_instance(
        &self,
        request: Request<GetInstanceRequest>,
    ) -> Result<Response<Instance>, Status> {
        let (request, user_token): (GetInstanceRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let internal =
            logic::deploy::get_instance(self.db.as_ref(), &request.instance_id, &user_token)
                .await
                .map_err(map_deploy_error)?;
        let result = Instance::try_convert(internal).map_err(map_convert_error)?;
        Ok(Response::new(result))
    }

    async fn list_instances(
        &self,
        request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        let (_, user_token): (ListInstancesRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let items = logic::deploy::list_instances(self.db.as_ref(), &user_token)
            .await
            .map_err(map_deploy_error)?;

        items
            .into_iter()
            .map(|internal| Instance::try_convert(internal).map_err(map_convert_error))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| ListInstancesResponse { items })
            .map(Response::new)
    }

    async fn get_deployment(
        &self,
        request: Request<GetDeploymentRequest>,
    ) -> Result<Response<Deployment>, Status> {
        let (request, user_token): (GetDeploymentRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let internal =
            logic::deploy::get_deployment(self.db.as_ref(), &request.deployment_id, &user_token)
                .await
                .map_err(map_deploy_error)?;
        let result = Deployment::try_convert(internal).map_err(map_convert_error)?;
        Ok(Response::new(result))
    }

    async fn get_current_deployment(
        &self,
        request: Request<GetCurrentDeploymentRequest>,
    ) -> Result<Response<Deployment>, Status> {
        let (request, user_token): (GetCurrentDeploymentRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let internal = logic::deploy::get_current_deployment(
            self.db.as_ref(),
            &request.instance_id,
            &user_token,
        )
        .await
        .map_err(map_deploy_error)?;
        let result = Deployment::try_convert(internal).map_err(map_convert_error)?;
        Ok(Response::new(result))
    }

    async fn list_deployments(
        &self,
        request: Request<ListDeploymentsRequest>,
    ) -> Result<Response<ListDeploymentsResponse>, Status> {
        let (request, user_token): (ListDeploymentsRequestInternal, _) =
            parse_request_with_headers(self.db.as_ref(), request).await?;
        let items =
            logic::deploy::list_deployments(self.db.as_ref(), &request.instance_id, &user_token)
                .await
                .map_err(map_deploy_error)?;

        items
            .into_iter()
            .map(|internal| Deployment::try_convert(internal).map_err(map_convert_error))
            .collect::<Result<Vec<_>, _>>()
            .map(|items| ListDeploymentsResponse { items })
            .map(Response::new)
    }
}

async fn parse_request_with_headers<C, B, I>(
    db: &C,
    request: Request<B>,
) -> Result<(I, UserToken), Status>
where
    C: ConnectionTrait,
    I: TryConvert<B>,
{
    let (meta, _, request) = request.into_parts();
    let user_token = UserToken::try_from_http_headers(db, &meta.into_headers())
        .await
        .map_err(map_auth_error)?;
    let request = I::try_convert(request).map_err(map_convert_error)?;
    Ok((request, user_token))
}

fn map_convert_error(e: String) -> Status {
    Status::invalid_argument(e.to_string())
}

fn map_deploy_error(err: DeployError) -> Status {
    tracing::error!("deploy error: {:?}", err);
    match err {
        DeployError::InstanceExists(_) => Status::already_exists(err.to_string()),
        DeployError::InstanceNotFound(_) => Status::not_found(err.to_string()),
        DeployError::Config(e) => Status::invalid_argument(e.to_string()),
        DeployError::Github(e) => Status::internal(e.to_string()),
        DeployError::Db(e) => Status::internal(e.to_string()),
        DeployError::Internal(e) => Status::internal(e.to_string()),
        DeployError::Auth(e) => map_auth_error(e),
        DeployError::DeploymentNotFound => Status::not_found(err.to_string()),
        DeployError::InvalidStateTransition(_, _) => Status::invalid_argument(err.to_string()),
        DeployError::GithubWorkflow(_) => Status::internal(err.to_string()),
    }
}

fn map_auth_error(err: AuthError) -> Status {
    match err {
        AuthError::NoToken => Status::unauthenticated(err.to_string()),
        AuthError::TokenNotFound => Status::unauthenticated(err.to_string()),
        AuthError::Internal(e) => Status::internal(e.to_string()),
        AuthError::NotFound => Status::not_found(err.to_string()),
        AuthError::Unauthorized(_) => Status::permission_denied(err.to_string()),
        AuthError::Db(e) => Status::internal(e.to_string()),
        AuthError::InsufficientBalance => Status::permission_denied(err.to_string()),
    }
}
