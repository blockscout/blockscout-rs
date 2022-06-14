use std::sync::Arc;

use actix_web::{
    test::{self, TestRequest},
    App,
};
use serde_json::json;
use verification::{AppRouter, Config, VerificationResponse, VerificationStatus};

#[actix_rt::test]
async fn should_return_200() {
    let mut config = Config::default();
    config.verifier.disabled = true;
    let app_router = Arc::new(
        AppRouter::new(config)
            .await
            .expect("couldn't initialize the app"),
    );
    let mut app = test::init_service(
        App::new().configure(|service_config| app_router.register_routes(service_config)),
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

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body,
        serde_json::json!({
            "message": "OK",
            "result": {
                "contract_name": "Storage",
                "compiler_version": "0.8.7+commit.e28d00a7",
                "evm_version": "london",
                "constructor_arguments": null,
                "optimization": false,
                "optimization_runs": 200,
                "contract_libraries": {},
                "abi": "[{\"inputs\":[],\"name\":\"retrieve\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"num\",\"type\":\"uint256\"}],\"name\":\"store\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]",
                "sources": {
                    "1_Storage.sol": "// SPDX-License-Identifier: GPL-3.0\n\npragma solidity >=0.7.0 <0.9.0;\n\n/**\n * @title Storage\n * @dev Store & retrieve value in a variable\n * @custom:dev-run-script ./scripts/deploy_with_ethers.ts\n */\ncontract Storage {\n\n    uint256 number;\n\n    /**\n     * @dev Store value in variable\n     * @param num value to store\n     */\n    function store(uint256 num) public {\n        number = num;\n    }\n\n    /**\n     * @dev Return value \n     * @return value of 'number'\n     */\n    function retrieve() public view returns (uint256){\n        return number;\n    }\n}"
                }
            },
            "status": "0"
        }),
    );
}

#[actix_rt::test]
async fn invalid_contracts() {
    let mut config = Config::default();
    config.verifier.disabled = true;
    let app_router = Arc::new(
        AppRouter::new(config)
            .await
            .expect("couldn't initialize the app"),
    );
    let mut app = test::init_service(
        App::new().configure(|service_config| app_router.register_routes(service_config)),
    )
    .await;

    let metadata_content = include_str!("contracts/storage/metadata.json");
    for (request_body, error_message) in [
        (
            json!({
                // relies on fact that the kovan HASN'T any contract with this address
                "address": "0x1234567890123456789012345678901234567890",
                "chain": "42",
                "files": {"metadata.json": metadata_content},
            }),
            "Kovan does not have a contract",
        ),
        (
            json!({
                "address": "0x1234567890123456789012345678901234567890",
                "chain": "42",
                "files": {},
            }),
            "Metadata file not found",
        ),
        (
            json!({
                // relies on fact that kovan has some contract, but it is not verified in
                // sourcify and `files` contains invalid source code
                "address": "0x14132d1e8f4AaFCef230f5900bf8A3fFA4CCCC22",
                "chain": "42",
                "files": {
                    "metadata.json": metadata_content,
                    "source.sol": "",
                },
            }),
            "deployed and recompiled bytecode don't match",
        ),
    ] {
        let resp = TestRequest::get()
            .uri("/api/v1/verification/sourcify")
            .set_json(&request_body)
            .send_request(&mut app)
            .await;

        let body: VerificationResponse = test::read_body_json(resp).await;

        assert!(body.result.is_none());
        assert_eq!(body.status, VerificationStatus::Failed);
        assert!(
            body.message.contains(error_message),
            "body message: {}, expected message: {}",
            body.message,
            error_message
        );
    }
}
