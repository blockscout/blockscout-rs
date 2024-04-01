use super::{GithubClient, GithubError};
use chrono::Utc;
use octocrab::models::workflows::Run;
use serde::{Deserialize, Serialize};

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

    async fn run_and_get_latest(
        &self,
        client: &GithubClient,
        max_try: u8,
    ) -> Result<Option<Run>, GithubError> {
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
pub enum AppVariant {
    #[serde(rename = "autodeploy")]
    Instance,
    #[serde(rename = "autodeploy-postgresql")]
    Postgres,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployWorkflow {
    pub client: String,
    pub app: AppVariant,
}

impl Workflow for DeployWorkflow {
    fn id() -> &'static str {
        "deploy.yaml"
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CleanupWorkflow {
    pub client: String,
    pub app: AppVariant,
}

impl Workflow for CleanupWorkflow {
    fn id() -> &'static str {
        "cleanup.yaml"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::github::MockedGithubRepo;

    #[tokio::test]
    async fn run_and_get_workflow_works() {
        let mock_repo = MockedGithubRepo::default();
        let handles = mock_repo.build_mock_handlers();

        let client = GithubClient::try_from(&mock_repo).unwrap();

        let deploy = DeployWorkflow {
            client: "test-client".to_string(),
            app: AppVariant::Instance,
        };
        let run = deploy
            .run_and_get_latest(&client, 5)
            .await
            .expect("run and get workflow")
            .expect("no workflows returned");

        assert!(
            run.name.contains("autodeploy"),
            "run name {} should contain autodeploy",
            run.name
        );
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
