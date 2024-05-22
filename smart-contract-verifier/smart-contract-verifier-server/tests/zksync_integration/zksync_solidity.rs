use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::zksync::solidity::ListCompilersResponse;

#[tokio::test]
async fn list_compilers() {
    const PATH: &str = "/api/v2/zksync-verifier/solidity/versions";

    let server_base = super::start().await;

    let response: ListCompilersResponse =
        blockscout_service_launcher::test_server::send_get_request(&server_base, PATH).await;

    // The list of compilers is progressively updated and depends on the fetcher type.
    // So here we will just check that one of the latest releases is available.

    assert!(
        response
            .solc_compilers
            .contains(&"v0.8.25+commit.b61c2a91".to_string()),
        "solc_compiler v0.8.25 is missed; response={:#?}",
        response.solc_compilers,
    );

    assert!(
        response
            .zk_compilers
            .contains(&"v1.4.1".to_string()),
        "zk_compiler v1.4.1 is missed; response={:#?}",
        response.zk_compilers,
    )
}
