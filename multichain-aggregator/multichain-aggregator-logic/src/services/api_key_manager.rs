use crate::{
    repository::api_keys,
    types::api_keys::{ApiKey, ApiKeyError},
};
use sea_orm::DatabaseConnection;

pub struct ApiKeyManager {
    db: DatabaseConnection,
    metadata_import_api_key: Option<String>,
}

impl ApiKeyManager {
    pub fn new(db: DatabaseConnection, metadata_import_api_key: Option<String>) -> Self {
        Self {
            db,
            metadata_import_api_key,
        }
    }

    pub async fn validate_api_key(&self, api_key: ApiKey) -> Result<(), ApiKeyError> {
        let api_key =
            api_keys::find_by_key_and_chain_id(&self.db, api_key.key, api_key.chain_id).await?;

        match api_key {
            Some(_) => Ok(()),
            None => Err(ApiKeyError::InvalidToken("Invalid API key".to_string())),
        }
    }

    pub fn validate_metadata_import_api_key(&self, api_key: &str) -> Result<(), ApiKeyError> {
        if self.metadata_import_api_key.as_deref() != Some(api_key) {
            return Err(ApiKeyError::InvalidToken("Invalid API key".to_string()));
        }
        Ok(())
    }
}
