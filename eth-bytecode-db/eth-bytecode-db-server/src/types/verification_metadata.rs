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

        let chain_id = from_optional_string(value.chain_id, "metadata.chain_id")?;
        let contract_address = from_optional_string::<DisplayBytes>(
            value.contract_address,
            "metadata.contract_address",
        )?
        .map(|v| v.0);
        let transaction_hash = from_optional_string::<DisplayBytes>(
            value.transaction_hash,
            "metadata.transaction_hash",
        )?
        .map(|v| v.0);
        let deployer =
            from_optional_string::<DisplayBytes>(value.deployer, "metadata.deployer")?.map(|v| v.0);
        let creation_code =
            from_optional_string::<DisplayBytes>(value.creation_code, "metadata.creation_code")?
                .map(|v| v.0);
        let runtime_code =
            from_optional_string::<DisplayBytes>(value.runtime_code, "metadata.runtime_code")?
                .map(|v| v.0);

        Ok(verification::VerificationMetadata {
            chain_id,
            contract_address,
            transaction_hash,
            block_number: value.block_number,
            transaction_index: value.transaction_index,
            deployer,
            creation_code,
            runtime_code,
        })
    }
}

fn from_optional_string<T: FromStr>(
    value: Option<String>,
    arg_name: &str,
) -> Result<Option<T>, tonic::Status> {
    value
        .map(|v| {
            T::from_str(&v)
                .map_err(|_err| tonic::Status::invalid_argument(format!("Invalid {arg_name}")))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_proto_to_verification_metadata() {
        let bytes_from_str = |s: &str| DisplayBytes::from_str(s).unwrap().0;

        let proto_type = proto::VerificationMetadata {
            chain_id: Some("1".into()),
            contract_address: Some("0xcafecafecafecafecafecafecafecafecafecafe".into()),
            transaction_hash: Some(
                "0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f".into(),
            ),
            block_number: Some(1),
            transaction_index: Some(1),
            deployer: Some("0x000102030405060708090a0b0c0d0e0f10111213".into()),
            creation_code: Some("0xcafecafecafecafe1234567890abcdef".into()),
            runtime_code: Some("0x1234567890abcdef".into()),
        };

        let expected = verification::VerificationMetadata {
            chain_id: Some(1),
            contract_address: Some(bytes_from_str("0xcafecafecafecafecafecafecafecafecafecafe")),
            transaction_hash: Some(bytes_from_str(
                "0x000102030405060708090a0b0c0d0e0f000102030405060708090a0b0c0d0e0f",
            )),
            block_number: Some(1),
            transaction_index: Some(1),
            deployer: Some(
                DisplayBytes::from_str("0x000102030405060708090a0b0c0d0e0f10111213")
                    .unwrap()
                    .0,
            ),
            creation_code: Some(bytes_from_str("0xcafecafecafecafe1234567890abcdef")),
            runtime_code: Some(bytes_from_str("0x1234567890abcdef")),
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
