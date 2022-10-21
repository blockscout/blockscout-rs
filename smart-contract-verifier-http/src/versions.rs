use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct VersionsResponse {
    pub versions: Vec<String>,
}

#[cfg(test)]
mod tests {
    use crate::{tests::parse::test_serialize_json_ok, VersionsResponse};
    use serde_json::json;

    #[test]
    fn parse_response() {
        test_serialize_json_ok(vec![
            (
                VersionsResponse { versions: vec![] },
                json!({"versions": []}),
            ),
            (
                VersionsResponse {
                    versions: vec![
                        "v0.8.17+commit.8df45f5f".into(),
                        "v0.8.17-nightly.2022.8.24+commit.22a0c46e".into(),
                        "v0.8.17-nightly.2022.8.22+commit.a3de6cd6".into(),
                    ],
                },
                json!({"versions": ["v0.8.17+commit.8df45f5f","v0.8.17-nightly.2022.8.24+commit.22a0c46e","v0.8.17-nightly.2022.8.22+commit.a3de6cd6"]}),
            ),
        ])
    }
}
