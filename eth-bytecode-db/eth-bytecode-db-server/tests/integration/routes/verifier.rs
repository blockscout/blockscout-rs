use crate::types::{VerifierMock, VerifierRequest, VerifierResponse, VerifierRoute};
use smart_contract_verifier_proto::{
    blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2,
    http_client::mock::{MockSolidityVerifierService, SmartContractVerifierServer},
};

macro_rules! impl_verifier_route {
    ($name:ident, $request:path, $response:path, $mock_service:path, $expect_f:ident, $add_as_service_f:ident) => {
        pub struct $name;
        impl VerifierRoute for $name {
            type Request = $request;
            type Response = $response;
            type MockService = $mock_service;
        }
        impl VerifierMock<<$name as VerifierRoute>::Request, <$name as VerifierRoute>::Response>
            for $mock_service
        {
            fn expect<TestCase>(&mut self, test_case: TestCase)
            where
                TestCase: Send
                    + Clone
                    + 'static
                    + VerifierRequest<<$name as VerifierRoute>::Request>
                    + VerifierResponse<<$name as VerifierRoute>::Response>,
            {
                let test_case_f = test_case.clone();
                self.$expect_f()
                    .withf(move |request| test_case_f.with(request))
                    .returning(move |_| Ok(tonic::Response::new(test_case.returning_const())));
            }

            fn add_as_service(
                self,
                server: SmartContractVerifierServer,
            ) -> SmartContractVerifierServer {
                server.$add_as_service_f(self)
            }
        }
    };
}

impl_verifier_route!(
    SoliditySourcesBatchVerifyStandardJson,
    smart_contract_verifier_v2::BatchVerifySolidityStandardJsonRequest,
    smart_contract_verifier_v2::BatchVerifyResponse,
    MockSolidityVerifierService,
    expect_batch_verify_standard_json,
    solidity_service
);

impl_verifier_route!(
    SoliditySourcesVerifyMultiPart,
    smart_contract_verifier_v2::VerifySolidityMultiPartRequest,
    smart_contract_verifier_v2::VerifyResponse,
    MockSolidityVerifierService,
    expect_verify_multi_part,
    solidity_service
);

impl_verifier_route!(
    SoliditySourcesVerifyStandardJson,
    smart_contract_verifier_v2::VerifySolidityStandardJsonRequest,
    smart_contract_verifier_v2::VerifyResponse,
    MockSolidityVerifierService,
    expect_verify_standard_json,
    solidity_service
);
