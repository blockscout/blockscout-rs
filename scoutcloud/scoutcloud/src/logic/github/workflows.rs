use super::{types::RunStatus, GithubClient, GithubError};
use crate::logic::github::types::RunConclusion;
use chrono::Utc;
use lazy_static::lazy_static;
use octocrab::models::workflows::Run;
use serde::{Deserialize, Serialize};
use std::time::Duration;

lazy_static! {
    static ref GITHUB_WORKFLOW_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

#[async_trait::async_trait]
pub trait Workflow: Serialize + Send + Sync {
    fn id() -> &'static str;

    async fn run(&self, client: &GithubClient) -> Result<(), GithubError> {
        client
            .run_workflow(Self::id(), &client.default_branch_name, self)
            .await
    }
    async fn get_latest_run(
        client: &GithubClient,
        created_from: Option<chrono::DateTime<Utc>>,
    ) -> Result<Option<Run>, GithubError> {
        client
            .get_latest_workflow_run(Self::id(), created_from)
            .await
    }

    async fn run_and_get_latest_with_mutex(
        &self,
        client: &GithubClient,
        max_try: u8,
    ) -> Result<Option<Run>, GithubError> {
        // since we want to start workflow and get the latest run,
        // we need to lock the mutex to prevent getting wrong run
        let _lock = GITHUB_WORKFLOW_MUTEX.lock().await;
        let now = chrono::Utc::now();
        self.run(client).await?;
        for _ in 0..max_try {
            let maybe_run = Self::get_latest_run(client, Some(now)).await?;
            if let Some(run) = maybe_run {
                return Ok(Some(run));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        Ok(None)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployWorkflow {
    pub client: String,
}

impl Workflow for DeployWorkflow {
    fn id() -> &'static str {
        "deploy.yaml"
    }
}

impl DeployWorkflow {
    pub fn new(client: String) -> Self {
        Self { client }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CleanupWorkflow {
    pub client: String,
}

impl Workflow for CleanupWorkflow {
    fn id() -> &'static str {
        "cleanup.yaml"
    }
}

impl CleanupWorkflow {
    pub fn new(client: String) -> Self {
        Self { client }
    }
}

impl GithubClient {
    pub async fn wait_for_success_workflow(
        &self,
        run: &Run,
        timeout: Duration,
        sleep_between: Duration,
    ) -> Result<RunConclusion, GithubError> {
        let (status, conclusion) = self
            .wait_for_final_status_workflow(run, timeout, sleep_between)
            .await?;
        let run_name_debug = run.name.to_string();
        match status.is_completed() {
            true => match conclusion {
                Some(conclusion) if conclusion.is_ok() => {
                    tracing::info!(conclusion = ?conclusion, "'{run_name_debug}' deploy completed");
                    Ok(conclusion)
                }
                Some(conclusion) => Err(GithubError::GithubWorkflow(anyhow::anyhow!(
                    "failed to start '{run_name_debug}'. conclusion={conclusion:?}"
                ))),
                None => Err(GithubError::Internal(anyhow::anyhow!(
                    "no final result for workflow"
                ))),
            },
            false => Err(GithubError::GithubWorkflow(anyhow::anyhow!(
                "timed out waiting for '{run_name_debug}' deploy. status={status:?}"
            ))),
        }
    }

    async fn wait_for_final_status_workflow(
        &self,
        run: &Run,
        timeout: Duration,
        sleep_between: Duration,
    ) -> Result<(RunStatus, Option<RunConclusion>), GithubError> {
        tracing::info!(
            run_id = run.id.to_string(),
            "waiting for github workflow run '{}'",
            run.name
        );

        let maybe_timeout = tokio::time::timeout(timeout, async move {
            loop {
                tokio::time::sleep(sleep_between).await;
                let run = self.get_workflow_run(run.id).await?;
                let status = RunStatus::try_from_str(&run.status)?;
                let conclusion = run
                    .conclusion
                    .as_ref()
                    .map(RunConclusion::try_from_str)
                    .transpose()?;
                if status.is_completed() {
                    return Ok((status, conclusion));
                }
            }
        })
        .await;

        match maybe_timeout {
            Ok(result) => result,
            Err(_) => {
                let run = self.get_workflow_run(run.id).await?;
                let status = RunStatus::try_from_str(&run.status)?;
                let conclusion = run
                    .conclusion
                    .as_ref()
                    .map(RunConclusion::try_from_str)
                    .transpose()?;
                Ok((status, conclusion))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_utils;

    #[tokio::test]
    async fn run_and_get_workflow_works() {
        let (client, mock) = tests_utils::init::test_github_client().await;
        let handles = mock.build_handles();

        let deploy = DeployWorkflow {
            client: "test-client".to_string(),
        };
        let run = deploy
            .run_and_get_latest_with_mutex(&client, 5)
            .await
            .expect("run and get workflow")
            .expect("no workflows returned");

        // assert!(
        //     run.name.contains("autodeploy"),
        //     "run name {} should contain autodeploy",
        //     run.name
        // );

        // note that this value is configured inside `mock/data/fetch.py`, not in `DeployWorkflow.client`
        assert!(
            run.name.contains("test-client"),
            "run name {} should contain test-client",
            run.name
        );

        handles.assert("dispatch_deploy_yaml");
        handles.assert("runs_deploy_yaml");
        handles.assert_hits("dispatch_cleanup_yaml", 0);
        handles.assert_hits("runs_cleanup_yaml", 0);

        CleanupWorkflow::get_latest_run(&client, None)
            .await
            .expect("get workflow runs");
        handles.assert("runs_cleanup_yaml");
    }
}
