// SPDX-License-Identifier: LicenseRef-Blockscout

//! DB-backed HTTP contract tests asserting that a malformed (non-hex) path
//! segment on the `:byTx` / `:byAddress` list endpoints is rejected with
//! `InvalidArgument` (HTTP 400) before any database lookup, rather than
//! surfacing a generic internal error.

mod helpers;

use reqwest::StatusCode;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn list_endpoints_reject_malformed_hex_path_with_400() {
    let db = helpers::init_db("test", "list_endpoints_reject_malformed_hex_path").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |x| x).await;

    // (route, expected substring in the invalid-argument message)
    let cases = [
        (
            "/api/v1/interchain/messages:byTx/not-hex",
            "invalid tx_hash",
        ),
        (
            "/api/v1/interchain/messages:byAddress/not-hex",
            "invalid address",
        ),
        (
            "/api/v1/interchain/transfers:byTx/not-hex",
            "invalid tx_hash",
        ),
        (
            "/api/v1/interchain/transfers:byAddress/not-hex",
            "invalid address",
        ),
    ];

    for (route, expected_msg) in cases {
        let (status, body) = helpers::get_raw(&base, route).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "route {route}: {body}");
        // tonic InvalidArgument == code 3.
        assert_eq!(body["code"], serde_json::json!(3), "route {route}: {body}");
        assert!(
            body["message"].as_str().unwrap().contains(expected_msg),
            "route {route}: unexpected message: {body}"
        );
    }
}
