// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{ExecutionError, ExecutionOutput};
use crate::metrics::{
    COMPILER_RUNNER_ACTIVE_CONTAINERS, COMPILER_RUNNER_JOBS_CURRENT, COMPILER_RUNNER_JOBS_TOTAL,
    COMPILER_RUNNER_MAX_CONCURRENT_JOBS, COMPILER_RUNNER_OPERATION_DURATION,
    COMPILER_RUNNER_ORPHAN_CLEANUP_CONTAINERS_TOTAL, COMPILER_RUNNER_ORPHAN_CLEANUP_SWEEPS_TOTAL,
    COMPILER_RUNNER_TRANSFER_BYTES,
};
use prometheus::{HistogramTimer, IntGauge};

pub(super) const NATIVE: &str = "native";
pub(super) const DOCKER: &str = "docker";
pub(super) const SHARED: &str = "shared";
pub(super) const INPUT: &str = "input";
pub(super) const OUTPUT: &str = "output";
pub(super) const QUEUE: &str = "queue";
pub(super) const PREPARE: &str = "prepare";
pub(super) const CREATE: &str = "create";
pub(super) const UPLOAD: &str = "upload";
pub(super) const EXECUTE: &str = "execute";
pub(super) const CLEANUP: &str = "cleanup";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContainerFamily {
    Job,
    CacheProbe,
    CacheInitializer,
}

impl ContainerFamily {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Job => "job",
            Self::CacheProbe => "cache_probe",
            Self::CacheInitializer => "cache_initializer",
        }
    }
}

pub(super) fn start_operation(
    executor: &'static str,
    operation: &'static str,
    family: &'static str,
) -> HistogramTimer {
    COMPILER_RUNNER_OPERATION_DURATION
        .with_label_values(&[executor, operation, family])
        .start_timer()
}

pub(super) fn add_transfer_bytes(
    executor: &'static str,
    direction: &'static str,
    family: &'static str,
    bytes: usize,
) {
    COMPILER_RUNNER_TRANSFER_BYTES
        .with_label_values(&[executor, direction, family])
        .inc_by(bytes as u64);
}

pub(super) fn observe_capacity(max_concurrent_jobs: usize) -> CapacityGuard {
    let capacity = COMPILER_RUNNER_MAX_CONCURRENT_JOBS.with_label_values(&[SHARED]);
    let slots = i64::try_from(max_concurrent_jobs).unwrap_or(i64::MAX);
    for state in [QUEUE, "running"] {
        COMPILER_RUNNER_JOBS_CURRENT.with_label_values(&[SHARED, state]);
    }
    for family in [
        ContainerFamily::Job,
        ContainerFamily::CacheProbe,
        ContainerFamily::CacheInitializer,
    ] {
        COMPILER_RUNNER_ACTIVE_CONTAINERS.with_label_values(&[family.as_str()]);
    }
    CapacityGuard::new(capacity, slots)
}

pub(super) struct CapacityGuard {
    gauge: IntGauge,
    slots: i64,
}

impl CapacityGuard {
    fn new(gauge: IntGauge, slots: i64) -> Self {
        gauge.add(slots);
        Self { gauge, slots }
    }
}

impl Drop for CapacityGuard {
    fn drop(&mut self) {
        self.gauge.sub(self.slots);
    }
}

pub(super) struct GaugeGuard {
    gauge: IntGauge,
}

impl GaugeGuard {
    fn new(gauge: IntGauge) -> Self {
        gauge.inc();
        Self { gauge }
    }
}

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.gauge.dec();
    }
}

pub(super) fn observe_job_state(state: &'static str) -> GaugeGuard {
    GaugeGuard::new(COMPILER_RUNNER_JOBS_CURRENT.with_label_values(&[SHARED, state]))
}

pub(super) fn observe_container(family: ContainerFamily) -> GaugeGuard {
    GaugeGuard::new(COMPILER_RUNNER_ACTIVE_CONTAINERS.with_label_values(&[family.as_str()]))
}

pub(super) struct JobObservation {
    executor: &'static str,
    completed: bool,
    _total_timer: HistogramTimer,
}

pub(super) struct QueueCancellationObservation {
    executor: &'static str,
    admitted: bool,
}

impl QueueCancellationObservation {
    pub(super) fn new(executor: &'static str) -> Self {
        Self {
            executor,
            admitted: false,
        }
    }

    pub(super) fn admitted(mut self) {
        self.admitted = true;
    }
}

impl Drop for QueueCancellationObservation {
    fn drop(&mut self) {
        if !self.admitted {
            COMPILER_RUNNER_JOBS_TOTAL
                .with_label_values(&[self.executor, "canceled"])
                .inc();
        }
    }
}

impl JobObservation {
    pub(super) fn new(executor: &'static str, family: &'static str) -> Self {
        Self {
            executor,
            completed: false,
            _total_timer: start_operation(executor, "total", family),
        }
    }

    pub(super) fn finish(mut self, result: &Result<ExecutionOutput, ExecutionError>) {
        COMPILER_RUNNER_JOBS_TOTAL
            .with_label_values(&[self.executor, classify_outcome(result)])
            .inc();
        self.completed = true;
    }
}

