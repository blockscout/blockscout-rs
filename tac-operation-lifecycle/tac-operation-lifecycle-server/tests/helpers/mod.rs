use std::sync::Arc;

use blockscout_service_launcher::{test_database::TestDbGuard, test_server};
use futures::StreamExt;
use reqwest::Url;
use tac_operation_lifecycle_logic::{client::settings::RpcSettings, database::TacDatabase};
use tac_operation_lifecycle_server::Settings;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    let db_name = format!("testdb_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}
pub async fn init_tac_operation_lifecycle_server<F>(
    db_url: String,
    test_name: &str,
    settings_setup: F,
    realtime_boundary: u64,
) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(db_url.clone());
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings_setup(settings), base)
    };

    let test_db = init_db(test_name).await;
    let db = Arc::new(TacDatabase::new(test_db.client()));

    test_server::init_server(
        move || {
            tac_operation_lifecycle_server::run(settings, db.clone(), realtime_boundary.clone())
        },
        &base,
    )
    .await;
    base
}

#[cfg(test)]
mod tests {
    use futures::stream::select_all;
    use rand::Rng;
    use std::{sync::Arc, time};

    use super::*;
    use migration::sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use sea_orm::Database;
    use tac_operation_lifecycle_entity::interval;
    use tac_operation_lifecycle_logic::{
        client::Client, database::OrderDirection, settings::IndexerSettings, Indexer, IndexerJob,
    };
    use tokio::sync::Mutex;
    use tracing::Instrument;

    #[tokio::test]
    async fn test_save_intervals() {
        let db = init_db("save_intervals").await;
        let conn_with_db = Database::connect(&db.db_url()).await.unwrap();

        let catchup_interval = time::Duration::from_secs(rand::thread_rng().gen_range(1..100));
        let tasks_number = rand::thread_rng().gen_range(1..100);
        let lag = tasks_number * catchup_interval.as_secs();
        let current_epoch = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_timestamp = current_epoch - lag;
        tracing::debug!("start_timestamp: {}", start_timestamp);
        tracing::debug!("catchup_interval: {}", catchup_interval.as_secs());
        tracing::debug!("tasks_number: {}", tasks_number);
        tracing::debug!("current_epoch: {}", current_epoch);
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
        let indexer = Indexer::new(
            indexer_settings,
            Arc::new(TacDatabase::new(Arc::new(conn_with_db))),
        )
        .await
        .unwrap();
        let intervals_number = indexer
            .generate_historical_intervals(current_epoch)
            .await
            .unwrap();
        let intervals = interval::Entity::find()
            .all(db.client().as_ref())
            .await
            .unwrap();
        assert_eq!(intervals_number, tasks_number as usize);
        assert_eq!(intervals.len(), tasks_number as usize);
        for i in 0..tasks_number as usize {
            let index = i as u64;
            assert_eq!(
                intervals[i].start as u64,
                start_timestamp + index * catchup_interval.as_secs()
            );
            assert_eq!(
                intervals[i].end as u64,
                start_timestamp + (index + 1) * catchup_interval.as_secs()
            );
        }

        assert_eq!(indexer.watermark().await.unwrap(), current_epoch);
    }

    #[tokio::test]
    async fn test_job_stream() {
        use futures::stream::{select_with_strategy, PollNext};
        use std::collections::HashSet;

        // Define our own strategy function
        fn prio_left(_: &mut ()) -> PollNext {
            PollNext::Left
        }

        // Initialize test database and create indexer with test settings
        let db = init_db("test_job_stream").await;
        let conn_with_db = Database::connect(&db.db_url()).await.unwrap();
        let catchup_interval = time::Duration::from_secs(10); // Use fixed interval for predictable testing
        let tasks_number = 5; // Use fixed number of tasks for predictable testing
        let current_epoch = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start_timestamp = current_epoch - tasks_number * catchup_interval.as_secs();

        let indexer_settings = IndexerSettings {
            concurrency: 1,
            catchup_interval,
            start_timestamp,
            ..Default::default()
        };

        let indexer = Indexer::new(
            indexer_settings,
            Arc::new(TacDatabase::new(Arc::new(conn_with_db))),
        )
        .await
        .unwrap();

        // Save intervals first
        indexer
            .generate_historical_intervals(current_epoch)
            .await
            .unwrap();

        // Create prioritized streams like in the actual implementation
        let high_priority = indexer.interval_stream(OrderDirection::LatestFirst, None, None);
        let low_priority = indexer.interval_stream(OrderDirection::EarliestFirst, None, None);
        let mut combined_stream = select_with_strategy(high_priority, low_priority, prio_left);

        // Collect jobs from the prioritized stream
        let mut received_jobs = Vec::new();
        let mut seen_intervals = HashSet::new();

        // We'll collect jobs for a short time to get the initial batch
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(1));
        tokio::pin!(timeout);

