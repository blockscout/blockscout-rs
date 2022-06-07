use actix_web::{
    test::{self, TestRequest},
    App,
};
use serde_json::json;
use verification::{routes, Config, VerificationResponse, VerificationStatus};

#[actix_rt::test]
async fn should_return_200() {
    let config = Config::parse().expect("Failed to parse config");
    let mut app = test::init_service(
        App::new().configure(|service_config| routes::config(service_config, config)),
    )
    .await;

    let metadata = include_str!("contracts/storage/metadata.json");
    let source = include_str!("contracts/storage/source.sol");
    let request_body = json!({
        // relies on the fact that the kovan testnet has this contract
        // https://kovan.etherscan.io/address/0xfbe36e5cad207d5fdee40e6568bb276a351f6713
        "address": "0xFBe36e5cAD207d5fDee40E6568bb276a351f6713",
        "chain": "42",
        "files": {
            "source.sol": source,
            "metadata.json": metadata,
        }
    });

    let resp = TestRequest::get()
        .uri("/api/v1/verification/sourcify")
        .set_json(&request_body)
        .send_request(&mut app)
        .await;

    assert!(
        resp.status().is_success(),
        "failed to verify contract, status is {}",
        resp.status()
    );

    let body: VerificationResponse = test::read_body_json(resp).await;
    assert_eq!(
        body.status,
        VerificationStatus::Ok,
        "invalid verification status",
    )

    // TODO: check response body and consider negative cases
}
