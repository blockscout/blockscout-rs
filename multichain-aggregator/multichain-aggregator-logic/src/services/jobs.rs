use std::{future::Future, sync::Arc};
use tokio::time::{Duration, Instant};
use tokio_cron_scheduler::{Job, JobSchedulerError};

pub fn create_repeated_job<F, Fut, E>(
    name: &'static str,
    interval: Duration,
    update_fn: F,
) -> Result<Job, JobSchedulerError>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), E>> + Send,
    E: std::fmt::Debug,
{
    let update_fn = Arc::new(update_fn);
    Job::new_repeated_async(interval, move |_uuid, _lock| {
        let update_fn = Arc::clone(&update_fn);
        Box::pin(async move {
            let now = Instant::now();
            if let Err(err) = update_fn().await {
                tracing::error!(err = ?err, "failed to update {name}");
            }
            let elapsed = now.elapsed();
            tracing::info!(elapsed_secs = elapsed.as_secs_f32(), "{name} updated");
        })
    })
}
