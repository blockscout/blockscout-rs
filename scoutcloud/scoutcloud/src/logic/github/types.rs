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
