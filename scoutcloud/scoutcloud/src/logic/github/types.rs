use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Serialize, Deserialize, Debug)]
pub struct OnlySha {
    pub sha: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateBlobRequest {
    pub content: String,
    pub encoding: String,
}

impl CreateBlobRequest {
    pub fn with_default_encoding(content: impl Display) -> Self {
        Self {
            content: content.to_string(),
            encoding: "utf-8".to_string(),
        }
    }
}

pub type CreateBlobResponse = OnlySha;

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateTreeRequest {
    pub base_tree: String,
    pub tree: Vec<TreeItem>,
    pub blob_sha: String,
}

impl CreateTreeRequest {
    pub fn with_single_blob(base_tree: &str, path: &str, blob_sha: &str) -> Self {
        Self {
            base_tree: base_tree.to_string(),
            tree: vec![TreeItem {
                path: path.to_string(),
                mode: "100644".to_string(),
                _type: "blob".to_string(),
                sha: blob_sha.to_string(),
            }],
            blob_sha: blob_sha.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TreeItem {
    pub path: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub sha: String,
}

pub type CreateTreeResponse = OnlySha;

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateCommitRequest {
    pub message: String,
    pub parents: Vec<String>,
    pub tree: String,
}

pub type CreateCommitResponse = OnlySha;

pub type UpdateBranchRequest = OnlySha;

#[derive(Serialize, Debug)]
pub struct WorkflowDispatchRequest<P: Serialize + ?Sized> {
    #[serde(rename = "ref")]
    pub _ref: String,
    pub inputs: P,
}

#[derive(Serialize, Debug)]
pub struct WorkflowRunsListRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}

// https://github.com/octokit/webhooks.net/blob/aaeeebd41d7ff49a3253146a5e54d0410e6b4ad0/src/Octokit.Webhooks/Models/WorkflowRunStatus.cs#L4
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Requested,
    InProgress,
    Completed,
    Queued,
    Waiting,
}

impl RunStatus {
    pub fn try_from_str(value: impl Into<String>) -> Result<Self, anyhow::Error> {
        let value = value.into();
        serde_plain::from_str(&value)
            .map_err(|_| anyhow::anyhow!("invalid run status from github: {value}"))
    }
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed)
    }
}

// https://github.com/octokit/webhooks.net/blob/aaeeebd41d7ff49a3253146a5e54d0410e6b4ad0/src/Octokit.Webhooks/Models/WorkflowRunConclusion.cs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunConclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    TimedOut,
    ActionRequired,
    Stale,
    Skipped,
}

impl RunConclusion {
    pub fn try_from_str(value: impl Into<String>) -> Result<Self, anyhow::Error> {
        let value = value.into();
        serde_plain::from_str(&value)
            .map_err(|_| anyhow::anyhow!("invalid run conclusion from github: {value}"))
    }

    pub fn is_ok(&self) -> bool {
        match self {
            RunConclusion::Success | RunConclusion::Neutral => true,
            RunConclusion::Failure
            | RunConclusion::Cancelled
            | RunConclusion::TimedOut
            | RunConclusion::ActionRequired
            | RunConclusion::Stale
            | RunConclusion::Skipped => false,
        }
    }
}
