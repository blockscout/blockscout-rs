use crate::logic::config::ConfigError;
use std::collections::{BTreeMap, HashMap};

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

    fn new(v: Self::SourceType, context: &ConfigValidationContext) -> Result<Self, ConfigError>
    where
        Self: Sized;

    async fn build_config_vars(
        &self,
        context: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, ConfigError>;

    fn maybe_default(_context: &ConfigValidationContext) -> Option<Self::SourceType> {
        None
    }
}

#[derive(Clone, Default)]
pub struct ParsedVars(pub BTreeMap<ParsedVariableKey, serde_json::Value>);

#[derive(Clone)]
pub struct ConfigValidationContext {
    pub client_name: String,
    pub current_parsed_config: HashMap<String, Vec<ParsedVariable>>,
}

impl ConfigValidationContext {
    pub fn new(
        client_name: String,
        current_parsed_vars: HashMap<String, Vec<ParsedVariable>>,
    ) -> Self {
        Self {
            client_name,
            current_parsed_config: current_parsed_vars,
        }
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            client_name: name.into(),
            current_parsed_config: Default::default(),
        }
    }
}
