use httpmock::{Method, Mock, MockServer};
use serde::Deserialize;
use std::collections::HashMap;

pub struct MockedGithubRepo {
    pub server: MockServer,
    pub token: String,
    pub owner: String,
    pub repo: String,
    pub default_main_branch: String,
}

impl Default for MockedGithubRepo {
    fn default() -> Self {
        let server = MockServer::start();
        Self {
            server,
            token: "test-token".to_string(),
            repo: "test-repo".to_string(),
            owner: "test-owner".to_string(),
            default_main_branch: "main".to_string(),
        }
    }
}

pub struct GithubMockedHandles<'a>(HashMap<String, Mock<'a>>);

impl GithubMockedHandles<'_> {
    pub fn assert(&self, name: &str) {
        self.with_name(name).assert();
    }

    pub fn assert_hits(&self, name: &str, hits: usize) {
        self.with_name(name).assert_hits(hits);
    }

    pub fn with_name(&self, name: &str) -> &Mock {
        let name = if !name.ends_with(".json") {
            name.to_string() + ".json"
        } else {
            name.to_string()
        };
        self.0
            .get(&name)
            .unwrap_or_else(|| panic!("provided name '{name}' should be in handles map"))
    }
}

#[derive(Debug, Deserialize)]
struct MockCase {
    filename: String,
    url: String,
    method: Method,
    status: u16,
    response: serde_json::Value,
}

impl MockedGithubRepo {
    pub fn build_mock_handlers(&self) -> GithubMockedHandles {
        let mut handles = HashMap::new();
        for case_raw in [
            include_str!("data/commits.json"),
            include_str!("data/main.json"),
            include_str!("data/new_blob.json"),
            include_str!("data/new_commit.json"),
            include_str!("data/new_tree.json"),
            include_str!("data/workflows.json"),
            include_str!("data/update_main.json"),
            include_str!("data/dispatch_cleanup_yaml.json"),
            include_str!("data/dispatch_deploy_yaml.json"),
            include_str!("data/runs_cleanup_yaml.json"),
            include_str!("data/runs_deploy_yaml.json"),
        ] {
            let case: MockCase = serde_json::from_str(case_raw).expect("invalid json");
            let url = case
                .url
                .replace("{owner}", &self.owner)
                .replace("{repo}", &self.repo);
            let mock = self.server.mock(|when, then| {
                when.method(case.method).path(&url);
                then.status(case.status).json_body(case.response);
            });
            handles.insert(case.filename, mock);
        }
        GithubMockedHandles(handles)
    }
}
