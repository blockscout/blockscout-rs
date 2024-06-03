use crate::v1::types;
use crate::{Client, Result};

pub async fn get(client: &Client) -> Result<types::Health> {
    let path = "/api/v1/health";
    client.get_request(client.build_url(path)).await
}
