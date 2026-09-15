// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{
    metrics as runner_metrics, CompilerExecutor, CompilerInvocation, ExecutionError,
    ExecutionOutput,
};
use crate::metrics::{self, GuardedGauge};
use async_trait::async_trait;
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct ConcurrencyLimitedCompilerExecutor {
    inner: Arc<dyn CompilerExecutor>,
    permits: Arc<Semaphore>,
    _capacity: Option<Arc<runner_metrics::CapacityGuard>>,
}

impl ConcurrencyLimitedCompilerExecutor {
    pub fn new(inner: Arc<dyn CompilerExecutor>, max_concurrent_jobs: NonZeroUsize) -> Self {
        let max_concurrent_jobs = max_concurrent_jobs.get();
        Self {
            inner,
            permits: Arc::new(Semaphore::new(max_concurrent_jobs)),
            _capacity: Some(Arc::new(runner_metrics::observe_capacity(
                max_concurrent_jobs,
            ))),
        }
    }

    /// Compatibility path for callers that already own the shared semaphore.
    pub fn with_semaphore(inner: Arc<dyn CompilerExecutor>, permits: Arc<Semaphore>) -> Self {
        Self {
            inner,
            permits,
            _capacity: None,
        }
    }
}

#[async_trait]
impl CompilerExecutor for ConcurrencyLimitedCompilerExecutor {
    async fn execute(
        &self,
        mut invocation: CompilerInvocation,
    ) -> Result<ExecutionOutput, ExecutionError> {
        let queue_cancellation =
            runner_metrics::QueueCancellationObservation::new(runner_metrics::SHARED);
        let runner_queue = runner_metrics::observe_job_state(runner_metrics::QUEUE);
        let runner_queue_timer = runner_metrics::start_operation(
            runner_metrics::SHARED,
            runner_metrics::QUEUE,
            "process",
        );
        let queue_gauge = metrics::COMPILATIONS_IN_QUEUE.guarded_inc();
        let queue_timer = metrics::COMPILATION_QUEUE_TIME.start_timer();
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| ExecutionError::Infrastructure(error.into()))?;
        drop(queue_timer);
        drop(queue_gauge);
        drop(runner_queue_timer);
        drop(runner_queue);
        queue_cancellation.admitted();

