// Tests were generated via Claude.
// We need to clean them up and leave only the valid ones.

use std::time::Duration;

use super::{
    http_client::{HttpClient, HttpClientSettings},
    BeaconBlockHeadersProvider, BlobSidecarsProvider, BlobsProvider,
};

const DEFAULT_HTTP_ADDRESS: &str = "https://ethereum-sepolia-beacon-api.publicnode.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

fn create_client() -> HttpClient {
    let settings = HttpClientSettings {
        beacon_url: url::Url::parse(DEFAULT_HTTP_ADDRESS).expect("valid url"),
        timeout: DEFAULT_TIMEOUT,
    };
    HttpClient::new(settings)
}

mod beacon_block_header {
    use super::*;
    use api_client_framework::Error;

    #[tokio::test]
    async fn success_with_head() {
        let client = create_client();
        let result = client.beacon_block_header("head".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let response = result.unwrap();
        assert!(response.data.header.message.slot > 0);
    }

    #[tokio::test]
    async fn success_with_genesis() {
        let client = create_client();
        let result = client.beacon_block_header("genesis".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let response = result.unwrap();
        assert_eq!(response.data.header.message.slot, 0);
        assert_eq!(response.data.header.message.proposer_index, 0);
    }

    #[tokio::test]
    async fn success_with_finalized() {
        let client = create_client();
        let result = client.beacon_block_header("finalized".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let response = result.unwrap();
        assert!(response.finalized);
    }

    #[tokio::test]
    async fn success_with_slot_number() {
        let client = create_client();
        let result = client.beacon_block_header("1000".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        let response = result.unwrap();
        assert_eq!(response.data.header.message.slot, 1000);
    }

    #[tokio::test]
    async fn not_found_slot_too_far_in_future() {
        let client = create_client();
        let result = client
            .beacon_block_header("18446744073709551615".to_string())
            .await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::NotFound),
            "expected NotFound error"
        );
    }

    #[tokio::test]
    async fn bad_request_invalid_block_id() {
        let client = create_client();
        let result = client
            .beacon_block_header("not_a_valid_block_id!!!".to_string())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidStatusCode { status_code, .. } => {
                assert_eq!(status_code, reqwest::StatusCode::BAD_REQUEST);
            }
            Error::NotFound => {
                // Some nodes may return 404 for invalid block IDs
            }
            err => panic!("expected InvalidStatusCode or NotFound, got {:?}", err),
        }
    }
}

mod blob_sidecars {
    use super::*;
    use api_client_framework::Error;

    #[tokio::test]
    async fn success_with_head() {
        let client = create_client();
        let result = client.blob_sidecars("head".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        // Head block may or may not have blobs, so we just check the request succeeds
    }

    #[tokio::test]
    async fn success_with_finalized() {
        let client = create_client();
        let result = client.blob_sidecars("finalized".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
    }

    #[tokio::test]
    async fn not_found_slot_too_far_in_future() {
        let client = create_client();
        let result = client
            .blob_sidecars("18446744073709551615".to_string())
            .await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::NotFound),
            "expected NotFound error"
        );
    }

    #[tokio::test]
    async fn bad_request_invalid_block_id() {
        let client = create_client();
        let result = client
            .blob_sidecars("not_a_valid_block_id!!!".to_string())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidStatusCode { status_code, .. } => {
                assert_eq!(status_code, reqwest::StatusCode::BAD_REQUEST);
            }
            Error::NotFound => {
                // Some nodes may return 404 for invalid block IDs
            }
            err => panic!("expected InvalidStatusCode or NotFound, got {:?}", err),
        }
    }
}

mod blobs {
    use super::*;
    use api_client_framework::Error;

    #[tokio::test]
    async fn success_with_head() {
        let client = create_client();
        let result = client.blobs("head".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
        // Head block may or may not have blobs, so we just check the request succeeds
    }

    #[tokio::test]
    async fn success_with_finalized() {
        let client = create_client();
        let result = client.blobs("finalized".to_string()).await;

        assert!(result.is_ok(), "expected success, got {:?}", result.err());
    }

    #[tokio::test]
    async fn not_found_slot_too_far_in_future() {
        let client = create_client();
        let result = client.blobs("18446744073709551615".to_string()).await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::NotFound),
            "expected NotFound error"
        );
    }

    #[tokio::test]
    async fn bad_request_invalid_block_id() {
        let client = create_client();
        let result = client.blobs("not_a_valid_block_id!!!".to_string()).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidStatusCode { status_code, .. } => {
                assert_eq!(status_code, reqwest::StatusCode::BAD_REQUEST);
            }
            Error::NotFound => {
                // Some nodes may return 404 for invalid block IDs
            }
            err => panic!("expected InvalidStatusCode or NotFound, got {:?}", err),
        }
    }
}
