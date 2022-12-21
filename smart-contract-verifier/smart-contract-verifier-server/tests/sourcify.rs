use actix_web::{test, test::TestRequest, App};
use pretty_assertions::assert_eq;
use serde_json::json;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    sourcify_verifier_actix::route_sourcify_verifier, VerifyResponse,
};
use smart_contract_verifier_server::{Settings, SourcifyVerifierService};
use std::sync::Arc;
use tokio::sync::OnceCell;

const ROUTE: &str = "/verifier/sourcify/sources:verify";

async fn global_service() -> &'static Arc<SourcifyVerifierService> {
    static SERVICE: OnceCell<Arc<SourcifyVerifierService>> = OnceCell::const_new();
    SERVICE
        .get_or_init(|| async {
            let settings = Settings::default();
            let service =
                SourcifyVerifierService::new(settings.sourcify, settings.extensions.sourcify)
                    .await
                    .expect("couldn't initialize the service");
            Arc::new(service)
        })
        .await
}

// #[rstest::rstest]
// #[case("0x1277E7D253e0c073418B986b8228BF282554cA5e", "FULL")]
// #[case("0xec979FF845de38501bAE33a70C981fa3C65C08c7", "PARTIAL")]
#[tokio::test]
// async fn should_return_200(#[case] address: String, #[case] match_type: String) {
async fn should_return_200() {
    let address = "0x1277E7D253e0c073418B986b8228BF282554cA5e";
    let match_type = "FULL";
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_sourcify_verifier(config, service.clone())),
    )
    .await;

    let metadata = include_str!("contracts/storage/metadata.json");
    let source = include_str!("contracts/storage/source.sol");
    let request_body = json!({
        // relies on the fact that the sokol testnet has this contract
        // https://blockscout.com/poa/sokol/address/0x1277E7D253e0c073418B986b8228BF282554cA5e
        "address": address,
        "chain": "77",
        "files": {
            "source.sol": source,
            "metadata.json": metadata,
        }
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
                "fileName": "contracts/1_Storage.sol",
                "contractName": "Storage",
                "compilerVersion": "0.8.7+commit.e28d00a7",
                "constructorArguments": null,
                "abi": "[{\"inputs\":[],\"name\":\"retrieve\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"num\",\"type\":\"uint256\"}],\"name\":\"store\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]",
                "sourceFiles": {
                    "contracts/1_Storage.sol": "// SPDX-License-Identifier: GPL-3.0\n\npragma solidity >=0.7.0 <0.9.0;\n\n/**\n * @title Storage\n * @dev Store & retrieve value in a variable\n * @custom:dev-run-script ./scripts/deploy_with_ethers.ts\n */\ncontract Storage {\n\n    uint256 number;\n\n    /**\n     * @dev Store value in variable\n     * @param num value to store\n     */\n    function store(uint256 num) public {\n        number = num;\n    }\n\n    /**\n     * @dev Return value \n     * @return value of 'number'\n     */\n    function retrieve() public view returns (uint256){\n        return number;\n    }\n}"
                },
                "compilerSettings": "{\"compilationTarget\":{\"contracts/1_Storage.sol\":\"Storage\"},\"evmVersion\":\"london\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]}",
                "match": match_type,
                "sourceType": "SOLIDITY",
            },
            "extraData": {
                "localCreationInputParts": [],
                "localDeployedBytecodeParts": [],
            }
        }),
    );
}

#[tokio::test]
async fn invalid_contracts() {
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_sourcify_verifier(config, service.clone())),
    )
    .await;

    let metadata_content = include_str!("contracts/storage/metadata.json");
    let source = include_str!("contracts/storage/source.sol");
    for (request_body, error_message) in [
        (
            json!({
                // relies on fact that the sokol HASN'T any contract with this address
                "address": "0x1234567890123456789012345678901234567890",
                "chain": "77",
                "files": {
                    "metadata.json": metadata_content,
                    "contracts/1_Storage.sol": source,
                },
            }),
            "Sokol does not have a contract",
        ),
        (
            json!({
                "address": "0x1234567890123456789012345678901234567890",
                "chain": "77",
                "files": {},
            }),
            "Metadata file not found",
        ),
        (
            json!({
                // relies on fact that sokol has some contract, but it is not verified in
                // sourcify and `source` contains wrong source code
                "address": "0xDD00Fe656dC893863Ae537430Dd13631aA2F55F0",
                "chain": "77",
                "files": {
                    "metadata.json": metadata_content,
                    "contracts/1_Storage.sol": source,
                },
            }),
            "deployed and recompiled bytecode don't match",
        ),
    ] {
        let resp = TestRequest::post()
            .uri(ROUTE)
            .set_json(&request_body)
            .send_request(&app)
            .await;

        let body: VerifyResponse = test::read_body_json(resp).await;

        assert_eq!(body.status().as_str_name(), "FAILURE");
        assert!(body.source.is_none());
        assert!(body.extra_data.is_none());
        assert!(
            body.message.contains(error_message),
            "body message: {}, expected message: {}",
            body.message,
            error_message
        );
    }
}
