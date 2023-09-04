use super::{JaegerSettings, TracingFormat, TracingSettings};
use opentelemetry::{
    global::{self},
    sdk::{self, propagation::TraceContextPropagator},
    trace::TraceError,
};
use std::marker::Send;
use tracing_subscriber::{
    filter::LevelFilter, fmt::format::FmtSpan, layer::SubscriberExt, prelude::*, Layer, Registry,
};

pub fn init_logs(
    service_name: &str,
    tracing_settings: &TracingSettings,
    jaeger_settings: &JaegerSettings,
) -> Result<(), anyhow::Error> {
    // If tracing is disabled, there is nothing to initialize
    if !tracing_settings.enabled {
        return Ok(());
    }

    let stdout: Box<(dyn Layer<Registry> + Sync + Send + 'static)> = match tracing_settings.format {
        TracingFormat::Default => Box::new(
            tracing_subscriber::fmt::layer()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_filter(
                    tracing_subscriber::EnvFilter::builder()
                        .with_default_directive(LevelFilter::INFO.into())
                        .from_env_lossy(),
                ),
        ),
        TracingFormat::Json => Box::new(
            tracing_subscriber::fmt::layer()
                .json()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_filter(
                    tracing_subscriber::EnvFilter::builder()
                        .with_default_directive(LevelFilter::INFO.into())
                        .from_env_lossy(),
                ),
        ),
    };

    let registry = tracing_subscriber::registry()
        // output logs (tracing) to stdout with log level taken from env (default is INFO)
        .with(stdout);
    if jaeger_settings.enabled {
        let tracer = init_jaeger_tracer(service_name, &jaeger_settings.agent_endpoint)?;
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
    }?;
    Ok(())
}

pub fn init_jaeger_tracer(
    service_name: &str,
    endpoint: &str,
) -> Result<sdk::trace::Tracer, TraceError> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name(service_name)
        .with_endpoint(endpoint)
        .with_auto_split_batch(true)
        .install_batch(opentelemetry::runtime::Tokio)
}
