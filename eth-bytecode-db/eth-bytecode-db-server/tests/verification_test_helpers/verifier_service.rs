use async_trait::async_trait;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use smart_contract_verifier_proto::{
    blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2,
    http_client::mock::{
        MockSolidityVerifierService, MockSourcifyVerifierService, MockVyperVerifierService,
        SmartContractVerifierServer,
    },
};
use tonic::Response;

#[async_trait]
pub trait VerifierService<Request, Response> {
    const ROUTE: &'static str;

    fn add_into_service(&mut self, response: Response);

    fn build_server(self) -> SmartContractVerifierServer;
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifySolidityMultiPartRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockSolidityVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/solidity/sources:verify-multi-part";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_multi_part()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifySolidityStandardJsonRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockSolidityVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/solidity/sources:verify-standard-json";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_standard_json()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::ListCompilerVersionsRequest,
        smart_contract_verifier_v2::ListCompilerVersionsResponse,
    > for MockSolidityVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/solidity/versions";

    fn add_into_service(
        &mut self,
        response: smart_contract_verifier_v2::ListCompilerVersionsResponse,
    ) {
        self.expect_list_compiler_versions()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifyVyperMultiPartRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockVyperVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/vyper/sources:verify-multi-part";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_multi_part()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().vyper_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifyVyperStandardJsonRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockVyperVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/vyper/sources:verify-standard-json";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_standard_json()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().vyper_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::ListCompilerVersionsRequest,
        smart_contract_verifier_v2::ListCompilerVersionsResponse,
    > for MockVyperVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/vyper/versions";

    fn add_into_service(
        &mut self,
        response: smart_contract_verifier_v2::ListCompilerVersionsResponse,
    ) {
        self.expect_list_compiler_versions()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().vyper_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifySourcifyRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockSourcifyVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/sourcify/sources:verify";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }
}

#[async_trait]
impl
    VerifierService<
        eth_bytecode_db_v2::VerifyFromEtherscanSourcifyRequest,
        smart_contract_verifier_v2::VerifyResponse,
    > for MockSourcifyVerifierService
{
    const ROUTE: &'static str = "/api/v2/verifier/sourcify/sources:verify-from-etherscan";

    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_from_etherscan()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }
}
