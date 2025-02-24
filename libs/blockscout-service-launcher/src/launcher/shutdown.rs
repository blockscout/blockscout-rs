use std::future::Future;

use tokio::task::JoinSet;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use super::launch::TaskTrackers;

/// Graceful shutdown according to https://tokio.rs/tokio/topics/shutdown
#[derive(Clone)]
pub struct GracefulShutdownHandler {
    pub shutdown_token: Option<CancellationToken>,
    pub task_tracker: Option<TaskTracker>,
}

impl GracefulShutdownHandler {
    /// No external tracking
    pub fn new_empty() -> Self {
        Self {
            shutdown_token: None,
            task_tracker: None,
        }
    }

    pub fn from_token(token: CancellationToken) -> Self {
        Self {
            shutdown_token: Some(token),
            task_tracker: None,
        }
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
            shutdown_token: value.shutdown_token.unwrap_or(CancellationToken::new()),
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
        if let Some(t) = &self.task_trackers.external {
            futures.spawn(
                self.task_trackers
                    .local
                    .track_future(t.track_future(future)),
            )
        } else {
            futures.spawn(self.task_trackers.local.track_future(future))
        }
    }
}
