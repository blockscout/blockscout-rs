use anyhow::Context;
use bens_logic::subgraphs_reader::SubgraphReader;
use std::sync::Arc;
use tokio_cron_scheduler::Job;

pub fn refresh_cache_job(
    schedule: &str,
    subgraph_reader: Arc<SubgraphReader>,
) -> Result<Job, anyhow::Error> {
    let job = Job::new_async(schedule, move |_uuid, mut _l| {
        let reader = subgraph_reader.clone();
        Box::pin(async move {
            tracing::info!("refresh subgraph cache");
            let now = std::time::Instant::now();
            match reader.as_ref().refresh_cache().await {
                Ok(_) => {
                    tracing::info!(
                        elapsed_secs = now.elapsed().as_secs_f32(),
                        "updated subgraph successfully"
                    );
                }
                Err(err) => {
                    tracing::error!(err = ?err, "error during refreshing subgraph");
                }
            };
        })
    })
    .context("creating refresh cache job")?;

    Ok(job)
}
