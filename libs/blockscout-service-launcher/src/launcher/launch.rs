use super::{
    metrics::Metrics,
    router::{configure_router, HttpRouter},
    settings::{MetricsSettings, ServerSettings},
    span_builder::CompactRootSpanBuilder,
    HttpServerSettings,
};
use actix_web::{middleware::Condition, App, HttpServer};
use actix_web_prom::PrometheusMetrics;
use std::{future::Future, net::SocketAddr, time::Duration};
use tokio::{task::JoinSet, time::timeout};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing_actix_web::TracingLogger;

pub(crate) const SHUTDOWN_TIMEOUT_SEC: u64 = 10;

pub struct LaunchSettings {
    pub service_name: String,
    pub server: ServerSettings,
    pub metrics: MetricsSettings,
}

#[derive(Clone)]
pub(crate) struct TaskTrackers {
    pub local: TaskTracker,
    pub external: Option<TaskTracker>,
}

impl TaskTrackers {
    pub fn new(external: Option<TaskTracker>) -> Self {
        Self {
            local: TaskTracker::new(),
            external,
        }
    }

    pub fn close(&self) {
        self.local.close();
        if let Some(t) = &self.external {
            t.close();
        }
    }

    /// Should be cancel-safe, just like `TaskTracker::wait()`
    pub async fn wait(&self) {
        self.local.wait().await;
        if let Some(t) = &self.external {
            t.wait().await;
        }
    }

    pub fn track_future<F>(&self, future: F) -> impl Future<Output = F::Output>
    where
        F: Future,
    {
        let future = self.local.track_future(future);
        if let Some(t) = &self.external {
            either::Left(t.track_future(future))
        } else {
            either::Right(future)
        }
    }
}

async fn spawn_and_track<F>(
    futures: &mut JoinSet<F::Output>,
    trackers: &TaskTrackers,
    future: F,
) -> tokio::task::AbortHandle
where
    F: Future,
    F: Send + 'static,
    F::Output: Send,
{
    if let Some(t) = &trackers.external {
        futures.spawn(trackers.local.track_future(t.track_future(future)))
    } else {
        futures.spawn(trackers.local.track_future(future))
    }
}

pub async fn launch<R>(
    settings: &LaunchSettings,
    http: R,
    grpc: tonic::transport::server::Router,
    shutdown: Option<CancellationToken>,
    task_tracker: Option<TaskTracker>,
) -> Result<(), anyhow::Error>
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    let metrics = settings
        .metrics
        .enabled
        .then(|| Metrics::new(&settings.service_name, &settings.metrics.route));

    let mut futures = JoinSet::new();
    let trackers = TaskTrackers::new(task_tracker);

    if settings.server.http.enabled {
        let http_server = http_serve(
            http,
            metrics
                .as_ref()
                .map(|metrics| metrics.http_middleware().clone()),
            &settings.server.http,
            shutdown.clone(),
            &trackers,
        );
        spawn_and_track(&mut futures, &trackers, async move {
            http_server.await.map_err(anyhow::Error::msg)
        })
        .await;
    }

    if settings.server.grpc.enabled {
        let grpc_server = grpc_serve(grpc, settings.server.grpc.addr, shutdown.clone());
        spawn_and_track(&mut futures, &trackers, async move {
            grpc_server.await.map_err(anyhow::Error::msg)
        })
        .await;
    }

    if let Some(metrics) = metrics {
        let addr = settings.metrics.addr;
        let shutdown = shutdown.clone();
        let trackers_ = trackers.clone();
        spawn_and_track(&mut futures, &trackers, async move {
            metrics.run_server(addr, shutdown, &trackers_).await?;
            Ok(())
        })
        .await;
    }
    if let Some(ref shutdown) = shutdown {
        let shutdown = shutdown.clone();
        spawn_and_track(&mut futures, &trackers, async move {
            shutdown.cancelled().await;
            Ok(())
        })
        .await;
    }

    let res = futures.join_next().await.expect("future set is not empty");
    trackers.close();
    if let Some(shutdown) = shutdown {
        shutdown.cancel();
        if timeout(Duration::from_secs(SHUTDOWN_TIMEOUT_SEC), trackers.wait())
            .await
            .is_err()
        {
            // timed out; fallback to simple task abort
            futures.abort_all();
        }
    } else {
        futures.abort_all();
    }
    trackers.wait().await;
    futures.join_all().await;
    res?
}

pub(crate) async fn stop_actix_server_on_cancel(
    actix_handle: actix_web::dev::ServerHandle,
    shutdown: CancellationToken,
    graceful: bool,
) {
    shutdown.cancelled().await;
    tracing::info!(
        "Shutting down actix server (gracefully: {graceful}).\
        Should finish within {SHUTDOWN_TIMEOUT_SEC} seconds..."
    );
    actix_handle.stop(graceful).await;
}

pub(crate) async fn grpc_cancel_signal(shutdown: CancellationToken) {
    shutdown.cancelled().await;
    tracing::info!("Shutting down grpc server...");
}

fn http_serve<R>(
    http: R,
    metrics: Option<PrometheusMetrics>,
    settings: &HttpServerSettings,
    shutdown: Option<CancellationToken>,
    task_trackers: &TaskTrackers,
) -> actix_web::dev::Server
where
    R: HttpRouter + Send + Sync + Clone + 'static,
{
    tracing::info!("starting http server on addr {}", settings.addr);

    // Initialize the tracing logger not to print http request and response messages on health endpoint
    CompactRootSpanBuilder::init_skip_http_trace_paths(["/health"]);

    let json_cfg = actix_web::web::JsonConfig::default().limit(settings.max_body_size);
    let cors_settings = settings.cors.clone();
    let cors_enabled = cors_settings.enabled;
    let server = if let Some(metrics) = metrics {
        HttpServer::new(move || {
            let cors = cors_settings.clone().build();
            App::new()
                .wrap(TracingLogger::<CompactRootSpanBuilder>::new())
                .wrap(metrics.clone())
                .wrap(Condition::new(cors_enabled, cors))
                .app_data(json_cfg.clone())
                .configure(configure_router(&http))
        })
        .shutdown_timeout(SHUTDOWN_TIMEOUT_SEC)
        .bind(settings.addr)
        .expect("failed to bind server")
        .run()
    } else {
        HttpServer::new(move || {
            let cors = cors_settings.clone().build();
            App::new()
                .wrap(TracingLogger::<CompactRootSpanBuilder>::new())
                .wrap(Condition::new(cors_enabled, cors))
                .app_data(json_cfg.clone())
                .configure(configure_router(&http))
        })
        .shutdown_timeout(SHUTDOWN_TIMEOUT_SEC)
        .bind(settings.addr)
        .expect("failed to bind server")
        .run()
    };
    if let Some(shutdown) = shutdown {
        tokio::spawn(task_trackers.track_future(stop_actix_server_on_cancel(
            server.handle(),
            shutdown,
            true,
        )));
    }
    server
}

async fn grpc_serve(
    grpc: tonic::transport::server::Router,
    addr: SocketAddr,
    shutdown: Option<CancellationToken>,
) -> Result<(), tonic::transport::Error> {
    tracing::info!("starting grpc server on addr {}", addr);
    if let Some(shutdown) = shutdown {
        grpc.serve_with_shutdown(addr, grpc_cancel_signal(shutdown))
            .await
    } else {
        grpc.serve(addr).await
    }
}
