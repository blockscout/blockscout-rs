// SPDX-License-Identifier: LicenseRef-Blockscout

// Pins the JSON shape of the admin messages. The HTTP route (de)serializes them
// through serde, so these tests describe what a client must send and receive.
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    SetStatsAssetIconRequest, SetStatsAssetIconResponse, SetTokenIconRequest, SetTokenIconResponse,
};
use serde_json::json;

#[test]
fn admin_json_stats_asset_id_must_be_a_string() {
    let request: SetStatsAssetIconRequest = serde_json::from_value(json!({
        "stats_asset_id": "42",
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .expect("an int64 sent as a JSON string is accepted");
    assert_eq!(request.stats_asset_id, 42);
    assert_eq!(
        request.icon_url.as_deref(),
        Some("https://example.com/i.png")
    );

    serde_json::from_value::<SetStatsAssetIconRequest>(json!({
        "stats_asset_id": 42,
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .expect_err("an int64 sent as a JSON number is rejected");
}

#[test]
fn admin_json_missing_reason_is_an_error() {
    serde_json::from_value::<SetStatsAssetIconRequest>(json!({
        "stats_asset_id": "42",
        "clear": true,
    }))
    .expect_err("`reason` is a non-optional field, so it is required");
}

#[test]
fn admin_json_absent_clear_is_none() {
    let request: SetStatsAssetIconRequest = serde_json::from_value(json!({
        "stats_asset_id": "42",
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .unwrap();
    assert_eq!(request.clear, None);

    let request: SetStatsAssetIconRequest = serde_json::from_value(json!({
        "stats_asset_id": "42",
        "clear": true,
        "reason": "TICKET-1",
    }))
    .unwrap();
    assert_eq!(request.clear, Some(true));
    assert_eq!(request.icon_url, None);
}

#[test]
fn admin_json_null_before_is_serialized_as_null() {
    let response = SetStatsAssetIconResponse {
        audit_id: 7,
        icon_url_before: None,
        icon_url_after: Some("https://example.com/i.png".to_string()),
    };
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["audit_id"], "7");
    assert_eq!(
        json.get("icon_url_before"),
        Some(&serde_json::Value::Null),
        "`null` before means the asset had no icon; it must not be omitted"
    );
    assert_eq!(json["icon_url_after"], "https://example.com/i.png");
}

#[test]
fn admin_json_set_token_icon_chain_id_must_be_a_string() {
    let request: SetTokenIconRequest = serde_json::from_value(json!({
        "chain_id": "100",
        "address": "0x6b175474e89094c44da98b954eedeac495271d0f",
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .expect("an int64 sent as a JSON string is accepted");
    assert_eq!(request.chain_id, 100);
    assert_eq!(
        request.address.as_deref(),
        Some("0x6b175474e89094c44da98b954eedeac495271d0f")
    );
    assert_eq!(request.native, None);

    serde_json::from_value::<SetTokenIconRequest>(json!({
        "chain_id": 100,
        "native": true,
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .expect_err("an int64 sent as a JSON number is rejected");
}

#[test]
fn admin_json_set_token_icon_absent_native_is_none() {
    let request: SetTokenIconRequest = serde_json::from_value(json!({
        "chain_id": "1",
        "address": "0x6b175474e89094c44da98b954eedeac495271d0f",
        "clear": true,
        "reason": "TICKET-1",
    }))
    .unwrap();
    assert_eq!(request.native, None);
    assert_eq!(request.clear, Some(true));
    assert_eq!(request.icon_url, None);

    let request: SetTokenIconRequest = serde_json::from_value(json!({
        "chain_id": "100",
        "native": true,
        "icon_url": "https://example.com/i.png",
        "reason": "TICKET-1",
    }))
    .unwrap();
    assert_eq!(request.native, Some(true));
    assert_eq!(request.address, None);
    assert_eq!(request.clear, None);

    serde_json::from_value::<SetTokenIconRequest>(json!({
        "chain_id": "100",
        "native": true,
        "clear": true,
    }))
    .expect_err("`reason` is a non-optional field, so it is required");
}

#[test]
fn admin_json_set_token_icon_null_before_is_serialized_as_null() {
    let response = SetTokenIconResponse {
        audit_id: 7,
        icon_url_before: None,
        icon_url_after: None,
    };
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["audit_id"], "7");
    for field in ["icon_url_before", "icon_url_after"] {
        assert_eq!(
            json.get(field),
            Some(&serde_json::Value::Null),
            "`{field}` must be present and null, not omitted"
        );
    }
}
