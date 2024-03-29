use crate::logic::config::Error;

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
    fn new(v: V, context: &ConfigValidationContext) -> Result<Self, Error>
    where
        Self: Sized;

    async fn build_config_vars(
        &self,
        context: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, Error>;

    fn maybe_default(_context: &ConfigValidationContext) -> Option<V> {
        None
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConfigValidationContext {
    pub client_name: String,
}
