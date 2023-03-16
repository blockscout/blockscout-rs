use crate::proto;
use amplify::{From, Wrapper};
use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::verification;
use std::str::FromStr;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct VerificationMetadataWrapper(proto::VerificationMetadata);

impl TryFrom<VerificationMetadataWrapper> for verification::VerificationMetadata {
    type Error = tonic::Status;

    fn try_from(value: VerificationMetadataWrapper) -> Result<Self, Self::Error> {
        let value = value.0;
        Ok(verification::VerificationMetadata {
            chain_id: i64::from_str(&value.chain_id)
                .map_err(|_err| tonic::Status::invalid_argument("Invalid metadata.chain_id"))?,
            contract_address: DisplayBytes::from_str(&value.contract_address)
                .map_err(|_err| tonic::Status::invalid_argument("Invalid contract address"))?
                .0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_proto_to_verification_metadata() {
        let proto_type = proto::VerificationMetadata {
            chain_id: "1".into(),
            contract_address: "0xcafecafecafecafecafecafecafecafecafecafe".into(),
        };

        let expected = verification::VerificationMetadata {
            chain_id: 1,
            contract_address: DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafe")
                .unwrap()
                .0,
        };

        let wrapper: VerificationMetadataWrapper = proto_type.into();
        let result = verification::VerificationMetadata::try_from(wrapper);

        assert_eq!(
            result.expect("Valid metadata should not result in error"),
            expected,
            "Invalid metadata conversion result"
        );
    }
}
