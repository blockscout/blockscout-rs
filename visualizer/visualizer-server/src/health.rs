use crate::proto::blockscout::visualizer::v1::{
    health_check_response, health_server::Health, HealthCheckRequest, HealthCheckResponse,
};

pub use crate::proto::blockscout::visualizer::v1::health_actix::route_health;

#[derive(Default)]
pub struct HealthService {}

#[async_trait::async_trait]
impl Health for HealthService {
    async fn check(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(HealthCheckResponse {
            status: health_check_response::ServingStatus::Serving as i32,
        }))
    }
}