        let mut all_jobs_received = false;
        while !all_jobs_received {
            tokio::select! {
                Some(job) = combined_stream.next() => {
                    match job {
                        IndexerJob::Interval(interval_job) => {
                            let thread_id = std::thread::current().id();
                            tracing::debug!("Thread {:?} Received interval job: {:?}", thread_id, interval_job);
                            // Ensure we haven't seen this interval before
                            let interval_key = (interval_job.interval.start, interval_job.interval.end);
                            let (start, end) = interval_key;
                            assert!(!seen_intervals.contains(&interval_key), "Received duplicate interval: {:?}", interval_key);
                            seen_intervals.insert(interval_key);

                            // Verify job timestamps are within expected range
                            assert!(start >= start_timestamp as i64, "Job start time {} is before start_timestamp {}", start, start_timestamp);
                            assert!(end <= current_epoch as i64, "Job end time {} is after current_epoch {}", end, current_epoch);

                            // Verify job interval matches catchup_interval
                            assert_eq!(end - start, catchup_interval.as_secs() as i64,
                                "Job interval {:?} doesn't match catchup_interval {}",
                                (end - start), catchup_interval.as_secs());

                            // After each job, verify its interval is marked as in-progress
                            let intervals = interval::Entity::find()
                                .filter(interval::Column::Start.eq(start))
                                .filter(interval::Column::End.eq(end))
                                .one(db.client().as_ref())
                                .await.unwrap();

                            if let Some(interval) = intervals {
                                assert_eq!(interval.status, 1,
                                    "Interval with start={}, end={} not marked as in-progress",
                                    start, end);
                            } else {
                                panic!("Could not find interval for job {:?}", interval_job);
                            }

                            received_jobs.push(interval_job);
                            tracing::debug!("Received {} jobs", received_jobs.len());

                            if received_jobs.len() >= tasks_number as usize {
                                tracing::debug!("all jobs received");
                                all_jobs_received = true;
                            }
                        },
                        IndexerJob::Operation(_) | IndexerJob::Realtime => {
                            // Skip operation jobs in this test as we're only testing intervals
                            continue;
                        }
                    }
                }
                _ = &mut timeout => {
                    break;
                }
            }
        }

        tracing::debug!("--------------------------------");
        tracing::debug!("Received {} jobs", received_jobs.len());
        tracing::debug!("all_jobs_received: {}", all_jobs_received);
        tracing::debug!("--------------------------------");

        // Verify we received all expected jobs
        assert_eq!(
            received_jobs.len(),
            tasks_number as usize,
            "Did not receive expected number of jobs. Got {}, expected {}",
            received_jobs.len(),
            tasks_number
        );

        // TODO:instead of checking each consecutive pair, we could verify that all jobs
        // from the high-priority range are processed before any jobs from the low-priority range

