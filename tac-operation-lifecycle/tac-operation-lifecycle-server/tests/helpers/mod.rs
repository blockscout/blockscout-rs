use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server
};
use futures::StreamExt;
use reqwest::Url;
use tac_operation_lifecycle_server::Settings;

pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}
pub async fn init_tac_operation_lifecycle_server<F>(
    db_url: String,
    settings_setup: F
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(
            db_url
            );
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    test_server::init_server(|| tac_operation_lifecycle_server::run(settings), &base).await;
    base
}


#[cfg(test)]
mod tests {
    use std::time;
    use rand::Rng;

    use tac_operation_lifecycle_logic::{settings::IndexerSettings, Indexer};
    use tac_operation_lifecycle_entity::interval;
    use migration::sea_orm::EntityTrait;
    use super::*;
    
    #[tokio::test]
    async fn test_save_intervals() {
        let db = init_db("test_save_intervals", "test").await;
        let catchup_interval = time::Duration::from_secs(rand::thread_rng().gen_range(1..100));
        let tasks_number = rand::thread_rng().gen_range(1..100);
        let lag = tasks_number * catchup_interval.as_secs();
        let current_epoch = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
        let start_timestamp =  current_epoch - lag;
        println!("start_timestamp: {}", start_timestamp);
        println!("catchup_interval: {}", catchup_interval.as_secs());
        println!("tasks_number: {}", tasks_number); 
        println!("current_epoch: {}", current_epoch);
        let indexer_settings = IndexerSettings {
            concurrency: 1,
            //random catchup interval from 1 to 100
            catchup_interval,
            // current epoch - random from 1 to 100 times catchup interval
            start_timestamp,
            ..Default::default()
        };
        // let settings = Settings::default("postgres://postgres:postgres@database:5432/blockscout".to_string());
        // let server = init_tac_operation_lifecycle_server(db.db_url(), |settings| settings).await;
        let indexer = Indexer::new(indexer_settings, db.client()).await.unwrap();
        indexer.save_intervals().await.unwrap();
        let intervals = interval::Entity::find()
            .all(db.client().as_ref())
            .await.unwrap();
        assert_eq!(intervals.len(), tasks_number as usize);
        for i in 0..tasks_number as usize {
            let index = i as u64;
            assert_eq!(intervals[i].start as u64, start_timestamp + index * catchup_interval.as_secs());
            assert_eq!(intervals[i].end as u64, start_timestamp + (index + 1) * catchup_interval.as_secs());
        }

        assert_eq!(indexer.watermark(), current_epoch);
    }

    #[tokio::test]
    async fn test_poll_for_new_jobs() {
        use std::collections::HashSet;

        // Initialize test database and create indexer with test settings
        let db = init_db("test_poll_jobs", "test").await;
        let catchup_interval = time::Duration::from_secs(10); // Use fixed interval for predictable testing
        let tasks_number = 5; // Use fixed number of tasks for predictable testing
        let current_epoch = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_secs();
        let start_timestamp = current_epoch - tasks_number * catchup_interval.as_secs();

        let indexer_settings = IndexerSettings {
            concurrency: 1,
            catchup_interval,
            start_timestamp,
            ..Default::default()
        };

        let indexer = Indexer::new(indexer_settings, db.client()).await.unwrap();
        
        // Save intervals first
        indexer.save_intervals().await.unwrap();

        // Get the stream of jobs
        let mut job_stream = indexer.poll_for_new_jobs();
        
        // Collect all jobs from the stream (we'll break after getting expected number)
        let mut received_jobs = Vec::new();
        let mut seen_intervals = HashSet::new();

        // We'll collect jobs for a short time to get the initial batch
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(1));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                Some(job) = job_stream.next() => {
                    // Ensure we haven't seen this interval before
                    let interval_key = (job.start, job.end);
                    assert!(!seen_intervals.contains(&interval_key), "Received duplicate interval: {:?}", interval_key);
                    seen_intervals.insert(interval_key);
                    
                    // Verify job timestamps are within expected range
                    assert!(job.start >= start_timestamp, "Job start time {} is before start_timestamp {}", job.start, start_timestamp);
                    assert!(job.end <= current_epoch, "Job end time {} is after current_epoch {}", job.end, current_epoch);
                    
                    // Verify job interval matches catchup_interval
                    assert_eq!(job.end - job.start, catchup_interval.as_secs(), 
                        "Job interval {:?} doesn't match catchup_interval {}", 
                        (job.end - job.start), catchup_interval.as_secs());

                    received_jobs.push(job);

                    if received_jobs.len() >= tasks_number as usize {
                        break;
                    }
                }
                _ = &mut timeout => {
                    break;
                }
            }
        }

        // Verify we received all expected jobs
        assert_eq!(received_jobs.len(), tasks_number as usize, 
            "Did not receive expected number of jobs. Got {}, expected {}", 
            received_jobs.len(), tasks_number);

        // Verify jobs are in descending order by start time
        for i in 1..received_jobs.len() {
            assert!(received_jobs[i-1].start > received_jobs[i].start, 
                "Jobs not in descending order: {:?} before {:?}", 
                received_jobs[i-1], received_jobs[i]);
        }

        // Verify all intervals are now marked as in-progress
        let intervals = interval::Entity::find()
            .all(db.client().as_ref())
            .await.unwrap();

        
        for interval in intervals {
            println!("interval: {:?}", interval);
            assert_eq!(interval.status, 1, "Interval {} not marked as in-progress", interval.id);
        }
    }
}
