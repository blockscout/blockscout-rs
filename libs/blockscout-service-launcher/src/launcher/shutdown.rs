use std::{future::Future, time::Duration};

use tokio::{task::JoinSet, time::error::Elapsed};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

/// Graceful shutdown according to https://tokio.rs/tokio/topics/shutdown
#[derive(Clone)]
pub struct GracefulShutdownHandler {
    pub shutdown_token: CancellationToken,
    pub task_tracker: TaskTracker,
}

impl GracefulShutdownHandler {
    pub fn new() -> Self {
        Self {
            shutdown_token: CancellationToken::new(),
            task_tracker: TaskTracker::new(),
        }
    }

    pub fn from_token(token: CancellationToken) -> Self {
        Self {
            shutdown_token: token,
            task_tracker: TaskTracker::new(),
        }
    }

    /// Close the tasks with shutdown token and wait for their completion
    /// with provided (or default) timeout (`duration`).
    pub async fn cancel_wait_timeout(&self, duration: Option<Duration>) -> Result<(), Elapsed> {
        self.shutdown_token.cancel();
        self.task_tracker.close();
        let duration = duration.unwrap_or(Duration::from_secs(15));
        tokio::time::timeout(duration, self.task_tracker.wait()).await
    }
}

/// `GracefulShutdownHandler` but with local task tracker always included.
/// See [`TaskTrackers`] for details
#[derive(Clone)]
pub struct LocalGracefulShutdownHandler {
    pub shutdown_token: CancellationToken,
    pub task_trackers: TaskTrackers,
}

impl From<GracefulShutdownHandler> for LocalGracefulShutdownHandler {
    fn from(value: GracefulShutdownHandler) -> Self {
        Self {
            shutdown_token: value.shutdown_token,
            task_trackers: TaskTrackers::new(value.task_tracker),
        }
    }
}

impl LocalGracefulShutdownHandler {
    pub async fn spawn_and_track<F>(
        &self,
        futures: &mut JoinSet<F::Output>,
        future: F,
    ) -> tokio::task::AbortHandle
    where
        F: Future,
        F: Send + 'static,
        F::Output: Send,
    {
        futures.spawn(
            self.task_trackers
                .local
                .track_future(self.task_trackers.external.track_future(future)),
        )
    }
}

/// * `local` - tracker for tasks created within this crate.
/// * `external` - tracker provided by some dependant crate,
///     so that it can track our tasks as well.
#[derive(Clone)]
pub(crate) struct TaskTrackers {
    // we don't use `JoinSet` here because we wish to
    // share this tracker with many tasks
    pub local: TaskTracker,
    pub external: TaskTracker,
}

impl TaskTrackers {
    pub fn new(external: TaskTracker) -> Self {
        Self {
            local: TaskTracker::new(),
            external,
        }
    }

    pub fn close(&self) {
        self.local.close();
        self.external.close();
    }

    /// Should be cancel-safe, just like `TaskTracker::wait()`
    pub async fn wait(&self) {
        self.local.wait().await;
        self.external.wait().await;
    }

    pub fn track_future<F>(&self, future: F) -> impl Future<Output = F::Output>
    where
        F: Future,
    {
        self.external.track_future(self.local.track_future(future))
    }
}
