use serde::Serialize;

pub trait TestCaseRoute {
    const ROUTE: &'static str;
}

pub trait TestCaseRequest<Route: TestCaseRoute> {
    type Request: Serialize;

    fn to_request(&self) -> Self::Request;
    fn setup(&self) {}
}

pub trait TestCaseResponse<ProtoResponse>
where
    ProtoResponse: for<'de> serde::Deserialize<'de>,
{
    fn check(&self, actual_response: ProtoResponse);
}

pub fn from_file<Route, TestCase, ProtoResponse>(test_cases_dir: &str, test_case: &str) -> TestCase
where
    Route: TestCaseRoute,
    TestCase:
        TestCaseRequest<Route> + TestCaseResponse<ProtoResponse> + for<'de> serde::Deserialize<'de>,
    ProtoResponse: for<'de> serde::Deserialize<'de>,
{
    let test_case_path = format!("{test_cases_dir}/{test_case}.json");
    let content = std::fs::read_to_string(test_case_path).expect("failed to read file");

    serde_json::from_str(&content).expect("invalid test case format")
}

/***************************************************************/

pub mod verifier_alliance;

// pub mod batch_solidity;
// pub mod transformations;
