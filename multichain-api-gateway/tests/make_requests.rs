use std::str;

use actix_web::{
    test, web,
    web::{Bytes, Data},
    App,
};
use pretty_assertions::assert_eq;

use multichain_api_gateway::{handle_request, settings, ApiEndpoints};

/// In the test we check that valid responses are returned from the API.
/// Especially we call to the same network (xdai), but to different chains (mainnet, testnet).
/// Also check correctness of POST/GET propagation
#[actix_web::test]
async fn check_make_requests() {
    let settings = settings::BlockscoutSettings {
        base_url: "https://blockscout.com".parse().unwrap(),
        instances: vec![
            settings::Instance("eth".to_string(), "mainnet".to_string()),
            settings::Instance("xdai".to_string(), "mainnet".to_string()),
            settings::Instance("poa".to_string(), "sokol".to_string()),
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

    let expected = std::fs::read_to_string("tests/res/request_result.json").unwrap();

    let get_request = test::TestRequest::get().uri(uri).to_request();
    check_response(
        test::call_and_read_body(&app, get_request).await,
        expected.as_str(),
    );

    let post_request = test::TestRequest::post().uri(uri).to_request();
    check_response(
        test::call_and_read_body(&app, post_request).await,
        expected.as_str(),
    );
}

fn check_response(actual: Bytes, expected: &str) {
    let actual: serde_json::Value =
        serde_json::from_slice(&actual).expect("failed to convert response to json");
    let expected: serde_json::Value =
        serde_json::from_str(expected).expect("invalid expected string");
    // check responses in json form to simplify reading error in case of inequality
    assert_eq!(actual, expected)
}
