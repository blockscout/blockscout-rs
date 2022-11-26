use serde::{Deserialize, Serialize};
use smart_contract_verifier::sourcify::api::VerificationRequest;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyViaSourcifyRequest;
use std::ops::Deref;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifyViaSourcifyRequestWrapper(VerifyViaSourcifyRequest);

impl From<VerifyViaSourcifyRequest> for VerifyViaSourcifyRequestWrapper {
    fn from(inner: VerifyViaSourcifyRequest) -> Self {
        Self(inner)
    }
}

impl Deref for VerifyViaSourcifyRequestWrapper {
    type Target = VerifyViaSourcifyRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifyViaSourcifyRequestWrapper {
    pub fn new(inner: VerifyViaSourcifyRequest) -> Self {
        Self(inner)
    }

    pub fn into_inner(self) -> VerifyViaSourcifyRequest {
        self.0
    }
}

impl TryFrom<VerifyViaSourcifyRequestWrapper> for VerificationRequest {
    type Error = tonic::Status;

    fn try_from(request: VerifyViaSourcifyRequestWrapper) -> Result<Self, Self::Error> {
        let request = request.into_inner();
        Ok(Self {
            address: request.address,
            chain: request.chain,
            files: request.files,
            chosen_contract: request.chosen_contract.map(|i| i as usize),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

    #[test]
    fn try_into_verification_request() {
        let request = VerifyViaSourcifyRequest {
            address: "0x0123456789abcdef".to_string(),
            chain: "77".to_string(),
            files: BTreeMap::from([("metadata".into(), "metadata_content".into())]),
            chosen_contract: Some(2),
        };

        let verification_request: VerificationRequest =
            <VerifyViaSourcifyRequestWrapper>::from(request)
                .try_into()
                .expect("Try_into verification request failed");

        let expected = VerificationRequest {
            address: "0x0123456789abcdef".to_string(),
            chain: "77".to_string(),
            files: BTreeMap::from([("metadata".into(), "metadata_content".into())]),
            chosen_contract: Some(2),
        };

        assert_eq!(expected, verification_request);
    }
}
