use stylus_verifier_proto::grpc::health::v1::{
    health_check_response, health_server::Health, HealthCheckRequest, HealthCheckResponse,
};

#[derive(Default)]
pub struct HealthService {}

#[async_trait::async_trait]
impl Health for HealthService {
    async fn check(
        &self,
        _request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(HealthCheckResponse {
            status: health_check_response::ServingStatus::Serving.into(),
        }))
    }
}
