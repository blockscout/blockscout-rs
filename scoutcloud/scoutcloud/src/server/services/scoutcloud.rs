use crate::server::proto::scoutcloud_server::Scoutcloud;
use scoutcloud_proto::blockscout::scoutcloud::v1::{
    CreateInstanceRequest, CreateInstanceResponse, Deployment, GetCurrentDeploymentRequest,
    GetDeploymentRequest, GetInstanceRequest, Instance, ListDeploymentsRequest,
    ListDeploymentsResponse, ListInstancesRequest, ListInstancesResponse,
    UpdateConfigPartialRequest, UpdateConfigRequest, UpdateConfigResponse,
    UpdateInstanceStatusRequest, UpdateInstanceStatusResponse,
};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct ScoutcloudService {}

#[async_trait::async_trait]
impl Scoutcloud for ScoutcloudService {
    async fn create_instance(
        &self,
        _request: Request<CreateInstanceRequest>,
    ) -> Result<Response<CreateInstanceResponse>, Status> {
        todo!()
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