        invocation.set_admission_permit(permit);
        let _runner_active = runner_metrics::observe_job_state("running");
        let _active = metrics::COMPILATIONS_IN_FLIGHT.guarded_inc();
        let _timer = metrics::COMPILE_TIME.start_timer();
        self.inner.execute(invocation).await
    }

    async fn health_check(&self) -> Result<(), ExecutionError> {
        self.inner.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommandArgument, JobFile};
    use bytes::Bytes;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::{
        sync::Notify,
        time::{sleep, timeout, Duration},
    };

    struct BlockingExecutor {
        active: AtomicUsize,
        maximum: AtomicUsize,
        started: AtomicUsize,
        health_checks: AtomicUsize,
        release: Semaphore,
    }

    impl Default for BlockingExecutor {
        fn default() -> Self {
            Self {
                active: AtomicUsize::new(0),
                maximum: AtomicUsize::new(0),
                started: AtomicUsize::new(0),
                health_checks: AtomicUsize::new(0),
                release: Semaphore::new(0),
            }
        }
    }

    struct DetachedCleanupExecutor {
        started: AtomicUsize,
        cleanup_started: Arc<Notify>,
        cleanup_release: Arc<Semaphore>,
    }

    impl Default for DetachedCleanupExecutor {
        fn default() -> Self {
            Self {
                started: AtomicUsize::new(0),
                cleanup_started: Arc::new(Notify::new()),
                cleanup_release: Arc::new(Semaphore::new(0)),
            }
        }
    }

    struct DetachedCleanup {
        admission_permit: Option<Arc<tokio::sync::OwnedSemaphorePermit>>,
        started: Arc<Notify>,
        release: Arc<Semaphore>,
    }

    impl Drop for DetachedCleanup {
        fn drop(&mut self) {
            let admission_permit = self.admission_permit.take();
            let started = self.started.clone();
            let release = self.release.clone();
            tokio::spawn(async move {
                started.notify_one();
                let cleanup_permit = release.acquire().await.unwrap();
                cleanup_permit.forget();
                drop(admission_permit);
            });
        }
    }

    #[async_trait]
    impl CompilerExecutor for BlockingExecutor {
        async fn execute(
            &self,
            invocation: CompilerInvocation,
        ) -> Result<ExecutionOutput, ExecutionError> {
            let _admission_permit = invocation.admission_permit();
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            self.started.fetch_add(1, Ordering::SeqCst);
            let permit = self
                .release
                .acquire()
                .await
                .map_err(|error| ExecutionError::Infrastructure(error.into()))?;
            permit.forget();
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(ExecutionOutput {
                stdout: Bytes::new(),
                stderr: Bytes::new(),
                exit_code: 0,
                oom_killed: false,
            })
        }

        async fn health_check(&self) -> Result<(), ExecutionError> {
            self.health_checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl CompilerExecutor for DetachedCleanupExecutor {
        async fn execute(
            &self,
            invocation: CompilerInvocation,
        ) -> Result<ExecutionOutput, ExecutionError> {
            let invocation_number = self.started.fetch_add(1, Ordering::SeqCst);
            if invocation_number == 0 {
                let _cleanup = DetachedCleanup {
                    admission_permit: invocation.admission_permit(),
                    started: self.cleanup_started.clone(),
                    release: self.cleanup_release.clone(),
                };
                std::future::pending::<()>().await;
            }
            Ok(ExecutionOutput {
                stdout: Bytes::new(),
                stderr: Bytes::new(),
                exit_code: 0,
                oom_killed: false,
            })
        }
    }

    fn invocation(argument: &'static str) -> CompilerInvocation {
        CompilerInvocation::new(
            JobFile::bytes("compiler", "bin/compiler", Bytes::new()).unwrap(),
            vec![CommandArgument::literal(argument)],
            Bytes::new(),
        )
    }

    async fn wait_for_started(executor: &BlockingExecutor, expected: usize) {
        timeout(Duration::from_secs(1), async {
            while executor.started.load(Ordering::SeqCst) < expected {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("executor invocation should start");
    }

    #[tokio::test]
    async fn compilation_and_version_probe_share_one_fair_limit() {
        let inner = Arc::new(BlockingExecutor::default());
        let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::new(
            inner.clone(),
            NonZeroUsize::new(1).unwrap(),
        ));

        let compile = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--standard-json")).await }
        });
        wait_for_started(&inner, 1).await;

        let version_probe = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--version")).await }
        });
        sleep(Duration::from_millis(20)).await;
        assert_eq!(inner.started.load(Ordering::SeqCst), 1);

        inner.release.add_permits(1);
        compile.await.unwrap().unwrap();
        wait_for_started(&inner, 2).await;
        inner.release.add_permits(1);
        version_probe.await.unwrap().unwrap();

        assert_eq!(inner.maximum.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn queued_cancellation_is_counted_without_reaching_the_inner_executor() {
        let inner = Arc::new(BlockingExecutor::default());
        let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::new(
            inner.clone(),
            NonZeroUsize::new(1).unwrap(),
        ));

        let running = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--standard-json")).await }
        });
        wait_for_started(&inner, 1).await;

        let canceled_counter = crate::metrics::COMPILER_RUNNER_JOBS_TOTAL
            .with_label_values(&[runner_metrics::SHARED, "canceled"]);
        let before = canceled_counter.get();
        let queued = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--version")).await }
        });
        sleep(Duration::from_millis(20)).await;
        queued.abort();
        assert!(queued.await.unwrap_err().is_cancelled());

        assert_eq!(inner.started.load(Ordering::SeqCst), 1);
        assert_eq!(canceled_counter.get(), before + 1);

        inner.release.add_permits(1);
        running.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn health_check_bypasses_saturated_compiler_limit() {
        let inner = Arc::new(BlockingExecutor::default());
        let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::new(
            inner.clone(),
            NonZeroUsize::new(1).unwrap(),
        ));
        let running = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--standard-json")).await }
        });
        wait_for_started(&inner, 1).await;

        timeout(Duration::from_millis(100), executor.health_check())
            .await
            .expect("health check must not wait for a compiler permit")
            .unwrap();
        assert_eq!(inner.health_checks.load(Ordering::SeqCst), 1);

        inner.release.add_permits(1);
        running.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn admitted_permit_is_held_until_detached_cleanup_finishes() {
        let inner = Arc::new(DetachedCleanupExecutor::default());
        let executor = Arc::new(ConcurrencyLimitedCompilerExecutor::new(
            inner.clone(),
            NonZeroUsize::new(1).unwrap(),
        ));

        let first = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--standard-json")).await }
        });
        timeout(Duration::from_secs(1), async {
            while inner.started.load(Ordering::SeqCst) < 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let second = tokio::spawn({
            let executor = executor.clone();
            async move { executor.execute(invocation("--version")).await }
        });
        sleep(Duration::from_millis(20)).await;
        assert_eq!(inner.started.load(Ordering::SeqCst), 1);

        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        timeout(Duration::from_secs(1), inner.cleanup_started.notified())
            .await
            .unwrap();
        sleep(Duration::from_millis(20)).await;
        assert_eq!(inner.started.load(Ordering::SeqCst), 1);

        inner.cleanup_release.add_permits(1);
        second.await.unwrap().unwrap();
        assert_eq!(inner.started.load(Ordering::SeqCst), 2);
    }
}
