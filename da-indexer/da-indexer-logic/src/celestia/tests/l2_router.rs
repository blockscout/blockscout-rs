use crate::celestia::l2_router::{
    types::{L2BatchMetadata, L2Config, L2Type},
    L2Router,
};
use blockscout_display_bytes::{Bytes, ToHex};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
    time,
};
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[derive(Clone, Copy, Debug)]
enum TestL2Type {
    Optimism,
    Arbitrum,
}

impl Display for TestL2Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TestL2Type::Optimism => write!(f, "optimism"),
            TestL2Type::Arbitrum => write!(f, "arbitrum"),
        }
    }
}

impl TestL2Type {
    pub fn namespace(self) -> String {
        match self {
            TestL2Type::Optimism => {
                "0x00000000000000000000000000000000000000dff1031a5a802ae32d54".into()
            }
            TestL2Type::Arbitrum => {
                "0x00000000000000000000000000000000000000ca1de12a1f4dbe943b6b".into()
            }
        }
    }

    pub fn l2_config(self, l2_api_url: String) -> L2Config {
        match self {
            TestL2Type::Optimism => L2Config {
                l2_chain_type: L2Type::Optimism,
                l2_chain_id: 251202,
                l2_api_url,
                l2_blockscout_url: "https://foundation-network.cloud.blockscout.com/".to_string(),
                l1_chain_id: None,
                request_timeout: time::Duration::from_secs(5),
                request_retries: 1,
            },
            TestL2Type::Arbitrum => L2Config {
                l2_chain_type: L2Type::Arbitrum,
                l2_chain_id: 1918988905,
                l2_api_url,
                l2_blockscout_url: "https://rari-testnet.cloud.blockscout.com/".to_string(),
                l1_chain_id: Some(12),
                request_timeout: time::Duration::from_secs(5),
                request_retries: 1,
            },
        }
    }
}

#[tokio::test]
async fn test_optimism_router() {
    let height = 760960;
    let commitment = "0xf1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea";
    let blockscout_response_body = json!({
      "batch_data_container": "in_celestia",
      "blobs": [
        {
          "commitment": "0xf1a51990b5a358a2376e85648b489138ca38533e2b86e0283d41ceeebcf058ea",
          "height": 760960,
          "l1_timestamp": "2023-12-20T10:17:12.000000Z",
          "l1_transaction_hash": "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
          "namespace": "0x00000000000000000000000000000000000000dff1031a5a802ae32d54"
        },
        {
          "commitment": "0x3834d3a92ede97db07defc291c848b6085389c236f88c52b67a933271f316fee",
          "height": 760961,
          "l1_timestamp": "2023-12-20T10:17:24.000000Z",
          "l1_transaction_hash": "0x9abc0df13890e8c0818b448b15056ecd96368dc2b4f625c1232285e05e5b3826",
          "namespace": "0x00000000000000000000000000000000000000dff1031a5a802ae32d54"
        }
      ],
      "number": 5,
      "l1_timestamp": "2023-12-20T10:17:24.000000Z",
      "l1_transaction_hashes": [
        "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
        "0x9abc0df13890e8c0818b448b15056ecd96368dc2b4f625c1232285e05e5b3826"
      ],
      "l2_start_block_number": 29996,
      "l2_end_block_number": 33082,
      "transactions_count": 1
    });

    let batch_metadata = test_api_works(
        TestL2Type::Optimism,
        height,
        commitment,
        blockscout_response_body,
    )
    .await;

    assert_eq!(batch_metadata.chain_type, L2Type::Optimism);
    assert_eq!(batch_metadata.l2_chain_id, 251202);
    assert_eq!(batch_metadata.l2_batch_id, "5");
    assert_eq!(batch_metadata.l2_start_block, 29996);
    assert_eq!(batch_metadata.l2_end_block, 33082);
    assert_eq!(batch_metadata.l2_batch_tx_count, 1);
    assert_eq!(
        batch_metadata.l2_blockscout_url,
        "https://foundation-network.cloud.blockscout.com/batches/5",
    );
    assert_eq!(
        batch_metadata.l1_tx_hash,
        "0xf41211e966ec23032dde713d1f775ae5cb07dc5e15951281e6844d74cc02a930",
    );
    assert_eq!(batch_metadata.l1_tx_timestamp, 1703067444);
    assert_eq!(batch_metadata.l1_chain_id, None);
    assert_eq!(batch_metadata.related_blobs.len(), 1);
}

