use crate::logic::{github::MockedGithubRepo, jobs::JobsRunner, GithubClient};
use blockscout_service_launcher::test_database::TestDbGuard;
use fang::SleepParams;
use std::{sync::Arc, time::Duration};

pub async fn test_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}

pub async fn test_github_client() -> (GithubClient, MockedGithubRepo) {
    let mock = MockedGithubRepo::default();
    let client = GithubClient::try_from(&mock).expect("Failed to create mock GithubClient");
    (client, mock)
}

pub async fn test_jobs_runner(db: &TestDbGuard, github: Arc<GithubClient>) -> JobsRunner {
    let tests_sleep_params = SleepParams {
        sleep_period: Duration::from_millis(100),
        max_sleep_period: Duration::from_millis(100),
        min_sleep_period: Duration::from_millis(100),
        sleep_step: Duration::from_millis(100),
    };

    let _ = crate::logic::jobs::global::init_github_client(github);
    let _ = crate::logic::jobs::global::init_db_connection(db.client());

    JobsRunner::start_pool(&db.db_url(), tests_sleep_params)
        .await
        .expect("Failed to start jobs runner")
}

pub async fn jobs_runner_test_case(
    test_name: &str,
) -> (TestDbGuard, Arc<GithubClient>, MockedGithubRepo, JobsRunner) {
    use crate::tests_utils;
    let db = test_db("test", test_name).await;
    let (github, repo) = test_github_client().await;
    let github = Arc::new(github);
    let runner = test_jobs_runner(&db, github.clone()).await;
    tests_utils::mock::insert_default_data(&db.client())
        .await
        .unwrap();
    (db, github, repo, runner)
}
