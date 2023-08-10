use actix_web::{test, test::TestRequest, App};
use pretty_assertions::assert_eq;
use serde_json::json;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    sourcify_verifier_actix::route_sourcify_verifier,
};
use smart_contract_verifier_server::{Settings, SourcifyVerifierService};
use std::sync::Arc;

const ROUTE: &str = "/api/v2/verifier/sourcify/sources:verify-from-etherscan";

async fn init_service() -> Arc<SourcifyVerifierService> {
    let settings = Settings::default();
    let service = SourcifyVerifierService::new(settings.sourcify, settings.extensions.sourcify)
        .await
        .expect("couldn't initialize the service");
    Arc::new(service)
}

#[tokio::test]
async fn should_return_200() {
    let address = "0x20f6a0edCE30681CDE6debAa58ed9768E42d1899".to_string();

    let service = init_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_sourcify_verifier(config, service.clone())),
    )
    .await;

    let request_body = json!({
        // relies on the fact that the Ethereum Testnet Goerli has this contract
            // https://goerli.etherscan.io/address/0x20f6a0edCE30681CDE6debAa58ed9768E42d1899#code
        "address": address,
        "chain": "5"
    });

    let resp = TestRequest::post()
        .uri(ROUTE)
        .set_json(&request_body)
        .send_request(&app)
        .await;

    assert!(
        resp.status().is_success(),
        "failed to verify contract, status is {}",
        resp.status()
    );

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(
        body,
        json!({
            "message": "OK",
            "status": "SUCCESS",
            "source": {
                "fileName": "C1.sol",
                "contractName": "C1",
                "compilerVersion": "0.8.7+commit.e28d00a7",
                "constructorArguments": null,
                "abi": "[{\"inputs\":[],\"name\":\"a\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"b\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"c\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[],\"name\":\"d\",\"outputs\":[{\"internalType\":\"address\",\"name\":\"\",\"type\":\"address\"}],\"stateMutability\":\"view\",\"type\":\"function\"}]",
                "sourceFiles": {
                    "C1.sol": "// SPDX-License-Identifier: MIT\r\npragma solidity 0.8.7;\r\n\r\ncontract C1 {\r\n    uint256 immutable public a = 0;\r\n    uint256 public b = 1202;\r\n    address public c = 0x9563Fdb01BFbF3D6c548C2C64E446cb5900ACA88;\r\n    address public d = 0x9563Fdb01BFbF3D6c548C2C64E446cb5900ACA88;\r\n}"
                },
                "compilerSettings": "{\"evmVersion\":\"london\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]}",
                "matchType": "PARTIAL",
                "sourceType": "SOLIDITY",
            },
            "extraData": {
                "localCreationInputParts": [],
                "localDeployedBytecodeParts": [],
            }
        }),
    );
}
