// SPDX-License-Identifier: LicenseRef-Blockscout

use prometheus::{
    core::Collector, Gauge, Histogram, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec,
    Opts, Registry,
};
use std::sync::OnceLock;

const OPERATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0,
];

/// Every metric this crate exports. Their names share one prefix chosen by the service, so the
/// same runner reports as `smart_contract_verifier_*` in one service and under another name in
/// the next.
pub(crate) struct Collectors {
    pub(crate) compile_time: Histogram,
    pub(crate) compilations_in_flight: Gauge,
    pub(crate) compilation_queue_time: Histogram,
    pub(crate) compilations_in_queue: Gauge,
    pub(crate) jobs_total: IntCounterVec,
    pub(crate) operation_duration: HistogramVec,
    pub(crate) transfer_bytes: IntCounterVec,
    pub(crate) active_containers: IntGaugeVec,
    pub(crate) max_concurrent_jobs: IntGaugeVec,
    pub(crate) jobs_current: IntGaugeVec,
    pub(crate) docker_connect_failures_total: IntCounterVec,
    pub(crate) orphan_cleanup_sweeps_total: IntCounterVec,
    pub(crate) orphan_cleanup_containers_total: IntCounterVec,
}

impl Collectors {
    fn new(namespace: &str) -> prometheus::Result<Self> {
        let opts = |name: &str, help: &str| Opts::new(name, help).namespace(namespace);
        let histogram_opts =
            |name: &str, help: &str| HistogramOpts::new(name, help).namespace(namespace);
        Ok(Self {
            compile_time: Histogram::with_opts(histogram_opts(
                "compile_time_seconds",
                "compiler invocation time in seconds, including compiler version probes",
            ))?,
            compilations_in_flight: Gauge::with_opts(opts(
                "compiles_in_flight",
                "number of compiler invocations currently running",
            ))?,
            compilation_queue_time: Histogram::with_opts(histogram_opts(
                "compilation_queue_time_seconds",
                "time waiting for the shared compiler invocation limit in seconds",
            ))?,
            compilations_in_queue: Gauge::with_opts(opts(
                "compiles_in_queue",
                "number of compiler invocations waiting for the shared limit",
            ))?,
            jobs_total: IntCounterVec::new(
                opts(
                    "compiler_runner_jobs_total",
                    "compiler runner jobs classified by terminal outcome",
                ),
                &["executor", "outcome"],
            )?,
            operation_duration: HistogramVec::new(
                histogram_opts(
                    "compiler_runner_operation_duration_seconds",
                    "time spent in compiler runner operations, including failed and canceled operations",
                )
                .buckets(OPERATION_BUCKETS.to_vec()),
                &["executor", "operation", "family"],
            )?,
            transfer_bytes: IntCounterVec::new(
                opts(
                    "compiler_runner_transfer_bytes_total",
                    "bytes successfully transferred across compiler executor boundaries",
                ),
                &["executor", "direction", "family"],
            )?,
            active_containers: IntGaugeVec::new(
                opts(
                    "compiler_runner_active_containers",
                    "Docker container lifecycles currently managed by this verifier process",
                ),
                &["family"],
            )?,
            max_concurrent_jobs: IntGaugeVec::new(
                opts(
                    "compiler_runner_max_concurrent_jobs",
                    "configured compiler runner concurrency; sum across replicas gives runner slot capacity",
                ),
                &["executor"],
            )?,
            jobs_current: IntGaugeVec::new(
                opts(
                    "compiler_runner_jobs_current",
                    "compiler runner jobs currently queued or holding a runner concurrency slot",
                ),
                &["executor", "state"],
            )?,
            docker_connect_failures_total: IntCounterVec::new(
                opts(
                    "compiler_runner_docker_connect_failures_total",
                    "Docker requests whose SSH session could not be established, by outcome",
                ),
                &["outcome"],
            )?,
            orphan_cleanup_sweeps_total: IntCounterVec::new(
                opts(
                    "compiler_runner_orphan_cleanup_sweeps_total",
                    "compiler container orphan-cleanup sweeps by trigger and outcome",
                ),
                &["trigger", "outcome"],
            )?,
            orphan_cleanup_containers_total: IntCounterVec::new(
                opts(
                    "compiler_runner_orphan_cleanup_containers_total",
                    "compiler containers observed by orphan cleanup, classified by family and outcome",
                ),
                &["family", "outcome"],
            )?,
        })
    }

