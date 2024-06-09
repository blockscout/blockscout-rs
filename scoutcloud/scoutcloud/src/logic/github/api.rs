use super::{types, GithubClient, GithubError};
use anyhow::Context;
use chrono::Utc;
use octocrab::{models as octo_types, models::RunId, Page};
use serde::Serialize;
use tracing::instrument;

impl GithubClient {
    #[instrument(skip(self, content), fields(content_len = content.len()))]
    pub async fn create_or_update_file(
        &self,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<(), GithubError> {
        let latest_commit = self
            .get_latest_commit()
            .await
            .context("get latest commit")
            .map_err(GithubError::CreatingFile)?;
        let blob = self
            .create_blob(content)
            .await
            .context("create blob")
            .map_err(GithubError::CreatingFile)?;
        let tree = self
            .create_tree(&latest_commit.sha, path, &blob.sha)
            .await
            .context("create tree")
            .map_err(GithubError::CreatingFile)?;
        let commit = self
            .create_commit(
                tree.sha,
                Self::build_commit_message(commit_message),
                latest_commit.sha,
            )
            .await
            .context("create commit")
            .map_err(GithubError::CreatingFile)?;
        self.update_branch(&commit.sha)
            .await
            .context("update branch")
            .map_err(GithubError::CreatingFile)?;
        Ok(())
    }

    pub async fn delete_file(&self, path: &str) -> Result<(), GithubError> {
        let file_sha = self.get_file(path).await?.sha;
        self.delete_file_by_sha(path, &file_sha).await?;
        Ok(())
    }

    pub async fn get_latest_commit(
        &self,
    ) -> Result<octocrab::models::repos::RepoCommit, GithubError> {
        let latest_commit = self
            .client
            .commits(self.owner.clone(), self.repo.clone())
            .get(self.default_branch_name.clone())
            .await?;
        Ok(latest_commit)
    }

    pub async fn get_file(&self, path: &str) -> Result<types::OnlySha, GithubError> {
        let url = format!(
            "/repos/{owner}/{repo}/contents/{path}",
            owner = self.owner,
            repo = self.repo,
        );
        let response = self
            .client
            .get(
                url,
                Some(&types::OnlyRef {
                    _ref: self.default_branch_name.clone(),
                }),
            )
            .await?;
        Ok(response)
    }

    pub async fn run_workflow<P: Serialize>(
        &self,
        workflow_id: impl Into<String>,
        _ref: impl Into<String>,
        inputs: P,
    ) -> Result<(), GithubError> {
        let workflow_dispatch = types::WorkflowDispatchRequest {
            _ref: _ref.into(),
            inputs,
        };
        self.client
            ._post(
                format!(
                    "/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches",
                    owner = self.owner,
                    repo = self.repo,
                    workflow_id = workflow_id.into()
                ),
                Some(&workflow_dispatch),
            )
            .await?;
        Ok(())
    }

    pub async fn get_workflow_runs(
        &self,
        workflow_id: impl Into<String>,
    ) -> Result<Vec<octo_types::workflows::Run>, GithubError> {
        let runs = self
            .client
            .workflows(self.owner.clone(), self.repo.clone())
            .list_runs(workflow_id)
            .send()
            .await?
            .take_items();
        Ok(runs)
    }

    pub async fn get_latest_workflow_run(
        &self,
        workflow_id: impl Into<String>,
        created_from: Option<chrono::DateTime<Utc>>,
    ) -> Result<Option<octo_types::workflows::Run>, GithubError> {
        let workflow_id = workflow_id.into();
        let url = format!(
            "/repos/{owner}/{repo}/actions/workflows/{workflow_id}/runs",
            owner = self.owner,
            repo = self.repo,
            workflow_id = workflow_id
        );
        let params = types::WorkflowRunsListRequest {
            created: created_from.map(|from| format!(">={}", from.to_rfc3339())),
            page: Some(1u32),
            per_page: Some(1u8),
        };
        let mut pages: Page<octo_types::workflows::Run> =
            self.client.get(url, Some(&params)).await?;

        Ok(pages.take_items().into_iter().next())
    }

    pub async fn get_workflow_run(
        &self,
        run_id: impl Into<RunId>,
    ) -> Result<octo_types::workflows::Run, GithubError> {
        let run = self
            .client
            .workflows(self.owner.clone(), self.repo.clone())
            .get(run_id.into())
            .await?;
        Ok(run)
    }

    async fn create_blob(&self, content: &str) -> Result<types::CreateBlobResponse, GithubError> {
        let blob: types::CreateBlobResponse = self
            .client
            .post(
                format!(
                    "/repos/{owner}/{repo}/git/blobs",
                    owner = self.owner,
                    repo = self.repo
                ),
                Some(&types::CreateBlobRequest::with_default_encoding(content)),
            )
            .await?;
        Ok(blob)
    }

    async fn create_tree(
        &self,
        base_tree: &str,
        path: &str,
        blob_sha: &str,
    ) -> Result<types::CreateTreeResponse, GithubError> {
        let tree: types::CreateTreeResponse = self
            .client
            .post(
                format!(
                    "/repos/{owner}/{repo}/git/trees",
                    owner = self.owner,
                    repo = self.repo
                ),
                Some(&types::CreateTreeRequest::with_single_blob(
                    base_tree, path, blob_sha,
                )),
            )
            .await?;
        Ok(tree)
    }

    async fn create_commit(
        &self,
        tree_sha: String,
        message: String,
        parent_sha: String,
    ) -> Result<types::CreateCommitResponse, GithubError> {
        let commit = self
            .client
            .post(
                format!(
                    "/repos/{owner}/{repo}/git/commits",
                    owner = self.owner,
                    repo = self.repo
                ),
                Some(&types::CreateCommitRequest {
                    tree: tree_sha,
                    message,
                    parents: vec![parent_sha],
                }),
            )
            .await?;
        Ok(commit)
    }

    async fn update_branch(&self, commit_sha: &str) -> Result<(), GithubError> {
        let _: serde_json::Value = self
            .client
            .patch(
                format!(
                    "/repos/{owner}/{repo}/git/refs/heads/{branch}",
                    owner = self.owner,
                    repo = self.repo,
                    branch = self.default_branch_name
                ),
                Some(&types::UpdateBranchRequest {
                    sha: commit_sha.to_string(),
                }),
            )
            .await?;
        Ok(())
    }

    async fn delete_file_by_sha(&self, path: &str, sha: &str) -> Result<(), GithubError> {
        let url = format!(
            "/repos/{owner}/{repo}/contents/{path}",
            owner = self.owner,
            repo = self.repo,
            path = path
        );
        let request = types::DeleteFileRequest {
            message: Self::build_commit_message("delete file"),
            sha: sha.to_string(),
        };
        let _: serde_json::Value = self.client.delete(url, Some(&request)).await?;
        Ok(())
    }

    fn build_commit_message(msg: &str) -> String {
        format!("[scoutcloud] {msg}")
    }
}

#[cfg(test)]
mod tests {
    use crate::tests_utils;

    #[tokio::test]
    async fn create_or_update_works() {
        let (client, mock) = tests_utils::init::test_github_client().await;
        let handles = mock.build_handles();

        client
            .create_or_update_file("file_name", "content2", "commit message")
            .await
            .expect("create or update file");
        handles.assert("main");
        handles.assert("new_blob");
        handles.assert("new_tree");
        handles.assert("new_commit");
        handles.assert("update_main");
    }
}