#[tokio::test]
async fn test_arbitrum_router() {
    let height = 2282948;
    let commitment = "0x5f4dece44a8b054de4fd1837c2fc0aef0e68b2f39d55ec0658bfb659ba7bb8e9";
    let blockscout_response_body = json!({
      "after_acc_hash":"0x2e503ef5d9f007bfbc710b44068a0806d0f9672c77ee646207361851f9d05820",
      "before_acc_hash":"0xf520a872900633ee12da651d817db40596ce5c5fb6711c3cfb7c870051a1d857",
      "commitment_transaction":{
        "block_number":20011092,
        "hash":"0x6090384cc3f60874ee6e4bcd213629f0b68ef7607fd012714905ebc28c28078e",
        "status":"finalized",
        "timestamp":"2024-06-03T11:47:35.000000Z"
      },
      "data_availability": {
        "batch_data_container": "in_celestia",
        "transaction_commitment": "0x5f4dece44a8b054de4fd1837c2fc0aef0e68b2f39d55ec0658bfb659ba7bb8e9",
        "height": 2282948
      },
      "end_block_number":217962052,
      "number":610699,
      "start_block_number":217961563,
      "transactions_count":3061
    });

    let batch_metadata = test_api_works(
        TestL2Type::Arbitrum,
        height,
        commitment,
        blockscout_response_body,
    )
    .await;

    assert_eq!(batch_metadata.chain_type, L2Type::Arbitrum);
    assert_eq!(batch_metadata.l2_chain_id, 1918988905);
    assert_eq!(batch_metadata.l2_batch_id, "610699");
    assert_eq!(batch_metadata.l2_start_block, 217961563);
    assert_eq!(batch_metadata.l2_end_block, 217962052);
    assert_eq!(batch_metadata.l2_batch_tx_count, 3061);
    assert_eq!(
        batch_metadata.l2_blockscout_url,
        "https://rari-testnet.cloud.blockscout.com/batches/610699",
    );
    assert_eq!(
        batch_metadata.l1_tx_hash,
        "0x6090384cc3f60874ee6e4bcd213629f0b68ef7607fd012714905ebc28c28078e",
    );
    assert_eq!(batch_metadata.l1_tx_timestamp, 1717415255);
    assert_eq!(batch_metadata.l1_chain_id, Some(12));
    assert_eq!(batch_metadata.related_blobs.len(), 0);
}

mod blockscout_api_changes_test {
    use super::*;

    #[tokio::test]
    async fn test_optimism_router_blockscout_v800() {
        let height = 4311103;
        let commitment = "0xaa28cd2dd708c3e255fc8b414c8eee07233c27e2fd4711e62ec2879585da06e7";
        let blockscout_response_body = json!({
          "number": 3460,
          "transactions_count": 804,
          "transaction_count": 804,
          "l1_transaction_hashes": [
            "0x8bf362e81564b4b02d5622280d97d38b948f778cf5475251c9d9b91d0514b7aa"
          ],
          "l1_timestamp": "2025-03-05T16:37:47.000000Z",
          "blobs": [
            {
              "commitment": "0xaa28cd2dd708c3e255fc8b414c8eee07233c27e2fd4711e62ec2879585da06e7",
              "height": 4311103,
              "l1_timestamp": "2025-03-05T16:37:47.000000Z",
              "l1_transaction_hash": "0x8bf362e81564b4b02d5622280d97d38b948f778cf5475251c9d9b91d0514b7aa",
              "namespace": "0x00000000000000000000000000000000000000dff1031a5a802ae32d54"
            }
          ],
          "batch_data_container": "in_celestia",
          "internal_id": 3460,
          "l2_block_end": 3613640,
          "l2_block_start": 3612870,
          "l2_end_block_number": 3613640,
          "l2_start_block_number": 3612870
        });

        test_api_works(
            TestL2Type::Optimism,
            height,
            commitment,
            blockscout_response_body,
        )
        .await;
    }

    #[tokio::test]
    async fn test_optimism_router_blockscout_v900() {
        let height = 4311103;
        let commitment = "0xaa28cd2dd708c3e255fc8b414c8eee07233c27e2fd4711e62ec2879585da06e7";
        let blockscout_response_body = json!({
          "number": 3460,
          "transactions_count": 804,
          "l1_timestamp": "2025-03-05T16:37:47.000000Z",
          "l1_transaction_hashes": [
            "0x8bf362e81564b4b02d5622280d97d38b948f778cf5475251c9d9b91d0514b7aa"
          ],
          "blobs": [
            {
              "commitment": "0xaa28cd2dd708c3e255fc8b414c8eee07233c27e2fd4711e62ec2879585da06e7",
              "height": 4311103,
              "l1_timestamp": "2025-03-05T16:37:47.000000Z",
              "l1_transaction_hash": "0x8bf362e81564b4b02d5622280d97d38b948f778cf5475251c9d9b91d0514b7aa",
              "namespace": "0x00000000000000000000000000000000000000dff1031a5a802ae32d54"
            }
          ],
          "batch_data_container": "in_celestia",
          "l2_end_block_number": 3613640,
          "l2_start_block_number": 3612870
        });

        test_api_works(
            TestL2Type::Optimism,
            height,
            commitment,
            blockscout_response_body,
        )
        .await;
    }

