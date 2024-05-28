use super::types;
use crate::{Client, Result};

pub async fn get(client: &Client) -> Result<types::IndexingStatus> {
    let path = "/api/v2/main-page/indexing-status";
    client.get_request(client.build_url(path)).await
}
