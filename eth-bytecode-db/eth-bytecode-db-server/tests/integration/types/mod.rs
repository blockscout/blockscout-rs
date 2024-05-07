use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::http_client::mock::SmartContractVerifierServer;
use std::path::PathBuf;

pub trait Route {
    const ROUTE: &'static str;
    type Request: Serialize;
    type Response: for<'a> Deserialize<'a>;
    type VerifierRoute: VerifierRoute;
}

pub trait VerifierRoute {
    type Request;
    type Response;

    type MockService: VerifierMock<Self::Request, Self::Response>;
}

pub trait VerifierRequest<Req> {
    fn with(&self, request: &tonic::Request<Req>) -> bool;
}

pub trait VerifierResponse<Res> {
    fn returning_const(&self) -> Res;
}

pub trait VerifierMock<Req, Res>: Default {
    fn expect<TestCase>(&mut self, test_case: TestCase)
    where
        TestCase: Send + Clone + 'static + VerifierRequest<Req> + VerifierResponse<Res>;

    fn add_as_service(self, server: SmartContractVerifierServer) -> SmartContractVerifierServer;
}

pub trait Request<Rou: Route> {
    fn to_request(&self) -> <Rou as Route>::Request;
}

pub trait _Response<ProtoResponse> {}

pub fn from_path<Rou, TestCase>(test_case_path: &PathBuf) -> TestCase
where
    Rou: Route,
    TestCase: for<'de> serde::Deserialize<'de>,
{
    let content = std::fs::read_to_string(test_case_path).expect("failed to read file");

    serde_json::from_str(&content).expect("invalid test case format")
}

/***************************************************************/

mod artifacts;
pub mod verifier_alliance;
