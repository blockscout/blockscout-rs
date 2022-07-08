use std::str;

use actix_web::{test, web, web::Data, App};

use multichain_api_gateway::{config, handle_request, ApiEndpoints};

/// In the test we check that valid responses are returned from the API.
/// Especially we call to the same network (xdai), but to different chains (mainnet, testnet).
#[actix_web::test]
async fn check_get_requests() {
    let settings = config::BlockscoutSettings {
        base_url: "https://blockscout.com".parse().unwrap(),
        instances: vec![
            config::Instance("eth".to_string(), "mainnet".to_string()),
            config::Instance("xdai".to_string(), "mainnet".to_string()),
            config::Instance("xdai".to_string(), "testnet".to_string()),
        ],
        concurrent_requests: 1,
        request_timeout: std::time::Duration::from_secs(60),
    };

    let apis_endpoints: ApiEndpoints = settings.try_into().unwrap();

    let app = test::init_service(
        App::new()
            .app_data(Data::new(apis_endpoints.clone()))
            .default_service(web::route().to(handle_request)),
    )
    .await;

    let uri = "/api?module=block&action=getblockreward&blockno=0";

    let req = test::TestRequest::get().uri(uri).to_request();

    let bytes = test::call_and_read_body(&app, req).await;
    let str = str::from_utf8(bytes.as_ref()).unwrap().to_string();
    let actual_raw: serde_json::Value = serde_json::from_str(str.as_str()).unwrap();
    let actual = serde_json::to_string_pretty(&actual_raw).unwrap();

    let mut expected = std::fs::read_to_string("tests/res/get_requests.json").unwrap();
    // Remove trailing newline that comes from "read_to_string"
    expected.pop();

    assert_eq!(expected, actual);
}
