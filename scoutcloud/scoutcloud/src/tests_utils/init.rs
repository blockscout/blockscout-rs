use crate::{
    logic::{
        github::MockedGithubRepo,
        jobs::{global, JobsRunner},
        GithubClient,
    },
    tests_utils,
};
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

pub async fn test_jobs_runner(db: &TestDbGuard) -> JobsRunner {
    let tests_sleep_params = SleepParams {
        sleep_period: Duration::from_millis(100),
        max_sleep_period: Duration::from_millis(100),
        min_sleep_period: Duration::from_millis(100),
        sleep_step: Duration::from_millis(100),
    };
    JobsRunner::start_pool(&db.db_url(), tests_sleep_params)
        .await
        .expect("Failed to start jobs runner")
}

pub async fn jobs_runner_test_case(
    test_name: &str,
) -> (TestDbGuard, Arc<GithubClient>, MockedGithubRepo, JobsRunner) {
    let db = test_db("test", test_name).await;
    let (github, repo) = test_github_client().await;
    let github = Arc::new(github);
    global::DATABASE
        .init(db.client())
        .await
        .expect("failed to init database");
    global::GITHUB
        .init(github.clone())
        .await
        .expect("failed to init github client");
    let runner = test_jobs_runner(&db).await;
    tests_utils::mock::insert_default_data(&db.client())
        .await
        .expect("Failed to insert default data");
    (db, github, repo, runner)
}
