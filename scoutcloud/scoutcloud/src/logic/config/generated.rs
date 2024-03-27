use crate::logic::config::{validated::ValidatedInstanceConfig, Error};
use json_dotpath::DotPaths;
lazy_static::lazy_static! {
    pub static ref DEFAULT_CONFIG: serde_json::Value = {
        serde_yaml::from_str(include_str!("default.yaml")).unwrap()
    };
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct GeneratedInstanceConfig {
    pub raw: serde_json::Value,
}

impl TryFrom<ValidatedInstanceConfig> for GeneratedInstanceConfig {
    type Error = Error;

    fn try_from(validated: ValidatedInstanceConfig) -> Result<Self, Self::Error> {
        let mut this = Self::default();
        for (key, value) in validated.vars {
            let path = key.get_path();
            update_json_by_path(&mut this.raw, &path, value).map_err(|e| {
                Error::Internal(anyhow::anyhow!("failed to update json '{path}' path: {e}"))
            })?;
        }

        Ok(this)
    }
}

impl GeneratedInstanceConfig {
    pub fn merged_with_defaults(&mut self) -> &mut Self {
        let mut this = DEFAULT_CONFIG.clone();
        merge(&mut this, &self.raw);
        self.raw = this;
        self
    }

    pub fn to_yaml(&self) -> Result<String, Error> {
        serde_yaml::to_string(&self.raw).map_err(|e| {
            Error::Internal(anyhow::anyhow!("failed to serialize config to yaml: {e}"))
        })
    }
}

fn update_json_by_path(
    json: &mut serde_json::Value,
    path: &str,
    new_value: serde_json::Value,
) -> Result<(), json_dotpath::Error> {
    json.dot_set(path, new_value)?;
    Ok(())
}

fn merge(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => *a = b.clone(),
    }
}

#[cfg(test)]
mod tests {
    use crate::logic::config::{
        generated::GeneratedInstanceConfig, validated::ValidatedInstanceConfig,
    };
    use httpmock::Method::*;
    use pretty_assertions::assert_eq;
    use scoutcloud_proto::blockscout::scoutcloud::v1::DeployConfigInternal;
    use serde_json::json;

    #[tokio::test]
    async fn config_parse_works() {
        let server = httpmock::MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("Content-Type", "application/json")
                .json_body_partial(
                    r#"{
                    "method": "eth_chainId"
                }"#,
                );
            then.status(200).json_body(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            }));
        });

        let config = DeployConfigInternal {
            rpc_url: server.url("/").parse().unwrap(),
            server_size: "small".to_string(),
            node_type: Some("geth".to_string()),
            chain_type: Some("".to_string()),
            chain_id: Some("77".to_string()),
            token_symbol: Some("".to_string()),
            instance_url: Some("".to_string()),
            logo_link: Some("http://example.com".parse().unwrap()),
            chain_name: Some("".to_string()),
            icon_link: Some("http://example.com".parse().unwrap()),
            homeplate_backgroup: Some("".to_string()),
            homeplace_text_color: Some("".to_string()),
        };

        let validated = ValidatedInstanceConfig::try_from_config(config)
            .await
            .expect("failed to parse config");

        assert_eq!(
            validated.vars.len(),
            4,
            "invalid parsed config: {:?}",
            validated
        );

        let generated =
            GeneratedInstanceConfig::try_from(validated).expect("failed to generate config");
        assert_eq!(
            generated.raw,
            json!({
                "blockscout": {
                    "env": {
                        "CHAIN_ID": "77",
                        "NODE_TYPE": "geth",
                        "ETHEREUM_JSONRPC_HTTP_URL": server.url("/"),
                    },
                    "resources": {
                      "limits": {
                        "memory": "4Gi",
                        "cpu": "2"
                      },
                      "requests": {
                        "memory": "2Gi",
                        "cpu": "1"
                      }
                    }
                },
            }),
        )
    }
}
