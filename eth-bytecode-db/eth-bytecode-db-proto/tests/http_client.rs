use eth_bytecode_db_proto::{
    blockscout::eth_bytecode_db::v2 as proto,
    http_client::{
        database_client, mock, solidity_verifier_client, sourcify_verifier_client,
        verifier_alliance_client, vyper_verifier_client, Client, Config,
    },
};
use tonic::Response;

macro_rules! set_expectation {
    ($mock_service:expr, $expect:ident, $request:ty, $response:ty) => {
        $mock_service
            .$expect()
            .withf(|request| request.get_ref() == &<$request>::default())
            .returning(move |_| Ok(Response::new(<$response>::default())));
        // .once();
    };
}

async fn build_client(server: mock::EthBytecodeDbServer) -> Client {
    let server_addr = server.start().await;
    let config = Config::new(format!("http://{server_addr}"));
    Client::new(config).await
}

#[tokio::test]
async fn database_service() {
    let mock_service = {
        let mut mock_service = mock::MockDatabaseService::default();
        set_expectation!(
            mock_service,
            expect_search_sources,
            proto::SearchSourcesRequest,
            proto::SearchSourcesResponse
        );
        set_expectation!(
            mock_service,
            expect_search_sourcify_sources,
            proto::SearchSourcifySourcesRequest,
            proto::SearchSourcesResponse
        );
        set_expectation!(
            mock_service,
            expect_search_alliance_sources,
            proto::SearchAllianceSourcesRequest,
            proto::SearchSourcesResponse
        );
        set_expectation!(
            mock_service,
            expect_search_all_sources,
            proto::SearchAllSourcesRequest,
            proto::SearchAllSourcesResponse
        );
        set_expectation!(
            mock_service,
            expect_search_event_descriptions,
            proto::SearchEventDescriptionsRequest,
            proto::SearchEventDescriptionsResponse
        );
        set_expectation!(
            mock_service,
            expect_batch_search_event_descriptions,
            proto::BatchSearchEventDescriptionsRequest,
            proto::BatchSearchEventDescriptionsResponse
        );

        mock_service
    };

    let client =
        build_client(mock::EthBytecodeDbServer::new().database_service(mock_service)).await;

    assert!(
        database_client::search_sources(&client, proto::SearchSourcesRequest::default())
            .await
            .is_ok()
    );
    assert!(database_client::search_sourcify_sources(
        &client,
        proto::SearchSourcifySourcesRequest::default()
    )
    .await
    .is_ok());
    assert!(database_client::search_alliance_sources(
        &client,
        proto::SearchAllianceSourcesRequest::default()
    )
    .await
    .is_ok());
    assert!(database_client::search_all_sources(
        &client,
        proto::SearchAllSourcesRequest::default()
    )
    .await
    .is_ok());
    assert!(database_client::search_event_descriptions(
        &client,
        proto::SearchEventDescriptionsRequest::default()
    )
    .await
    .is_ok());
    assert!(database_client::batch_search_event_descriptions(
        &client,
        proto::BatchSearchEventDescriptionsRequest::default()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn solidity_verifier_service() {
    let mock_service = {
        let mut mock_service = mock::MockSolidityVerifierService::default();
        set_expectation!(
            mock_service,
            expect_verify_multi_part,
            proto::VerifySolidityMultiPartRequest,
            proto::VerifyResponse
        );
        set_expectation!(
            mock_service,
            expect_verify_standard_json,
            proto::VerifySolidityStandardJsonRequest,
            proto::VerifyResponse
        );
        set_expectation!(
            mock_service,
            expect_list_compiler_versions,
            proto::ListCompilerVersionsRequest,
            proto::ListCompilerVersionsResponse
        );

        mock_service
    };

    let client =
        build_client(mock::EthBytecodeDbServer::new().solidity_service(mock_service)).await;

    assert!(solidity_verifier_client::verify_multi_part(
        &client,
        proto::VerifySolidityMultiPartRequest::default()
    )
    .await
    .is_ok());
    assert!(solidity_verifier_client::verify_standard_json(
        &client,
        proto::VerifySolidityStandardJsonRequest::default()
    )
    .await
    .is_ok());
    assert!(solidity_verifier_client::list_compiler_versions(
        &client,
        proto::ListCompilerVersionsRequest::default()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn vyper_verifier_service() {
    let mock_service = {
        let mut mock_service = mock::MockVyperVerifierService::default();
        set_expectation!(
            mock_service,
            expect_verify_multi_part,
            proto::VerifyVyperMultiPartRequest,
            proto::VerifyResponse
        );
        set_expectation!(
            mock_service,
            expect_verify_standard_json,
            proto::VerifyVyperStandardJsonRequest,
            proto::VerifyResponse
        );
        set_expectation!(
            mock_service,
            expect_list_compiler_versions,
            proto::ListCompilerVersionsRequest,
            proto::ListCompilerVersionsResponse
        );

        mock_service
    };

    let client = build_client(mock::EthBytecodeDbServer::new().vyper_service(mock_service)).await;

    assert!(vyper_verifier_client::verify_multi_part(
        &client,
        proto::VerifyVyperMultiPartRequest::default()
    )
    .await
    .is_ok());
    assert!(vyper_verifier_client::verify_standard_json(
        &client,
        proto::VerifyVyperStandardJsonRequest::default()
    )
    .await
    .is_ok());
    assert!(vyper_verifier_client::list_compiler_versions(
        &client,
        proto::ListCompilerVersionsRequest::default()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn sourcify_verifier_service() {
    let mock_service = {
        let mut mock_service = mock::MockSourcifyVerifierService::default();
        set_expectation!(
            mock_service,
            expect_verify,
            proto::VerifySourcifyRequest,
            proto::VerifyResponse
        );
        set_expectation!(
            mock_service,
            expect_verify_from_etherscan,
            proto::VerifyFromEtherscanSourcifyRequest,
            proto::VerifyResponse
        );

        mock_service
    };

    let client =
        build_client(mock::EthBytecodeDbServer::new().sourcify_verifier_service(mock_service))
            .await;

    assert!(
        sourcify_verifier_client::verify(&client, proto::VerifySourcifyRequest::default())
            .await
            .is_ok()
    );
    assert!(sourcify_verifier_client::verify_from_etherscan(
        &client,
        proto::VerifyFromEtherscanSourcifyRequest::default()
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn verifier_alliance_service() {
    let mock_service = {
        let mut mock_service = mock::MockVerifierAllianceService::default();
        set_expectation!(
            mock_service,
            expect_batch_import_solidity_multi_part,
            proto::VerifierAllianceBatchImportSolidityMultiPartRequest,
            proto::VerifierAllianceBatchImportResponse
        );
        set_expectation!(
            mock_service,
            expect_batch_import_solidity_standard_json,
            proto::VerifierAllianceBatchImportSolidityStandardJsonRequest,
            proto::VerifierAllianceBatchImportResponse
        );

        mock_service
    };

    let client =
        build_client(mock::EthBytecodeDbServer::new().verifier_alliance_service(mock_service))
            .await;

    assert!(verifier_alliance_client::batch_import_solidity_multi_part(
        &client,
        proto::VerifierAllianceBatchImportSolidityMultiPartRequest::default()
    )
    .await
    .is_ok(),);
    assert!(
        verifier_alliance_client::batch_import_solidity_standard_json(
            &client,
            proto::VerifierAllianceBatchImportSolidityStandardJsonRequest::default()
        )
        .await
        .is_ok(),
    )
}
