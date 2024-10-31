use crate::{
    repository::api_keys,
    types::api_keys::{ApiKey, ApiKeyError},
};
use sea_orm::DatabaseConnection;

pub struct ApiKeyManager {
    db: DatabaseConnection,
}

impl ApiKeyManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn validate_api_key(&self, api_key: ApiKey) -> Result<(), ApiKeyError> {
        let api_key =
            api_keys::find_by_key_and_chain_id(&self.db, api_key.key, api_key.chain_id).await?;
        if api_key.is_none() {
            return Err(ApiKeyError::InvalidToken("Invalid API key".to_string()));
        }
        Ok(())
    }
}