    fn all(&self) -> Vec<Box<dyn Collector>> {
        vec![
            Box::new(self.compile_time.clone()),
            Box::new(self.compilations_in_flight.clone()),
            Box::new(self.compilation_queue_time.clone()),
            Box::new(self.compilations_in_queue.clone()),
            Box::new(self.jobs_total.clone()),
            Box::new(self.operation_duration.clone()),
            Box::new(self.transfer_bytes.clone()),
            Box::new(self.active_containers.clone()),
            Box::new(self.max_concurrent_jobs.clone()),
            Box::new(self.jobs_current.clone()),
            Box::new(self.docker_connect_failures_total.clone()),
            Box::new(self.orphan_cleanup_sweeps_total.clone()),
            Box::new(self.orphan_cleanup_containers_total.clone()),
        ]
    }

    fn register(&self, registry: &Registry) -> prometheus::Result<()> {
        self.all()
            .into_iter()
            .try_for_each(|collector| registry.register(collector))
    }
}

/// The namespace the metrics were registered under, and the metrics themselves.
static COLLECTORS: OnceLock<(String, Collectors)> = OnceLock::new();

fn registered(namespace: &str, collectors: Collectors) -> (String, Collectors) {
    collectors
        .register(prometheus::default_registry())
        .expect("compiler runner metrics are registered once per process");
    (namespace.to_string(), collectors)
}

/// The crate's metrics, registered without a prefix unless [`set_metrics_namespace`] ran first.
pub(crate) fn collectors() -> &'static Collectors {
    let (_, collectors) = COLLECTORS.get_or_init(|| {
        let collectors = Collectors::new("").expect("metric definitions are valid");
        registered("", collectors)
    });
    collectors
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsNamespaceError {
    #[error("invalid metrics namespace {namespace:?}: {source}")]
    Invalid {
        namespace: String,
        source: prometheus::Error,
    },
    #[error(
        "compiler runner metrics are already registered under {registered:?}; \
         set the namespace before the first executor is built"
    )]
    AlreadyRegistered { registered: String },
}

/// Prefixes every metric of this crate with `namespace`, e.g. `smart_contract_verifier`.
///
/// Metrics are registered on first use and keep the prefix they were registered with, so call
/// this before building the first executor. Repeating the call with the same namespace is fine.
pub fn set_metrics_namespace(namespace: &str) -> Result<(), MetricsNamespaceError> {
    let collectors =
        Collectors::new(namespace).map_err(|source| MetricsNamespaceError::Invalid {
            namespace: namespace.to_string(),
            source,
        })?;
    let (registered_namespace, _) = COLLECTORS.get_or_init(|| registered(namespace, collectors));
    if registered_namespace == namespace {
        Ok(())
    } else {
        Err(MetricsNamespaceError::AlreadyRegistered {
            registered: registered_namespace.clone(),
        })
    }
}

pub struct GaugeGuard(&'static Gauge);

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.0.dec();
    }
}

pub trait GuardedGauge {
    fn guarded_inc(&'static self) -> GaugeGuard;
}

impl GuardedGauge for Gauge {
    fn guarded_inc(&'static self) -> GaugeGuard {
        self.inc();
        GaugeGuard(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(namespace: &str) -> Vec<String> {
        let mut names: Vec<String> = Collectors::new(namespace)
            .unwrap()
            .all()
            .iter()
            .flat_map(|collector| collector.desc())
            .map(|desc| desc.fq_name.clone())
            .collect();
        names.sort();
        names
    }

    /// The names the smart-contract-verifier has always published; dashboards and alerts use them.
    #[test]
    fn smart_contract_verifier_metric_names_are_stable() {
        assert_eq!(
            names("smart_contract_verifier"),
            [
                "smart_contract_verifier_compilation_queue_time_seconds",
                "smart_contract_verifier_compile_time_seconds",
                "smart_contract_verifier_compiler_runner_active_containers",
                "smart_contract_verifier_compiler_runner_docker_connect_failures_total",
                "smart_contract_verifier_compiler_runner_jobs_current",
                "smart_contract_verifier_compiler_runner_jobs_total",
                "smart_contract_verifier_compiler_runner_max_concurrent_jobs",
                "smart_contract_verifier_compiler_runner_operation_duration_seconds",
                "smart_contract_verifier_compiler_runner_orphan_cleanup_containers_total",
                "smart_contract_verifier_compiler_runner_orphan_cleanup_sweeps_total",
                "smart_contract_verifier_compiler_runner_transfer_bytes_total",
                "smart_contract_verifier_compiles_in_flight",
                "smart_contract_verifier_compiles_in_queue",
            ]
        );
    }

    #[test]
    fn namespace_prefixes_every_metric() {
        assert!(names("another_service")
            .iter()
            .all(|name| name.starts_with("another_service_")));
        assert!(names("").iter().all(|name| !name.starts_with('_')));
    }

    #[test]
    fn invalid_namespace_is_rejected() {
        let error = set_metrics_namespace("not a metric name").unwrap_err();
        assert!(matches!(error, MetricsNamespaceError::Invalid { .. }));
    }
}
