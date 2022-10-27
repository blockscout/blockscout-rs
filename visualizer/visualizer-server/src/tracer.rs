use opentelemetry::{
    global::{self},
    sdk::{self, propagation::TraceContextPropagator},
    trace::TraceError,
};
use tracing_subscriber::{filter::LevelFilter, layer::SubscriberExt, prelude::*};

use crate::settings::JaegerSettings;

pub fn init_logs(jaeger_settings: JaegerSettings) {
    let stdout = tracing_subscriber::fmt::layer().with_filter(
        tracing_subscriber::EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy(),
    );
    let registry = tracing_subscriber::registry()
        // output logs (tracing) to stdout with log level taken from env (default is INFO)
        .with(stdout);
    if jaeger_settings.enabled {
        let tracer =
            init_jaeger_tracer(&jaeger_settings.agent_endpoint).expect("failed to init tracer");
        registry
            // output traces to jaeger with default log level (default is DEBUG)
            .with(
                tracing_opentelemetry::layer()
                    .with_tracer(tracer)
                    .with_filter(LevelFilter::DEBUG),
            )
            .try_init()
    } else {
        registry.try_init()
    }
    .expect("failed to register tracer with registry");
}

pub fn init_jaeger_tracer(endpoint: &str) -> Result<sdk::trace::Tracer, TraceError> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name("visualizer")
        .with_endpoint(endpoint)
        .with_auto_split_batch(true)
        .install_batch(opentelemetry::runtime::Tokio)
}
