use super::Error;
use crate::logic::ValidatedInstanceConfig;
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
    pub fn from_default_file() -> Self {
        let raw = DEFAULT_CONFIG.clone();
        Self { raw }
    }

    pub fn merge(&mut self, other: &Self) -> &mut Self {
        merge(&mut self.raw, &other.raw);
        self
    }

    pub fn merged_with_defaults(&mut self) -> &mut Self {
        // we override default config with current config
        // therefore we merge `default` with `self`
        let mut default = Self::from_default_file();
        default.merge(self);
        self.raw = default.raw;
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
    use httpmock::{Method::*, MockServer};
    use pretty_assertions::assert_eq;
    use scoutcloud_proto::blockscout::scoutcloud::v1::{
        DeployConfigInternal, DeployConfigPartialInternal,
    };
    use serde_json::json;

    fn mock_rpc() -> MockServer {
        let server = httpmock::MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("Content-Type", "application/json")
                .json_body_partial(r#"{"method": "eth_chainId"}"#);
            then.status(200).json_body(serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": "0x1"
            }));
        });

        server
    }

    fn test_deploy_instance_config(server: &MockServer) -> DeployConfigInternal {
        DeployConfigInternal {
            rpc_url: server.url("/").parse().unwrap(),
            server_size: "small".to_string(),
            node_type: Some("geth".to_string()),
            chain_type: Some("stability".to_string()),
            chain_id: Some("77".to_string()),
            token_symbol: Some("EEE".to_string()),
            instance_url: Some("hostname-test".to_string()),
            logo_url: Some("http://example.com".parse().unwrap()),
            chain_name: Some("chain-test".to_string()),
            icon_url: Some("http://example.com".parse().unwrap()),
            homeplate_background: Some("#111111".to_string()),
            homeplate_text_color: Some("#222222".to_string()),
        }
    }

    #[tokio::test]
    async fn config_parse_works() {
        let server = mock_rpc();
        let config = test_deploy_instance_config(&server);
        let validated = ValidatedInstanceConfig::try_from_config(config)
            .await
            .expect("failed to parse config");

        assert_eq!(
            validated.vars.len(),
            14,
            "invalid parsed config: {:?}",
            validated
        );

        let generated =
            GeneratedInstanceConfig::try_from(validated).expect("failed to generate config");
        assert_eq!(
            generated.raw,
            json!({
                "blockscout": {
                    "image": {
                        "repository": "blockscout/blockscout-stability",
                    },
                    "ingress": {
                        "hostname": "hostname-test.k8s-dev.blockscout.com",
                    },
                    "env": {
                        "CHAIN_ID": "77",
                        "CHAIN_TYPE": "stability",
                        "NODE_TYPE": "geth",
                        "ETHEREUM_JSONRPC_HTTP_URL": server.url("/"),
                        "INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER": "true",
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
                "frontend": {
                    "env": {
                        "NEXT_PUBLIC_HOMEPAGE_PLATE_BACKGROUND": "#111111",
                        "NEXT_PUBLIC_HOMEPAGE_PLATE_TEXT_COLOR": "#222222",
                        "NEXT_PUBLIC_NETWORK_ICON": "http://example.com/",
                        "NEXT_PUBLIC_NETWORK_NAME": "chain-test",
                    }
                },
                "config": {
                    "network": {
                        "currency": {
                            "name": "EEE",
                            "symbol": "EEE",
                        }
                    }
                }
            }),
        )
    }

    #[tokio::test]
    async fn config_empty_parse_works() {
        let server = mock_rpc();
        let config = DeployConfigInternal {
            rpc_url: server.url("/").parse().unwrap(),
            server_size: "medium".to_string(),
            node_type: None,
            chain_type: None,
            chain_id: None,
            token_symbol: None,
            instance_url: None,
            logo_url: None,
            chain_name: None,
            icon_url: None,
            homeplate_background: None,
            homeplate_text_color: None,
        };

        let validated = ValidatedInstanceConfig::try_from_config(config)
            .await
            .expect("failed to parse config");

        assert_eq!(
            validated.vars.len(),
            6,
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
                        "NODE_TYPE": "geth",
                        "ETHEREUM_JSONRPC_HTTP_URL": server.url("/"),
                        "INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER": "true",
                    },
                    "resources": {
                      "limits": {
                        "memory": "8Gi",
                        "cpu": "4"
                      },
                      "requests": {
                        "memory": "4Gi",
                        "cpu": "2"
                      }
                    }
                },
                "config": {
                    "network": {
                        "currency": {
                            "name": "ETH",
                            "symbol": "ETH",
                        }
                    }
                }
            }),
        )
    }

    #[tokio::test]
    async fn config_partial_parse_works() {
        let config = DeployConfigPartialInternal {
            rpc_url: None,
            server_size: None,
            node_type: None,
            chain_type: Some("rsk".to_string()),
            chain_id: None,
            token_symbol: None,
            instance_url: None,
            logo_url: None,
            chain_name: None,
            icon_url: None,
            homeplate_background: None,
            homeplate_text_color: None,
        };

        let validated = ValidatedInstanceConfig::try_from_config_partial(config)
            .await
            .expect("failed to parse config");

        let generated =
            GeneratedInstanceConfig::try_from(validated).expect("failed to generate config");

        assert_eq!(
            generated.raw,
            json!({
                "blockscout": {
                    "image": {
                        "repository": "blockscout/blockscout-rsk",
                    },
                    "env": {
                        "CHAIN_TYPE": "rsk"
                    }
                }
            }),
        )
    }

    #[tokio::test]
    async fn config_merged_with_default_works() {
        let server = mock_rpc();
        let config = test_deploy_instance_config(&server);
        let server_url = server.url("/").to_string();
        let validated = ValidatedInstanceConfig::try_from_config(config)
            .await
            .expect("failed to parse config");
        let raw_yaml = GeneratedInstanceConfig::try_from(validated)
            .expect("failed to generate config")
            .merged_with_defaults()
            .to_yaml()
            .expect("failed to serialize config to yaml");
        assert_eq!(
            raw_yaml,
            format!(
                r#"blockscout:
  enabled: true
  env:
    ACCOUNT_POOL_SIZE: 10
    CHAIN_ID: '77'
    CHAIN_TYPE: stability
    COIN_BALANCE_HISTORY_DAYS: 90
    DISABLE_EXCHANGE_RATES: 'true'
    ETHEREUM_JSONRPC_DEBUG_TRACE_TRANSACTION_TIMEOUT: 20s
    ETHEREUM_JSONRPC_HTTP_URL: {server_url}
    FETCH_REWARDS_WAY: manual
    GRAPHIQL_TRANSACTION: 0xbf69c7abc4fee283b59a9633dadfdaedde5c5ee0fba3e80a08b5b8a3acbd4363
    HEALTHY_BLOCKS_PERIOD: 60
    HEART_BEAT_TIMEOUT: 30
    INDEXER_CATCHUP_BLOCKS_BATCH_SIZE: 20
    INDEXER_CATCHUP_BLOCKS_CONCURRENCY: 10
    INDEXER_COIN_BALANCES_BATCH_SIZE: 50
    INDEXER_DISABLE_EMPTY_BLOCKS_SANITIZER: 'false'
    INDEXER_DISABLE_INTERNAL_TRANSACTIONS_FETCHER: 'true'
    INDEXER_INTERNAL_TRANSACTIONS_BATCH_SIZE: 3
    INDEXER_MEMORY_LIMIT: 3g
    INDEXER_RECEIPTS_BATCH_SIZE: 50
    MICROSERVICE_SIG_PROVIDER_ENABLED: 'false'
    NODE_TYPE: geth
    POOL_SIZE: 200
    POOL_SIZE_API: 10
    SOURCIFY_INTEGRATION_ENABLED: 'true'
    TXS_STATS_DAYS_TO_COMPILE_AT_INIT: 10
  image:
    repository: blockscout/blockscout-stability
    tag: 6.3.0
  ingress:
    enabled: true
    hostname: hostname-test.k8s-dev.blockscout.com
  resources:
    limits:
      cpu: '2'
      memory: 4Gi
    requests:
      cpu: '1'
      memory: 2Gi
config:
  network:
    currency:
      name: EEE
      symbol: EEE
frontend:
  enabled: true
  env:
    NEXT_PUBLIC_API_BASE_PATH: /
    NEXT_PUBLIC_API_SPEC_URL: https://raw.githubusercontent.com/blockscout/blockscout-api-v2-swagger/main/swagger.yaml
    NEXT_PUBLIC_GRAPHIQL_TRANSACTION: 0xbf69c7abc4fee283b59a9633dadfdaedde5c5ee0fba3e80a08b5b8a3acbd4363
    NEXT_PUBLIC_HAS_BEACON_CHAIN: 'true'
    NEXT_PUBLIC_HOMEPAGE_CHARTS: '[''daily_txs'']'
    NEXT_PUBLIC_HOMEPAGE_PLATE_BACKGROUND: '#111111'
    NEXT_PUBLIC_HOMEPAGE_PLATE_TEXT_COLOR: '#222222'
    NEXT_PUBLIC_NETWORK_ICON: http://example.com/
    NEXT_PUBLIC_NETWORK_NAME: chain-test
    NEXT_PUBLIC_NETWORK_VERIFICATION_TYPE: validation
    NEXT_PUBLIC_VISUALIZE_API_HOST: https://visualizer.services.blockscout.com
  envFromSecret:
    FAVICON_GENERATOR_API_KEY: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/NEXT_PUBLIC_FAVICON_GENERATOR_API_KEY
    NEXT_PUBLIC_MIXPANEL_PROJECT_TOKEN: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/NEXT_PUBLIC_MIXPANEL_PROJECT_TOKEN
    NEXT_PUBLIC_RE_CAPTCHA_APP_SITE_KEY: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/NEXT_PUBLIC_RE_CAPTCHA_APP_SITE_KEY
    NEXT_PUBLIC_SENTRY_DSN: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/NEXT_PUBLIC_SENTRY_DSN
    NEXT_PUBLIC_WALLET_CONNECT_PROJECT_ID: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/NEXT_PUBLIC_WALLET_CONNECT_PROJECT_ID
    SENTRY_CSP_REPORT_URI: ref+vault://deployment-values/blockscout/common?token_env=VAULT_TOKEN&address=https://vault.k8s.blockscout.com#/SENTRY_CSP_REPORT_URI
  image:
    pullPolicy: Always
    tag: latest
  ingress:
    enabled: true
  replicas:
    app: 2
  resources: null
postgresql:
  resources:
    limits:
      cpu: '1'
      memory: 4Gi
    requests:
      cpu: 300m
      memory: 512Mi
"#
            )
        )
    }
}
