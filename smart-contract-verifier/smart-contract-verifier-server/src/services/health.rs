// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::proto::{
    health_check_response, health_server::Health, HealthCheckRequest, HealthCheckResponse,
};
use actix_web::{http::StatusCode, web, HttpResponse};
use serde::Deserialize;
use smart_contract_verifier::CompilerExecutor;
use std::sync::Arc;

pub const COMPILER_HEALTH_SERVICE: &str = "compiler";

#[derive(Default)]
pub struct HealthService {
    compiler_executor: Option<Arc<dyn CompilerExecutor>>,
}

impl HealthService {
    pub fn new(compiler_executor: Option<Arc<dyn CompilerExecutor>>) -> Self {
        Self { compiler_executor }
    }

    fn response(status: health_check_response::ServingStatus) -> HealthCheckResponse {
        HealthCheckResponse {
            status: status as i32,
        }
    }

    async fn check_service(&self, service: &str) -> Result<HealthCheckResponse, tonic::Status> {
        if service.is_empty() {
            return Ok(Self::response(
                health_check_response::ServingStatus::Serving,
            ));
        }
        if service != COMPILER_HEALTH_SERVICE {
            return Err(tonic::Status::not_found("unknown health service"));
        }

        let Some(executor) = &self.compiler_executor else {
            return Ok(Self::response(
                health_check_response::ServingStatus::Serving,
            ));
        };
        Ok(match executor.health_check().await {
            Ok(()) => Self::response(health_check_response::ServingStatus::Serving),
            Err(error) => {
                tracing::warn!(error = ?error, "compiler executor health check failed");
                Self::response(health_check_response::ServingStatus::NotServing)
            }
        })
    }
}

#[derive(Deserialize)]
struct HealthQuery {
    #[serde(default)]
    service: String,
}

async fn http_health(
    health: web::Data<HealthService>,
    query: web::Query<HealthQuery>,
) -> HttpResponse {
    let response = match health.check_service(&query.service).await {
        Ok(response) => response,
        Err(status) if status.code() == tonic::Code::NotFound => {
            return HttpResponse::NotFound().finish();
        }
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    let status = if response.status == health_check_response::ServingStatus::Serving as i32 {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    HttpResponse::build(status).json(response)
}

pub(crate) fn route_health(config: &mut web::ServiceConfig, health: Arc<HealthService>) {
    config
        .app_data(web::Data::from(health))
        .route("/health", web::get().to(http_health));
}

#[async_trait::async_trait]
impl Health for HealthService {
    async fn check(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(
            self.check_service(&request.get_ref().service).await?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};
    use async_trait::async_trait;
    use smart_contract_verifier::{CompilerInvocation, ExecutionError, ExecutionOutput};

    struct UnhealthyExecutor;

    #[async_trait]
    impl CompilerExecutor for UnhealthyExecutor {
        async fn execute(
            &self,
            _invocation: CompilerInvocation,
        ) -> Result<ExecutionOutput, ExecutionError> {
            unreachable!("health tests must not execute a compiler")
        }

        async fn health_check(&self) -> Result<(), ExecutionError> {
            Err(ExecutionError::Infrastructure(anyhow::anyhow!(
                "sensitive SSH failure"
            )))
        }
    }

    #[tokio::test]
    async fn liveness_does_not_depend_on_compiler_executor() {
        let health = HealthService::new(Some(Arc::new(UnhealthyExecutor)));
        let response = health
            .check(tonic::Request::new(HealthCheckRequest {
                service: String::new(),
            }))
            .await
            .unwrap();

        assert_eq!(
            response.into_inner().status,
            health_check_response::ServingStatus::Serving as i32
        );
    }

    #[tokio::test]
    async fn grpc_compiler_readiness_reports_not_serving() {
        let health = HealthService::new(Some(Arc::new(UnhealthyExecutor)));
        let response = health
            .check(tonic::Request::new(HealthCheckRequest {
                service: COMPILER_HEALTH_SERVICE.to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(
            response.into_inner().status,
            health_check_response::ServingStatus::NotServing as i32
        );
    }

    #[actix_web::test]
    async fn http_liveness_stays_up_while_compiler_readiness_is_unavailable() {
        let health = Arc::new(HealthService::new(Some(Arc::new(UnhealthyExecutor))));
        let app =
            test::init_service(App::new().configure(|config| route_health(config, health.clone())))
                .await;
        let liveness_request = test::TestRequest::get().uri("/health").to_request();
        let liveness_response = test::call_service(&app, liveness_request).await;
        assert_eq!(liveness_response.status(), StatusCode::OK);

        let request = test::TestRequest::get()
            .uri("/health?service=compiler")
            .to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn compiler_readiness_is_serving_when_no_compiler_endpoint_is_enabled() {
        let response = HealthService::default()
            .check(tonic::Request::new(HealthCheckRequest {
                service: COMPILER_HEALTH_SERVICE.to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(
            response.into_inner().status,
            health_check_response::ServingStatus::Serving as i32
        );
    }

    #[actix_web::test]
    async fn unknown_health_service_does_not_mask_a_probe_typo() {
        let health = Arc::new(HealthService::default());
        let grpc_error = health
            .check(tonic::Request::new(HealthCheckRequest {
                service: "compilers".to_string(),
            }))
            .await
            .unwrap_err();
        assert_eq!(grpc_error.code(), tonic::Code::NotFound);

        let app =
            test::init_service(App::new().configure(|config| route_health(config, health.clone())))
                .await;
        let request = test::TestRequest::get()
            .uri("/health?service=compilers")
            .to_request();
        let response = test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
