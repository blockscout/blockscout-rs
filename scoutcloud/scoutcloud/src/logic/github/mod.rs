mod api;
mod mock;
pub(crate) mod types;
mod workflows;

pub use mock::*;
pub use workflows::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GithubError {
    #[error("api error: {0}")]
    Octocrab(#[from] octocrab::Error),
    #[error("failed to create file: {0}")]
    CreatingFile(anyhow::Error),
    #[error("github workflow error: {0}")]
    GithubWorkflow(anyhow::Error),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Clone, Debug)]
pub struct GithubClient {
    client: octocrab::Octocrab,
    owner: String,
    repo: String,
    default_branch_name: String,
}

impl GithubClient {
    pub fn new(
        token: String,
        owner: String,
        repo: String,
        default_branch_name: String,
        uri: Option<&str>,
    ) -> Result<Self, octocrab::Error> {
        let mut builder = octocrab::Octocrab::builder();
        if let Some(uri) = uri {
            builder = builder.base_uri(uri)?;
        }
        let client = builder.personal_token(token).build()?;
        Ok(Self {
            client,
            owner,
            repo,
            default_branch_name,
        })
    }

    pub fn from_settings(
        settings: &crate::server::GithubSettings,
    ) -> Result<Self, octocrab::Error> {
        Self::new(
            settings.token.clone(),
            settings.owner.clone(),
            settings.repo.clone(),
            settings.branch.clone(),
            None,
        )
    }
}

#[cfg(test)]
impl TryFrom<&MockedGithubRepo> for GithubClient {
    type Error = octocrab::Error;

    fn try_from(mock: &MockedGithubRepo) -> Result<Self, Self::Error> {
        Self::new(
            mock.token.clone(),
            mock.owner.clone(),
            mock.repo.clone(),
            mock.default_main_branch.clone(),
            Some(mock.server.base_url().as_str()),
        )
    }
}
