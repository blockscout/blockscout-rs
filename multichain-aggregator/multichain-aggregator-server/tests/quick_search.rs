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
    let bens_server = mock_bens_server().await;
    let base = helpers::init_multichain_aggregator_server(db.db_url(), |mut x| {
        x.service.dapp_client.url = dapp_server.uri().parse().unwrap();
        x.service.token_info_client.url = token_info_server.uri().parse().unwrap();
        x.service.bens_client.url = bens_server.uri().parse().unwrap();
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
    assert_eq!(response.domains.len(), 0);

    let response: proto::QuickSearchResponse =
        test_server::send_get_request(&base, "/api/v1/search:quick?q=test.eth").await;

    assert_eq!(response.domains.len(), 1);

    let response: proto::QuickSearchResponse = test_server::send_get_request(
        &base,
        "/api/v1/search:quick?q=0x0000000000000000000000000000000000000500",
    )
    .await;

    assert_eq!(
        response.addresses[0].domain_info.as_ref().unwrap().name,
        "test-500.eth"
    );
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

async fn mock_bens_server() -> MockServer {
    let mock = MockServer::start().await;
    let domain_name = json!({
        "id": "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
        "name": "vitalik.eth",
        "resolved_address": {
            "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        },
        "owner": {
            "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        },
        "wrapped_owner": null,
        "registration_date": "2017-06-18T08:39:14.000Z",
        "expiry_date": null,
        "protocol": {
            "id": "ens",
            "short_name": "ENS",
            "title": "Ethereum Name Service",
            "description": "",
            "deployment_blockscout_base_url": "",
            "tld_list": ["eth"],
            "icon_url": "",
            "docs_url": ""
        }
    });
    Mock::given(method("GET"))
        .and(path("/api/v1/1/domains:lookup"))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
            "items": [domain_name]
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path(
            "/api/v1/1/addresses/0x0000000000000000000000000000000000000500",
        ))
        .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_json(json!({
            "domain": {
                "id": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "tokens": [],
                "name": "test-500.eth",
                "resolved_address": {
                    "hash": "0x0000000000000000000000000000000000000500"
                },
                "registration_date": "",
                "other_addresses": {},
                "stored_offchain": false,
                "resolved_with_wildcard": false,
            },
            "resolved_domains_count": 1
        })))
        .mount(&mock)
        .await;
    mock
}
