use crate::logic::{
    ConfigError, ConfigValidationContext, ParsedVariable, ParsedVariableKey, UserVariable,
};
use url::Url;

pub struct StatsEnabled(bool);

#[async_trait::async_trait]
impl UserVariable for StatsEnabled {
    type SourceType = bool;

    fn new(v: Self::SourceType, _context: &ConfigValidationContext) -> Result<Self, ConfigError> {
        Ok(Self(v))
    }

    async fn build_config_vars(
        &self,
        context: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, ConfigError> {
        let mut vars = vec![
            (
                ParsedVariableKey::ConfigPath("stats.enabled".to_string()),
                serde_json::Value::Bool(self.0),
            ),
            (
                ParsedVariableKey::ConfigPath("stats.ingress.enabled".to_string()),
                serde_json::Value::Bool(self.0),
            ),
        ];
        if self.0 {
            let base_url = extract_base_blockscout_url(context)?;
            if !is_blockscout_indexing_finished(&base_url).await? {
                return Err(ConfigError::Validation(
                    "blockscout didnt finish indexing".to_string(),
                ));
            }
            let cors = [
                base_url.as_str().trim_end_matches('/'),
                "https://*.k8s-dev.blockscout.com",
                "https://*.blockscout.com",
                "http://localhost:3000",
            ]
            .join(", ");
            vars.push((
                ParsedVariableKey::ConfigPath(
                    concat!(
                        "stats.",
                        "ingress.",
                        "annotations.",
                        "nginx\\.ingress\\.kubernetes\\.io/cors-allow-origin"
                    )
                    .to_string(),
                ),
                serde_json::Value::String(cors),
            ));
        }

        Ok(vars)
    }
}

fn extract_base_blockscout_url(context: &ConfigValidationContext) -> Result<Url, ConfigError> {
    let hostname = context
        .current_parsed_config
        .get("instance_url")
        .and_then(|v| v.first())
        .ok_or(ConfigError::Validation(
            "instance_url should be parsed before stats_enabled".to_string(),
        ))?
        .1
        .as_str()
        .ok_or(ConfigError::Validation(
            "instance_url should be a string".to_string(),
        ))?;

    super::instance_url::hostname_to_url(hostname)
}

async fn is_blockscout_indexing_finished(base_url: &Url) -> Result<bool, anyhow::Error> {
    let status = blockscout_client::main_page::indexing_status::get(
        &blockscout_client::Client::new(blockscout_client::Config::new(
            "blockscout".to_string(),
            base_url.clone(),
        )),
    )
    .await
    .map_err(|e| anyhow::anyhow!("failed to check blockscout ({base_url}) index status: {e}"))?;
    Ok(status.finished_indexing)
}
