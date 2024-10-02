use std::{path::PathBuf, sync::Arc, time::Duration};

use crate::{
    config::{read_charts_config, read_layout_config, read_update_groups_config},
    health::HealthService,
    read_service::ReadService,
    runtime_setup::RuntimeSetup,
    settings::{Settings, StartConditionSettings},
    update_service::UpdateService,
};

use anyhow::Context;
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::launcher::{self, LaunchSettings};
use reqwest::StatusCode;
use sea_orm::{ConnectOptions, Database};
use stats::metrics;
use stats_proto::blockscout::stats::v1::{
    health_actix::route_health,
    health_server::HealthServer,
    stats_service_actix::route_stats_service,
    stats_service_server::{StatsService, StatsServiceServer},
};
use tokio::time::sleep;
use tracing::{info, warn};

const SERVICE_NAME: &str = "stats";

#[derive(Clone)]
struct HttpRouter<S: StatsService> {
    stats: Arc<S>,
    health: Arc<HealthService>,
    swagger_path: PathBuf,
}

impl<S: StatsService> launcher::HttpRouter for HttpRouter<S> {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config
            .configure(|config| route_health(config, self.health.clone()))
            .configure(|config| route_stats_service(config, self.stats.clone()))
            .configure(|config| {
                route_swagger(
                    config,
                    self.swagger_path.clone(),
                    // it's ok to not have this endpoint in swagger, as it is
                    // the swagger itself
                    "/api/v1/docs/swagger.yaml",
                )
            });
    }
}

fn grpc_router<S: StatsService>(
    stats: Arc<S>,
    health: Arc<HealthService>,
) -> tonic::transport::server::Router {
    tonic::transport::Server::builder()
        .add_service(HealthServer::from_arc(health))
        .add_service(StatsServiceServer::from_arc(stats))
}

fn is_threshold_passed(
    threshold: Option<f32>,
    float_value: Option<String>,
    value_name: &str,
) -> Result<bool, anyhow::Error> {
    let Some(threshold) = threshold else {
        return Ok(true);
    };
    let value = float_value
        .map(|s| s.parse::<f64>())
        .transpose()
        .context(format!("Parsing `{value_name}`"))?;
    let Some(value) = value else {
        anyhow::bail!("Received `null` value of `{value_name}`. Can't determine indexing status.",);
    };
    if value < threshold.into() {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is not satisfied"
        );
        Ok(false)
    } else {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is satisfied"
        );
        Ok(true)
    }
}

fn is_retryable_code(status_code: &reqwest::StatusCode) -> bool {
    matches!(
        *status_code,
        StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::IM_A_TEAPOT
    )
}

async fn wait_for_blockscout_indexing(
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
) -> Result<(), anyhow::Error> {
    loop {
        match blockscout_client::apis::main_page_api::get_indexing_status(&api_config).await {
            Ok(result)
                if is_threshold_passed(
                    wait_config.blocks_ratio_threshold,
                    result.indexed_blocks_ratio.clone(),
                    "indexed_blocks_ratio",
                )? && is_threshold_passed(
                    wait_config.internal_transactions_ratio_threshold,
                    result.indexed_internal_transactions_ratio.clone(),
                    "indexed_internal_transactions_ratio",
                )? =>
            {
                info!("Blockscout indexing threshold passed");
                return Ok(());
            }
            Ok(_) => {}
            Err(blockscout_client::Error::ResponseError(r)) if is_retryable_code(&r.status) => {
                warn!("Error from indexing status endpoint: {r:?}");
            }
            Err(e) => {
                return Err(e).context("Requesting indexing status");
            }
        }
        info!(
            "Blockscout is not indexed enough. Checking again in {} secs",
            wait_config.check_period_secs
        );
        sleep(Duration::from_secs(wait_config.check_period_secs.into())).await;
    }
}

