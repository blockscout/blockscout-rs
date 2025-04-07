mod helpers;

use blockscout_service_launcher::{database, test_server};
use migration::Migrator;
use multichain_aggregator_logic::{repository::chains, types::chains::Chain};
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1 as proto;
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_fetch_chains() {
    let db = database!(Migrator);

    chains::upsert_many(
        db.client().as_ref(),
        vec![
            Chain {
                id: 1,
                name: Some("Chain 1".to_string()),
                explorer_url: Some("https://test1".to_string()),
                icon_url: Some("https://test1".to_string()),
            },
            Chain {
                id: 2,
                name: Some("Chain 2".to_string()),
                explorer_url: Some("https://test2".to_string()),
                icon_url: Some("https://test2".to_string()),
            },
            Chain {
                id: 3,
                name: Some("Chain 3".to_string()),
                explorer_url: Some("https://test3".to_string()),
                icon_url: Some("https://test3".to_string()),
            },
        ],
    )
    .await
    .unwrap();

    let token_info_server = mock_token_info_server(&[2, 4]).await;
    let dapp_server = mock_dapp_server(&[3, 5]).await;
    let base = helpers::init_multichain_aggregator_server(db.db_url(), |mut x| {
        x.service.dapp_client.url = dapp_server.uri().parse().unwrap();
        x.service.token_info_client.url = token_info_server.uri().parse().unwrap();
        x
    })
    .await;

    let response: proto::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains?only_active=true").await;
    assert_eq!(
        response.items,
        vec![
            proto::Chain {
                id: "2".to_string(),
                name: "Chain 2".to_string(),
                explorer_url: "https://test2".to_string(),
                icon_url: "https://test2".to_string(),
            },
            proto::Chain {
                id: "3".to_string(),
                name: "Chain 3".to_string(),
                explorer_url: "https://test3".to_string(),
                icon_url: "https://test3".to_string(),
            },
        ]
    );

    let response: proto::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains").await;
    let mut chains = response.items;
    chains.sort_by_key(|c| c.id.clone());
    assert_eq!(
        chains,
        vec![
            proto::Chain {
                id: "1".to_string(),
                name: "Chain 1".to_string(),
                explorer_url: "https://test1".to_string(),
                icon_url: "https://test1".to_string(),
            },
            proto::Chain {
                id: "2".to_string(),
                name: "Chain 2".to_string(),
                explorer_url: "https://test2".to_string(),
                icon_url: "https://test2".to_string(),
            },
            proto::Chain {
                id: "3".to_string(),
                name: "Chain 3".to_string(),
                explorer_url: "https://test3".to_string(),
                icon_url: "https://test3".to_string(),
            },
        ]
    );
}

async fn mock_token_info_server(chain_ids: &[u64]) -> MockServer {
    let mock = MockServer::start().await;
    let payload = json!({
        "chains": chain_ids
    });
    Mock::given(method("GET"))
        .and(path("/api/v1/token-infos/chains"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(payload))
        .mount(&mock)
        .await;
    mock
}

async fn mock_dapp_server(chain_ids: &[u64]) -> MockServer {
    let mock = MockServer::start().await;
    let payload = json!(chain_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<String>>());
    Mock::given(method("GET"))
        .and(path("/api/v1/marketplace/chains"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(payload))
        .mount(&mock)
        .await;
    mock
}
