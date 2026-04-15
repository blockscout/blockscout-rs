use anyhow::Context;
use bens_logic::subgraph::SubgraphReader;
use std::sync::Arc;
use tokio_cron_scheduler::Job;

pub fn refresh_cache_job(
    schedule: &str,
    subgraph_reader: Arc<SubgraphReader>,
) -> Result<Job, anyhow::Error> {
    let job = Job::new_async(schedule, move |_uuid, mut _l| {
        let reader = subgraph_reader.clone();
        Box::pin(async move {
            let pool = reader.pg_pool_write();
            tracing::info!(
                target: "bens.refresh_cache",
                pool_size = pool.size(),
                pool_num_idle = pool.num_idle(),
                pool_max_connections = pool.options().get_max_connections(),
                "refresh subgraph cache: starting (sqlx pool snapshot)"
            );
            let now = std::time::Instant::now();
            match reader.as_ref().refresh_cache().await {
                Ok(_) => {
                    tracing::info!(
                        target: "bens.refresh_cache",
                        elapsed_secs = now.elapsed().as_secs_f32(),
                        pool_size = pool.size(),
                        pool_num_idle = pool.num_idle(),
                        "updated subgraph successfully"
                    );
                }
                Err(err) => {
                    tracing::error!(
                        target: "bens.refresh_cache",
                        err = ?err,
                        pool_size = pool.size(),
                        pool_num_idle = pool.num_idle(),
                        "error during refreshing subgraph"
                    );
                }
            };
        })
    })
    .context("creating refresh cache job")?;

    Ok(job)
}
