mod helpers;
mod test_db;

use alloy_primitives::Address;
use blockscout_service_launcher::{database, test_server};
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1 as proto;
use reqwest::StatusCode;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_quick_search() {
    let db = database!(test_db::TestMigrator);

    let quick_search_chains = vec![5, 3, 2, 1];

    let token_info_server = mock_token_info_server().await;
    let dapp_server = mock_dapp_server().await;
    let base = helpers::init_multichain_aggregator_server(db.db_url(), |mut x| {
        x.service.dapp_client.url = dapp_server.uri().parse().unwrap();
        x.service.token_info_client.url = token_info_server.uri().parse().unwrap();
        x.service.quick_search_chains = quick_search_chains.clone();
        x
    })
    .await;

    let response: proto::QuickSearchResponse =
        test_server::send_get_request(&base, "/api/v1/search:quick?q=test").await;

    assert_eq!(
        response
            .addresses
            .into_iter()
            .map(|a| a.chain_id.parse::<i64>().unwrap())
            .collect::<Vec<_>>(),
        quick_search_chains
    );
    assert_eq!(response.tokens.len(), 1);
    assert!(response.tokens[0].is_verified_contract);
    assert_eq!(response.dapps.len(), 1);
}

async fn mock_token_info_server() -> MockServer {
    let mock = MockServer::start().await;
    let payload = json!({
        "token_infos": [
            {
                "tokenAddress": Address::from_slice(&[0; 20]).to_string(),
                "chainId": "1",
                "iconUrl": "https://test1",
                "tokenName": "Test Token 1",
                "tokenSymbol": "TKN1"
            },
            {
                "tokenAddress": Address::from_slice(&[1; 20]).to_string(),
                "chainId": "10",
                "iconUrl": "https://test2",
                "tokenName": "Test Token 2",
                "tokenSymbol": "TKN2"
            }
        ]
    });
    Mock::given(method("GET"))
        .and(path("/api/v1/token-infos:search"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(payload))
        .mount(&mock)
        .await;
    mock
}

async fn mock_dapp_server() -> MockServer {
    let mock = MockServer::start().await;
    let payload = json!([
        {
            "dapp": {
                "id": "1",
                "title": "Dapp 1",
                "logo": "https://test1",
                "shortDescription": "Short description 1"
            },
            "chainId": "10"
        },
        {
            "dapp": {
                "id": "2",
                "title": "Dapp 2",
                "logo": "https://test2",
                "shortDescription": "Short description 2"
            },
            "chainId": "2"
        }
    ]);
    Mock::given(method("GET"))
        .and(path("/api/v1/marketplace/dapps:search"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(payload))
        .mount(&mock)
        .await;
    mock
}
