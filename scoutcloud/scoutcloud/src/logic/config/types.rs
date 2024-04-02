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
pub trait UserVariable: Send + Sync {
    type SourceType: Send + Sync;

    fn new(v: Self::SourceType, context: &ConfigValidationContext) -> Result<Self, Error>
    where
        Self: Sized;

    async fn build_config_vars(
        &self,
        context: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, Error>;

    fn maybe_default(_context: &ConfigValidationContext) -> Option<Self::SourceType> {
        None
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConfigValidationContext {
    pub client_name: String,
}
