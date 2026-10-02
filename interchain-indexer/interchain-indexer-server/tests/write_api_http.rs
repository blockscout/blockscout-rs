// SPDX-License-Identifier: LicenseRef-Blockscout

//! HTTP smoke test of the operator write API: the `x-api-key` header must reach
//! the handler, and the JSON/HTTP contract (401, 404, 400) must hold.
//!
//! One test function on purpose: every `init_interchain_indexer_server` boots a
//! whole service and never shuts it down, so more of them exhaust the file
//! descriptor limit on macOS (see the `test` recipe in the justfile).

mod helpers;

use interchain_indexer_entity::stats_assets;
use reqwest::StatusCode;
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const ROUTE: &str = "/api/v1/admin/stats/assets:setIcon";
const KEY: &str = "test-key";

#[tokio::test]
#[ignore = "needs database"]
async fn write_api_http_smoke() {
    let db = helpers::init_db("write_api", "write_api_http_smoke").await;
    let key_digest = hex::encode(Sha256::digest(KEY.as_bytes()));
    let base = helpers::init_interchain_indexer_server(db.db_url(), |mut settings| {
        settings
            .write_api
            .keys_sha256
            .insert("tester".to_string(), key_digest.clone());
        settings
    })
    .await;
    let url = base.join(ROUTE).unwrap();
    let client = reqwest::Client::new();

    let asset_id = stats_assets::Entity::insert(stats_assets::ActiveModel {
        icon_url: Set(None),
        ..Default::default()
    })
    .exec_with_returning(db.client().as_ref())
    .await
    .unwrap()
    .id;

    // The body is JSON and int64 values are JSON strings.
    let post = |key: Option<&str>, content_type: Option<&str>, body: Value| {
        let mut request = client.post(url.clone()).body(body.to_string());
        if let Some(key) = key {
            request = request.header("x-api-key", key);
        }
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
        }
        async move {
            let response = request.send().await.expect("request failed");
            let status = response.status();
            let body: Value = response.json().await.expect("body is not JSON");
            (status, body)
        }
    };
    let json_type = Some("application/json");
    let unknown_asset = json!({
        "stats_asset_id": "999999",
        "icon_url": "https://example.com/i.png",
        "reason": "smoke",
    });

    // No key: 401 with the gRPC `UNAUTHENTICATED` code in the body.
    let (status, body) = post(None, json_type, unknown_asset.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["code"], json!(16), "{body}");

    // A wrong key is the same 401.
    let (status, body) = post(Some("wrong-key"), json_type, unknown_asset.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

    // A valid key reaches the handler (so the header got into the request
    // metadata), which reports the unknown asset.
    let (status, body) = post(Some(KEY), json_type, unknown_asset).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["code"], json!(5), "{body}");

    // int64 as a JSON number is rejected before the handler.
    let (status, body) = post(
        Some(KEY),
        json_type,
        json!({
            "stats_asset_id": 999999,
            "icon_url": "https://example.com/i.png",
            "reason": "smoke",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // So is a body sent without `Content-Type: application/json`.
    let (status, body) = post(
        Some(KEY),
        None,
        json!({
            "stats_asset_id": asset_id.to_string(),
            "icon_url": "https://example.com/i.png",
            "reason": "smoke",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // A request that passes everything applies, and `null` is explicit in the
    // response for an asset that had no icon.
    let (status, body) = post(
        Some(KEY),
        json_type,
        json!({
            "stats_asset_id": asset_id.to_string(),
            "icon_url": "https://example.com/i.png",
            "reason": "smoke",
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body.get("icon_url_before"),
        Some(&Value::Null),
        "the field must be present and null, not omitted: {body}"
    );
    assert_eq!(body["icon_url_after"], json!("https://example.com/i.png"));
    assert!(
        body["audit_id"].is_string(),
        "int64 is a JSON string: {body}"
    );
}