        // for i in 1..received_jobs.len() {
        //     assert!(received_jobs[i-1].interval.start > received_jobs[i].interval.start,
        //         "Jobs not in descending order: {:?} before {:?}",
        //         received_jobs[i-1], received_jobs[i]);
        // }
    }

    #[tokio::test]
    async fn test_operation_lifecycle_indexing() {
        use serde_json::json;
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        // Initialize test database and mock server
        let db = init_db("indexing").await;
        let conn_with_db = Database::connect(&db.db_url()).await.unwrap();
        let mock_server = MockServer::start().await;

        let _server = init_tac_operation_lifecycle_server(
            db.db_url(),
            "indexing",
            |mut settings| {
                settings.tracing.enabled = true;
                settings
            },
            0,
        )
        .await;
        // Set up the mock for /operationIds endpoint
        Mock::given(method("GET"))
            .and(path("/operation-ids"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "response": {
                        "operations": [
                            {
                                "operationId": "0x33e2ee58e3e8d48f064915a062adb02dcc062c0533fb429c7f703ba3e1fe33fb",
                                "timestamp": 1741794238
                            }
                        ],
                        "total": 1
                    }
                })))
            .mount(&mock_server)
            .await;

        // Set up the mock for /stage-profiling endpoint
        Mock::given(method("GET"))
            .and(path("/stage-profiling"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!({
                    "response": {
                        "0x33e2ee58e3e8d48f064915a062adb02dcc062c0533fb429c7f703ba3e1fe33fb": {
                            "operationType": "TON-TAC-TON",
                            "collectedInTAC": {
                                "exists": true,
                                "stageData": {
                                    "success": true,
                                    "timestamp": 1741794238,
                                    "transactions": [
                                        {
                                            "hash": "0xe169b9b9bffe366fdf08e035338d6d7b676ccde970f4bc0880d6ff7702337240",
                                            "blockchainType": "TON"
                                        },
                                        {
                                            "hash": "0xcc22ec9451cf4e0927561b38baf757831a6cdbba6ea7ba38fcf50e375926d6d1",
                                            "blockchainType": "TON"
                                        }
                                    ],
                                    "note": null
                                }
                            },
                            "includedInTACConsensus": {
                                "exists": true,
                                "stageData": {
                                    "success": true,
                                    "timestamp": 1741794247,
                                    "transactions": [
                                        {
                                            "hash": "0x064d10f4f972317d5f2e8927e71a58963383f74cc4067ba401e3234672c1cef2",
                                            "blockchainType": "TAC"
                                        }
                                    ],
                                    "note": null
                                }
                            }
                        }
                    }
                })))
            .mount(&mock_server)
            .await;

        // Set up indexer with specific start timestamp
        let start_timestamp = 1741794237; // Just before the operation timestamp
        let catchup_interval = time::Duration::from_secs(10);
        let jobs_number = 3;
        let current_epoch = start_timestamp + catchup_interval.as_secs() * jobs_number as u64;

        let mock_rpc_settings = RpcSettings {
            url: mock_server.uri(),
            ..Default::default()
        };

        let client = Arc::new(Mutex::new(Client::new(mock_rpc_settings)));
        let indexer_settings = IndexerSettings {
            concurrency: 1,
            catchup_interval,
            start_timestamp,

            ..Default::default()
        };

        let indexer = Indexer::new(
            indexer_settings,
            Arc::new(TacDatabase::new(Arc::new(conn_with_db))),
        )
        .await
        .unwrap();

        // Save intervals and start indexing
        let intervals_number = indexer
            .generate_historical_intervals(current_epoch)
            .await
            .unwrap();
        assert_eq!(intervals_number, jobs_number as usize);

        // Get the stream of jobs
        let interval_stream = indexer.interval_stream(OrderDirection::EarliestFirst, None, None);

        let operations_stream = indexer.operations_stream();

        let mut job_stream = select_all(vec![interval_stream, operations_stream]);

        // Process the stream and verify the sequence of events
        let mut operation_id_fetched = false;
        let mut stage_history_fetched = false;
        let mut interval_processed = false;

        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
        tokio::pin!(timeout);

        while !operation_id_fetched || !stage_history_fetched || !interval_processed {
            tokio::select! {
                Some(job) = job_stream.next() => {
                    match job {
                        IndexerJob::Interval(interval_job) => {
                            // Process the interval job
                            tracing::warn!("Processing interval job: {:?}", interval_job);
                            if let Err(e) = indexer.fetch_operations(&interval_job, client.clone()).instrument(tracing::info_span!(
                                "fetching operations",
                                interval_id = interval_job.interval.id,
                                start = interval_job.interval.start,
                                end = interval_job.interval.end,
                            )).await {
                                panic!("Failed to fetch operations: {} ", e);
                            }

                            // Verify interval contains our target timestamp
                            if interval_job.interval.start <= 1741794238 && interval_job.interval.end >= 1741794238 {
                                // Verify interval is marked as processed
                                let interval = interval::Entity::find()
                                    .filter(interval::Column::Start.eq(interval_job.interval.start))
                                    .filter(interval::Column::End.eq(interval_job.interval.end))
                                    .one(db.client().as_ref())
                                    .await
                                    .unwrap()
                                    .unwrap();

                                assert_eq!(interval.status, 2, "Interval should be marked as processed");
                                interval_processed = true;
                            }
                        },
                        IndexerJob::Operation(operation_job) => {
                            // Process the operation job
                            indexer.process_operation_with_retries(vec![&operation_job], client.clone()).await;

                            // Verify operation ID matches our mock
                            assert_eq!(
                                operation_job.operation.id,
                                "0x33e2ee58e3e8d48f064915a062adb02dcc062c0533fb429c7f703ba3e1fe33fb",
                                "Unexpected operation ID"
                            );
                            operation_id_fetched = true;
                            stage_history_fetched = true;
                        }
                        IndexerJob::Realtime => {
                            // Skip realtime jobs in this test as we're only testing operations
                            continue;
                        }
                    }

                }
                _ = &mut timeout => {
                    panic!("Test timed out before all conditions were met");
                }
            }
        }

        // Final assertions
        assert!(operation_id_fetched, "Operation ID was not fetched");
        assert!(stage_history_fetched, "Stage history was not fetched");
        assert!(interval_processed, "Interval was not processed");
    }
}