impl Drop for JobObservation {
    fn drop(&mut self) {
        if !self.completed {
            COMPILER_RUNNER_JOBS_TOTAL
                .with_label_values(&[self.executor, "canceled"])
                .inc();
        }
    }
}

fn classify_outcome(result: &Result<ExecutionOutput, ExecutionError>) -> &'static str {
    match result {
        Ok(output) if output.oom_killed => "oom_killed",
        Ok(output) if output.exit_code == 0 => "success",
        Ok(_) => "compiler_failure",
        Err(ExecutionError::InvalidInvocation(_)) => "invalid_invocation",
        Err(ExecutionError::UploadLimitExceeded { .. }) => "upload_limit",
        Err(ExecutionError::OutputLimitExceeded { .. }) => "output_limit",
        Err(ExecutionError::Timeout { .. }) => "timeout",
        Err(ExecutionError::CompilerFailed {
            oom_killed: true, ..
        }) => "oom_killed",
        Err(ExecutionError::CompilerFailed { .. }) => "compiler_failure",
        Err(ExecutionError::Infrastructure(_)) => "infrastructure_error",
    }
}

pub(super) fn count_orphan_sweep(trigger: &'static str, succeeded: bool) {
    COMPILER_RUNNER_ORPHAN_CLEANUP_SWEEPS_TOTAL
        .with_label_values(&[trigger, if succeeded { "success" } else { "error" }])
        .inc();
}

pub(super) fn count_orphan_container(family: &str, outcome: &'static str) {
    COMPILER_RUNNER_ORPHAN_CLEANUP_CONTAINERS_TOTAL
        .with_label_values(&[family, outcome])
        .inc();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn output(exit_code: i64, oom_killed: bool) -> Result<ExecutionOutput, ExecutionError> {
        Ok(ExecutionOutput {
            stdout: Bytes::new(),
            stderr: Bytes::new(),
            exit_code,
            oom_killed,
        })
    }

    #[test]
    fn classifies_all_terminal_outcomes() {
        let cases = [
            (output(0, false), "success"),
            (output(1, false), "compiler_failure"),
            (output(137, true), "oom_killed"),
            (
                Err(ExecutionError::InvalidInvocation("bad".to_string())),
                "invalid_invocation",
            ),
            (
                Err(ExecutionError::UploadLimitExceeded { limit: 1 }),
                "upload_limit",
            ),
            (
                Err(ExecutionError::OutputLimitExceeded { limit: 1 }),
                "output_limit",
            ),
            (Err(ExecutionError::Timeout { seconds: 1 }), "timeout"),
            (
                Err(ExecutionError::CompilerFailed {
                    compiler: "compiler".to_string(),
                    exit_code: 1,
                    stderr: String::new(),
                    oom_killed: false,
                }),
                "compiler_failure",
            ),
            (
                Err(ExecutionError::CompilerFailed {
                    compiler: "compiler".to_string(),
                    exit_code: 137,
                    stderr: String::new(),
                    oom_killed: true,
                }),
                "oom_killed",
            ),
            (
                Err(ExecutionError::Infrastructure(anyhow::anyhow!("failed"))),
                "infrastructure_error",
            ),
        ];

        for (result, expected) in cases {
            assert_eq!(classify_outcome(&result), expected);
        }
    }

    #[test]
    fn gauge_guard_balances_on_drop() {
        let gauge = IntGauge::new("compiler_runner_test_gauge", "test gauge").unwrap();
        {
            let _guard = GaugeGuard::new(gauge.clone());
            assert_eq!(gauge.get(), 1);
        }
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn capacity_guards_are_additive_and_balance_on_drop() {
        let gauge = IntGauge::new("compiler_runner_test_capacity", "test capacity").unwrap();
        let first = CapacityGuard::new(gauge.clone(), 2);
        let second = CapacityGuard::new(gauge.clone(), 3);
        assert_eq!(gauge.get(), 5);
        drop(first);
        assert_eq!(gauge.get(), 3);
        drop(second);
        assert_eq!(gauge.get(), 0);
    }

    #[test]
    fn job_observation_records_cancellation_once() {
        let counter = COMPILER_RUNNER_JOBS_TOTAL.with_label_values(&["test", "canceled"]);
        let before = counter.get();
        drop(JobObservation::new("test", "process"));
        assert_eq!(counter.get(), before + 1);
    }

    #[test]
    fn queue_observation_only_records_pre_admission_cancellation() {
        let counter = COMPILER_RUNNER_JOBS_TOTAL.with_label_values(&["test_queue", "canceled"]);
        let before = counter.get();
        drop(QueueCancellationObservation::new("test_queue"));
        assert_eq!(counter.get(), before + 1);

        QueueCancellationObservation::new("test_queue").admitted();
        assert_eq!(counter.get(), before + 1);
    }

    #[test]
    fn operation_timer_observes_when_canceled() {
        let histogram =
            COMPILER_RUNNER_OPERATION_DURATION.with_label_values(&["test", "queue", "process"]);
        let before = histogram.get_sample_count();
        drop(start_operation("test", "queue", "process"));
        assert_eq!(histogram.get_sample_count(), before + 1);
    }
}
