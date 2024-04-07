mod types;
mod verifier_alliance;
mod verifier_service;

/************************************************/

use types::{TestCaseRequest, TestCaseResponse, TestCaseRoute};

// async fn test_setup<Request: TestCaseRequest>(test_case: &Request) -> ServiceResponse {
//     let service = global_service().await;
//     let app = test::init_service(
//         App::new().configure(|config| route_solidity_verifier(config, service.clone())),
//     )
//         .await;
//
//     TestRequest::post()
//         .uri(Request::route())
//         .set_json(&test_case.to_request())
//         .send_request(&app)
//         .await
// }
//
// async fn test_success<Request, Response>(test_case_request: &Request, test_case_response: &Response)
//     where
//         Request: TestCaseRequest,
//         Response: TestCaseResponse,
// {
//     let response = test_setup(test_case_request).await;
//     if !response.status().is_success() {
//         let status = response.status();
//         let body = read_body(response).await;
//         let message = from_utf8(&body).expect("Read body as UTF-8");
//         panic!("Invalid status code (success expected). Status: {status}. Messsage: {message}")
//     }
//
//     test_case_response.check(read_body_json(response).await);
// }
