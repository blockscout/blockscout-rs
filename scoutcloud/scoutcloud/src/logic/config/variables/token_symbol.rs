use crate::logic::{ParsedVariable, ParsedVariableKey, UserVariable};

pub struct TokenSymbol {}

#[async_trait::async_trait]
impl UserVariable<String> for TokenSymbol {
    async fn build_config_vars(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        Ok(vec![
            (
                ParsedVariableKey::ConfigPath("config.network.currency.name".to_string()),
                serde_json::json!(v),
            ),
            (
                ParsedVariableKey::ConfigPath("config.network.currency.symbol".to_string()),
                serde_json::json!(v),
            ),
        ])
    }

    fn maybe_default() -> Option<String> {
        Some("ETH".to_string())
    }
}
