#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ParsedVariableKey {
    BackendEnv(String),
    FrontendEnv(String),
    ConfigPath(String),
}

impl ParsedVariableKey {
    pub fn get_path(&self) -> String {
        match self {
            ParsedVariableKey::BackendEnv(env) => format!("blockscout.env.{env}"),
            ParsedVariableKey::FrontendEnv(env) => format!("frontend.env.{env}"),
            ParsedVariableKey::ConfigPath(path) => path.clone(),
        }
    }
}

pub type ParsedVariable = (ParsedVariableKey, serde_json::Value);

#[async_trait::async_trait]
pub trait UserVariable<V>: Send + Sync
where
    V: Send + Sync,
{
    async fn build_config_vars(v: V) -> Result<Vec<ParsedVariable>, anyhow::Error>;

    fn validate(_v: V) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn maybe_default() -> Option<V> {
        None
    }
}