pub async fn stats(settings: Settings) -> Result<(), anyhow::Error> {
    blockscout_service_launcher::tracing::init_logs(
        SERVICE_NAME,
        &settings.tracing,
        &settings.jaeger,
    )?;
    let charts_config = read_charts_config(&settings.charts_config)?;
    let layout_config = read_layout_config(&settings.layout_config)?;
    let update_groups_config = read_update_groups_config(&settings.update_groups_config)?;
    let mut opt = ConnectOptions::new(settings.db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    blockscout_service_launcher::database::initialize_postgres::<stats::migration::Migrator>(
        opt.clone(),
        settings.create_database,
        settings.run_migrations,
    )
    .await?;
    let db = Arc::new(Database::connect(opt).await.context("stats DB")?);

    let mut opt = ConnectOptions::new(settings.blockscout_db_url.clone());
    opt.sqlx_logging_level(tracing::log::LevelFilter::Debug);
    // we'd like to have each batch to resolve in under 1 hour
    // as it seems to be the middleground between too many steps & occupying DB for too long
    opt.sqlx_slow_statements_logging_settings(
        tracing::log::LevelFilter::Warn,
        Duration::from_secs(3600),
    );
    let blockscout = Arc::new(Database::connect(opt).await.context("blockscout DB")?);

    let charts = Arc::new(RuntimeSetup::new(
        charts_config,
        layout_config,
        update_groups_config,
    )?);

    // TODO: maybe run this with migrations or have special config
    for group_entry in charts.update_groups.values() {
        group_entry
            .group
            .create_charts_with_mutexes(&db, None, &group_entry.enabled_members)
            .await?;
    }

    // Wait for blockscout to index, if necessary.
    if let Some(api_config) = settings.api_url.map(blockscout_client::Configuration::new) {
        wait_for_blockscout_indexing(api_config, settings.conditional_start).await?;
    }

    let update_service =
        Arc::new(UpdateService::new(db.clone(), blockscout, charts.clone()).await?);

    tokio::spawn(async move {
        update_service
            .force_async_update_and_run(
                settings.concurrent_start_updates,
                settings.default_schedule,
                settings.force_update_on_start,
            )
            .await;
    });

    if settings.metrics.enabled {
        metrics::initialize_metrics(charts.charts_info.keys().map(|f| f.as_str()));
    }

    let read_service = Arc::new(ReadService::new(db, charts, settings.limits.into()).await?);
    let health = Arc::new(HealthService::default());

    let grpc_router = grpc_router(read_service.clone(), health.clone());
    let http_router = HttpRouter {
        stats: read_service,
        health: health.clone(),
        swagger_path: settings.swagger_file,
    };

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
    };

    launcher::launch(&launch_settings, http_router, grpc_router).await
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use tokio::task::JoinSet;
    use tokio::time::error::Elapsed;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    async fn mock_indexing_status(response: ResponseTemplate) -> MockServer {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/main-page/indexing-status"))
            .respond_with(response)
            .mount(&mock_server)
            .await;
        mock_server
    }

    async fn test_wait_indexing(
        wait_config: StartConditionSettings,
        response: ResponseTemplate,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        let server = mock_indexing_status(response).await;
        tokio::time::timeout(
            Duration::from_millis(500),
            wait_for_blockscout_indexing(
                blockscout_client::Configuration::new(url::Url::from_str(&server.uri()).unwrap()),
                wait_config,
            ),
        )
        .await
    }

    #[tokio::test]
    async fn wait_for_blockscout_indexing_works_with_200_response() {
        let wait_config = StartConditionSettings {
            enabled: true,
            blocks_ratio_threshold: Some(0.9),
            internal_transactions_ratio_threshold: Some(0.9),
            check_period_secs: 0,
        };
        test_wait_indexing(
            wait_config.clone(),
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": true,
                    "finished_indexing_blocks": true,
                    "indexed_blocks_ratio": "1.00",
                    "indexed_internal_transactions_ratio": "1"
                }"#,
            ),
        )
        .await
        .expect("must not timeout")
        .expect("must not error");

        test_wait_indexing(
            wait_config,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "0.80",
                    "indexed_internal_transactions_ratio": "0.80"
                }"#,
            ),
        )
        .await
        .expect_err("must time out");
    }

    #[tokio::test]
    async fn wait_for_blockscout_indexing_works_with_slow_response() {
        let wait_config = StartConditionSettings {
            enabled: true,
            blocks_ratio_threshold: Some(0.9),
            internal_transactions_ratio_threshold: Some(0.9),
            check_period_secs: 0,
        };

        test_wait_indexing(
            wait_config,
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                        "finished_indexing": false,
                        "finished_indexing_blocks": false,
                        "indexed_blocks_ratio": "1.0",
                        "indexed_internal_transactions_ratio": "1.0"
                    }"#,
                )
                .set_delay(Duration::from_millis(100)),
        )
        .await
        .expect("must not timeout")
        .expect("must not error")
    }

    #[tokio::test]
    async fn wait_for_blockscout_indexing_works_with_infinite_timeout() {
        let wait_config = StartConditionSettings {
            enabled: true,
            blocks_ratio_threshold: Some(0.9),
            internal_transactions_ratio_threshold: Some(0.9),
            check_period_secs: 0,
        };

        test_wait_indexing(
            wait_config,
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                        "finished_indexing": false,
                        "finished_indexing_blocks": false,
                        "indexed_blocks_ratio": "0.80",
                        "indexed_internal_transactions_ratio": "0.80"
                    }"#,
                )
                .set_delay(Duration::MAX),
        )
        .await
        .expect_err("must time out");
    }

    #[tokio::test]
    async fn wait_for_blockscout_indexing_retries_with_error_codes() {
        let wait_config = StartConditionSettings {
            enabled: true,
            blocks_ratio_threshold: Some(0.9),
            internal_transactions_ratio_threshold: Some(0.9),
            check_period_secs: 0,
        };

        let mut error_servers = JoinSet::from_iter([
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(429)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(500)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(503)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(504)),
        ]);
        #[allow(for_loops_over_fallibles)]
        for server in error_servers.join_next().await {
            let test_result = server.unwrap();
            test_result.expect_err("must time out");
        }
    }

    #[tokio::test]
    async fn wait_for_blockscout_indexing_fails_with_error_codes() {
        let wait_config = StartConditionSettings {
            enabled: true,
            blocks_ratio_threshold: Some(0.9),
            internal_transactions_ratio_threshold: Some(0.9),
            check_period_secs: 0,
        };

        let mut error_servers = JoinSet::from_iter([
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(400)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(403)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(404)),
            test_wait_indexing(wait_config.clone(), ResponseTemplate::new(405)),
        ]);
        #[allow(for_loops_over_fallibles)]
        for server in error_servers.join_next().await {
            let test_result = server.unwrap();
            test_result
                .expect("must fail immediately")
                .expect_err("must report error");
        }
    }
}
