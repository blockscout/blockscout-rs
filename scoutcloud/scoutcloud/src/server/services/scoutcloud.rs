use crate::{
    logic,
    logic::{users::AuthError, DeployError, GithubClient},
    server::proto::scoutcloud_server::Scoutcloud,
};
use convert_trait::TryConvert;
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    CreateInstanceRequest, CreateInstanceRequestInternal, CreateInstanceResponse, Deployment,
    GetCurrentDeploymentRequest, GetDeploymentRequest, GetInstanceRequest, Instance,
    ListDeploymentsRequest, ListDeploymentsResponse, ListInstancesRequest, ListInstancesResponse,
    UpdateConfigPartialRequest, UpdateConfigRequest, UpdateConfigResponse,
    UpdateInstanceStatusRequest, UpdateInstanceStatusResponse,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ScoutcloudService {
    db: Arc<DatabaseConnection>,
    github: Arc<GithubClient>,
}

impl ScoutcloudService {
    pub fn new(db: Arc<DatabaseConnection>, github: Arc<GithubClient>) -> Self {
        Self { db, github }
    }
}

#[async_trait::async_trait]
impl Scoutcloud for ScoutcloudService {
    async fn create_instance(
        &self,
        request: Request<CreateInstanceRequest>,
    ) -> Result<Response<CreateInstanceResponse>, Status> {
        let (meta, _, request) = request.into_parts();
        let user_token = logic::users::authenticate(self.db.as_ref(), &meta.into_headers())
            .await
            .map_err(map_auth_error)?;
        let request =
            CreateInstanceRequestInternal::try_convert(request).map_err(map_convert_error)?;
        let result = logic::deploy::create_instance(
            self.db.as_ref(),
            self.github.as_ref(),
            &request,
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
        _request: Request<UpdateConfigRequest>,
    ) -> Result<Response<UpdateConfigResponse>, Status> {
        todo!()
    }

    async fn update_config_partial(
        &self,
        _request: Request<UpdateConfigPartialRequest>,
    ) -> Result<Response<UpdateConfigResponse>, Status> {
        todo!()
    }

    async fn update_instance_status(
        &self,
        _request: Request<UpdateInstanceStatusRequest>,
    ) -> Result<Response<UpdateInstanceStatusResponse>, Status> {
        todo!()
    }

    async fn get_instance(
        &self,
        _request: Request<GetInstanceRequest>,
    ) -> Result<Response<Instance>, Status> {
        todo!()
    }

    async fn list_instances(
        &self,
        _request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        todo!()
    }

    async fn get_deployment(
        &self,
        _request: Request<GetDeploymentRequest>,
    ) -> Result<Response<Deployment>, Status> {
        todo!()
    }

    async fn get_current_deployment(
        &self,
        _request: Request<GetCurrentDeploymentRequest>,
    ) -> Result<Response<Deployment>, Status> {
        todo!()
    }

    async fn list_deployments(
        &self,
        _request: Request<ListDeploymentsRequest>,
    ) -> Result<Response<ListDeploymentsResponse>, Status> {
        todo!()
    }
}

fn map_convert_error(e: String) -> Status {
    Status::invalid_argument(e.to_string())
}

fn map_deploy_error(e: DeployError) -> Status {
    tracing::error!("deploy error: {:?}", e);
    match e {
        DeployError::Config(e) => Status::invalid_argument(e.to_string()),
        DeployError::InstanceExists(e) => Status::already_exists(e.to_string()),
        DeployError::Github(e) => Status::internal(e.to_string()),
        DeployError::Db(e) => Status::internal(e.to_string()),
        DeployError::Internal(e) => Status::internal(e.to_string()),
    }
}

fn map_auth_error(e: AuthError) -> Status {
    match e {
        AuthError::NoToken => Status::unauthenticated(e.to_string()),
        AuthError::TokenNotFound => Status::unauthenticated(e.to_string()),
        AuthError::Internal(e) => Status::internal(e.to_string()),
    }
}
