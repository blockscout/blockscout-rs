use crate::blockscout;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub blockscout_client: blockscout::Client,
}