    #[tokio::test]
    async fn test_arbitrum_router_blockscout_v800() {
        let height = 7650995;
        let commitment = "0xd0cfeee2cf20e5bf65e58ed133fac04f63a1f4102ec7440d092a66b2ef601d04";
        let blockscout_response_body = json!({
          "after_acc": "0x1a95218f202f805be78bb32b07cd03b3ed2ff4046444e27f1b162b136126d674",
          "after_acc_hash": "0x1a95218f202f805be78bb32b07cd03b3ed2ff4046444e27f1b162b136126d674",
          "before_acc": "0x6bde935f71d8a26792707d8470ee53b90d498f14288dfd89203a58b266bb4681",
          "before_acc_hash": "0x6bde935f71d8a26792707d8470ee53b90d498f14288dfd89203a58b266bb4681",
          "commitment_transaction": {
            "block_number": 184162105,
            "hash": "0xb58dcd07abec0f26e08c534af7c3a7b2d0965810257ec034e5142616238ac417",
            "status": "finalized",
            "timestamp": "2025-08-15T06:45:47.000000Z"
          },
          "data_availability": {
            "batch_data_container": "in_celestia",
            "height": 7650995,
            "transaction_commitment": "0xd0cfeee2cf20e5bf65e58ed133fac04f63a1f4102ec7440d092a66b2ef601d04"
          },
          "end_block": 817757,
          "end_block_number": 817757,
          "number": 89798,
          "start_block": 817756,
          "start_block_number": 817756,
          "transactions_count": 4
        });

        test_api_works(
            TestL2Type::Arbitrum,
            height,
            commitment,
            blockscout_response_body,
        )
        .await;
    }

    #[tokio::test]
    async fn test_arbitrum_router_blockscout_v900() {
        let height = 7650995;
        let commitment = "0xd0cfeee2cf20e5bf65e58ed133fac04f63a1f4102ec7440d092a66b2ef601d04";
        let blockscout_response_body = json!({
          "after_acc_hash": "0x1a95218f202f805be78bb32b07cd03b3ed2ff4046444e27f1b162b136126d674",
          "before_acc_hash": "0x6bde935f71d8a26792707d8470ee53b90d498f14288dfd89203a58b266bb4681",
          "commitment_transaction": {
            "block_number": 184162105,
            "hash": "0xb58dcd07abec0f26e08c534af7c3a7b2d0965810257ec034e5142616238ac417",
            "status": "finalized",
            "timestamp": "2025-08-15T06:45:47.000000Z"
          },
          "data_availability": {
            "batch_data_container": "in_celestia",
            "height": 7650995,
            "transaction_commitment": "0xd0cfeee2cf20e5bf65e58ed133fac04f63a1f4102ec7440d092a66b2ef601d04"
          },
          "end_block_number": 817757,
          "number": 89798,
          "start_block_number": 817756,
          "transactions_count": 4
        });

        test_api_works(
            TestL2Type::Arbitrum,
            height,
            commitment,
            blockscout_response_body,
        )
        .await;
    }
}

async fn test_api_works(
    l2_type: TestL2Type,
    height: u64,
    commitment: &str,
    blockscout_response_body: serde_json::Value,
) -> L2BatchMetadata {
    let commitment = Bytes::from_str(commitment).unwrap();

    let blockscout_request_path = format!(
        "api/v2/{l2_type}/batches/da/celestia/{height}/{}",
        commitment.to_hex()
    );
    let mock_server =
        create_blockscout_mock(&blockscout_request_path, blockscout_response_body).await;
    let l2_router = create_test_router(mock_server);

    let batch_metadata = l2_router
        .get_l2_batch_metadata(
            height,
            &Bytes::from_str(&l2_type.namespace()).unwrap(),
            &commitment,
        )
        .await
        .unwrap()
        .unwrap();
    batch_metadata
}

async fn create_blockscout_mock(
    request_path: &str,
    response_body: serde_json::Value,
) -> MockServer {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(request_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(response_body)))
        .mount(&mock_server)
        .await;
    mock_server
}

fn create_test_router(blockscout_mock_server: MockServer) -> L2Router {
    let mut routes: HashMap<String, L2Config> = HashMap::new();
    routes.insert(
        TestL2Type::Optimism.namespace(),
        TestL2Type::Optimism.l2_config(blockscout_mock_server.uri()),
    );
    routes.insert(
        TestL2Type::Arbitrum.namespace(),
        TestL2Type::Arbitrum.l2_config(blockscout_mock_server.uri()),
    );

    L2Router::new(routes).unwrap()
}
