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
            let result = update_fn().await;
            let elapsed_secs = now.elapsed().as_secs_f32();
            if let Err(err) = result {
                tracing::error!(elapsed_secs = elapsed_secs, err = ?err, "failed to update {name}");
            } else {
                tracing::info!(elapsed_secs = elapsed_secs, "{name} updated");
            }
        })
    })
}
