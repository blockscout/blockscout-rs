use std::net::SocketAddr;
use crate::blockscout::smart_contract_verifier::v2 as proto;
use wiremock::matchers;

pub struct Mock {
    inner: wiremock::MockServer,
}

impl Mock {
    pub fn address(&self) -> &SocketAddr {
        self.inner.address()
    }
}

pub struct MockBuilder {
    inner: wiremock::MockServer,
}

impl MockBuilder {
    pub async fn new() -> Self {
        Self { inner: wiremock::MockServer::start().await }
    }

    pub fn build(self) -> Mock {
        Mock { inner: self.inner }
    }
}

pub mod solidity_verifier_client {
    use super::{MockBuilder, matchers, proto};

    pub async fn verify_multi_part(mock_builder: &mut MockBuilder, request: proto::VerifySolidityMultiPartRequest, response: proto::VerifyResponse) {
        let mock = wiremock::Mock::given(matchers::method("POST"))
            .and(matchers::path("/api/v2/verifier/solidity/sources:verify-multi-part"))
            .and(matchers::body_json(request))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(response))
            .up_to_n_times(1)
            .expect(1);

        mock_builder.inner.register(mock).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::{Config, Client, self};

    #[tokio::test]
    async fn solidity_verify_multi_part() {
        let mut mock_builder = MockBuilder::new().await;
        let request = proto::VerifySolidityMultiPartRequest::default();
        let response = proto::VerifyResponse::default();
        solidity_verifier_client::verify_multi_part(&mut mock_builder, request.clone(), response.clone()).await;
        let mock = mock_builder.build();

        let address = mock.address();
        let client = Client::new(Config::builder(format!("http://{address}/")).build());
        let actual_response = http_client::solidity_verifier_client::verify_multi_part(&client, request.clone()).await.expect("sending http request");

        assert_eq!(response, actual_response, "Invalid response");
    }
}