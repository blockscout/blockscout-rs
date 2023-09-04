// use actix_web::{
//     test::{self, TestRequest},
//     App,
// };
// use pretty_assertions::assert_eq;
// use serde_json::json;
// use smart_contract_verifier_http::{
//     configure_router, AppRouter, Settings, VerificationResponse, VerificationStatus,
// };
// use std::sync::Arc;
//
// #[actix_rt::test]
// async fn should_return_200() {
//     let mut settings = Settings::default();
//     settings.solidity.enabled = false;
//     let app_router = Arc::new(
//         AppRouter::new(settings)
//             .await
//             .expect("couldn't initialize the app"),
//     );
//     let app = test::init_service(App::new().configure(configure_router(&*app_router))).await;
//
//     let metadata = include_str!("contracts/storage/metadata.json");
//     let source = include_str!("contracts/storage/source.sol");
//     let request_body = json!({
//         // relies on the fact that the Ethereum Testnet Goerli has this contract
//         // https://eth-goerli.blockscout.com/address/0x6da5E8Cd88641dd371F3ED7737664ea86B3C3ec8
//         "address": "0x6da5E8Cd88641dd371F3ED7737664ea86B3C3ec8",
//         "chain": "5",
//         "files": {
//             "source.sol": source,
//             "metadata.json": metadata,
//         }
//     });
//
//     let resp = TestRequest::post()
//         .uri("/api/v1/sourcify/verify")
//         .set_json(&request_body)
//         .send_request(&app)
//         .await;
//
//     assert!(
//         resp.status().is_success(),
//         "failed to verify contract, status is {}",
//         resp.status()
//     );
//
//     let body: serde_json::Value = test::read_body_json(resp).await;
//     assert_eq!(
//         body,
//         serde_json::json!({
//             "message": "OK",
//             "result": {
//                 "file_name": "contracts/1_Storage.sol",
//                 "contract_name": "Storage",
//                 "compiler_version": "0.8.7+commit.e28d00a7",
//                 "evm_version": "london",
//                 "constructor_arguments": null,
//                 "optimization": false,
//                 "optimization_runs": 200,
//                 "contract_libraries": {},
//                 "abi": "[{\"inputs\":[],\"name\":\"retrieve\",\"outputs\":[{\"internalType\":\"uint256\",\"name\":\"\",\"type\":\"uint256\"}],\"stateMutability\":\"view\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"uint256\",\"name\":\"num\",\"type\":\"uint256\"}],\"name\":\"store\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]",
//                 "sources": {
//                     "contracts/1_Storage.sol": "// SPDX-License-Identifier: GPL-3.0\n\npragma solidity >=0.7.0 <0.9.0;\n\n/**\n * @title Storage\n * @dev Store & retrieve value in a variable\n * @custom:dev-run-script ./scripts/deploy_with_ethers.ts\n */\ncontract Storage {\n\n    uint256 number;\n\n    /**\n     * @dev Store value in variable\n     * @param num value to store\n     */\n    function store(uint256 num) public {\n        number = num;\n    }\n\n    /**\n     * @dev Return value \n     * @return value of 'number'\n     */\n    function retrieve() public view returns (uint256){\n        return number;\n    }\n}"
//                 },
//                 "compiler_settings": "{\"compilationTarget\":{\"contracts/1_Storage.sol\":\"Storage\"},\"evmVersion\":\"london\",\"libraries\":{},\"metadata\":{\"bytecodeHash\":\"ipfs\"},\"optimizer\":{\"enabled\":false,\"runs\":200},\"remappings\":[]}"
//             },
//             "status": "0"
//         }),
//     );
// }
//
// #[actix_rt::test]
// async fn invalid_contracts() {
//     let mut settings = Settings::default();
//     settings.solidity.enabled = false;
//     let app_router = Arc::new(
//         AppRouter::new(settings)
//             .await
//             .expect("couldn't initialize the app"),
//     );
//     let app = test::init_service(App::new().configure(configure_router(&*app_router))).await;
//
//     let metadata_content = include_str!("contracts/storage/metadata.json");
//     let source = include_str!("contracts/storage/source.sol");
//     for (request_body, error_message) in [
//         (
//             json!({
//                 // relies on fact that the Ethereum Testnet Goerli HASN'T any contract with this address
//                 "address": "0x1234567890123456789012345678901234567890",
//                 "chain": "5",
//                 "files": {
//                     "metadata.json": metadata_content,
//                     "contracts/1_Storage.sol": source,
//                 },
//             }),
//             "does not have a contract",
//         ),
//         (
//             json!({
//                 "address": "0x1234567890123456789012345678901234567890",
//                 "chain": "5",
//                 "files": {},
//             }),
//             "Metadata file not found",
//         ),
//         (
//             json!({
//                 // relies on fact that Ethereum Testnet Goerli has some contract, but it is not verified in
//                 // sourcify and `source` contains wrong source code
//                 "address": "0xD3F4730068b57d11a5Cd4252D8a9012A188C5D3B",
//                 "chain": "5",
//                 "files": {
//                     "metadata.json": metadata_content,
//                     "contracts/1_Storage.sol": source,
//                 },
//             }),
//             "deployed and recompiled bytecode don't match",
//         ),
//     ] {
//         let resp = TestRequest::post()
//             .uri("/api/v1/sourcify/verify")
//             .set_json(&request_body)
//             .send_request(&app)
//             .await;
//
//         let body: VerificationResponse = test::read_body_json(resp).await;
//
//         assert!(body.result.is_none());
//         assert_eq!(body.status, VerificationStatus::Failed);
//         assert!(
//             body.message.contains(error_message),
//             "body message: {}, expected message: {}",
//             body.message,
//             error_message
//         );
//     }
// }
