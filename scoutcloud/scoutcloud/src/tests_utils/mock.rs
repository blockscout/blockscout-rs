use httpmock::{Method::GET, MockServer};
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde_json::json;

pub async fn insert_default_data(db: &DatabaseConnection) -> Result<(), anyhow::Error> {
    db.execute_unprepared(include_str!("data/default.sql"))
        .await?;
    Ok(())
}

pub fn mock_blockscout(healthy: bool, indexed: bool) -> MockServer {
    let server = MockServer::start();
    let _mock = server.mock(|when, then| {
        when.method(GET).path("/api/v2/main-page/indexing-status");
        then.status(200).json_body(json!({
          "finished_indexing": indexed,
          "finished_indexing_blocks": indexed,
          "indexed_blocks_ratio": "1.0",
          "indexed_internal_transactions_ratio": "1.0"
        }));
    });

    let _mock = server.mock(|when, then| {
        when.method(GET).path("/api/v1/health");
        then.status(200).json_body(json!({
            "data": {
                "cache_latest_block_inserted_at": "2024-06-04 13:36:11.000000Z",
                "cache_latest_block_number": "20018794",
                "latest_block_inserted_at": "2024-06-04 13:36:23.000000Z",
                "latest_block_number": "20018795"
            },
            "healthy": healthy
        }));
    });

    server
}
