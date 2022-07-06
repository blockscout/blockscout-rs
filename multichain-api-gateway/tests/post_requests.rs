use multichain_api_gateway::{config, handle_request};
use std::str;

use actix_web::{test, web, web::Data, App};
use multichain_api_gateway::ApiEndpoints;

/// Check that valid responses are returned from the API via POST request.
#[actix_web::test]
async fn make_post_requests() {
    let settings = config::BlockscoutSettings {
        base_url: "https://blockscout.com".parse().unwrap(),
        instances: vec![
            config::Instance("eth".to_string(), "mainnet".to_string()),
            config::Instance("poa".to_string(), "sokol".to_string()),
        ],
        concurrent_requests: 1,
    };

    let apis_endpoints: ApiEndpoints = settings.try_into().unwrap();

    let app = test::init_service(
        App::new()
            .app_data(Data::new(apis_endpoints.clone()))
            .default_service(web::route().to(handle_request)),
    )
    .await;

    let uri = "/api?module=contract&action=verify";

    let data = serde_json::Map::from_iter([
        (
            String::from("addressHash"),
            serde_json::Value::from("0xc63BB6555C90846afACaC08A0F0Aa5caFCB382a1"),
        ),
        (
            String::from("compilerVersion"),
            serde_json::Value::from("v0.5.4+commit.9549d8ff"),
        ),
        (
            String::from("contractSourceCode"),
            serde_json::Value::from("pragma solidity ^0.5.4; contract Test { }"),
        ),
        (String::from("name"), serde_json::Value::from("Test")),
        (String::from("optimization"), serde_json::Value::from(false)),
    ]);

    let req = test::TestRequest::post()
        .uri(uri)
        .set_json(data)
        .to_request();

    let bytes = test::call_and_read_body(&app, req).await;
    let str = str::from_utf8(bytes.as_ref()).unwrap().to_string();

    let actual_raw: serde_json::Value = serde_json::from_str(str.as_str()).unwrap();
    let actual = serde_json::to_string_pretty(&actual_raw).unwrap();

    let mut expected = std::fs::read_to_string("tests/res/post_requests.json").unwrap();
    // Remove trailing newline that comes from "read_to_string"
    expected.pop();

    assert_eq!(expected, actual);
}
