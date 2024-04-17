use serde_json::Value;

pub trait TestCaseRequest {
    fn route() -> &'static str;

    fn to_request(&self) -> Value;
}

pub trait TestCaseResponse {
    type Response: for<'de> serde::Deserialize<'de>;
    fn check(&self, actual_response: Self::Response);
}

pub fn from_file<Request, Response>(test_cases_dir: &str, test_case: &str) -> (Request, Response)
where
    Request: TestCaseRequest + for<'de> serde::Deserialize<'de>,
    Response: TestCaseResponse + for<'de> serde::Deserialize<'de>,
{
    let test_case_path = format!("{test_cases_dir}/{test_case}.json");
    let content = std::fs::read_to_string(test_case_path).expect("failed to read file");

    let request: Request =
        serde_json::from_str(&content).expect("invalid test case request format");
    let response: Response =
        serde_json::from_str(&content).expect("invalid test case response format");
    (request, response)
}

/***************************************************************/

pub mod batch_solidity;
pub mod transformations;
