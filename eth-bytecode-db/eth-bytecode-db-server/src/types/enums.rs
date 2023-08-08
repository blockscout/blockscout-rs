use crate::proto;
use amplify::{From, Wrapper};
use eth_bytecode_db::{search, verification};

/********** Bytecode Type **********/

#[derive(Wrapper, From, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BytecodeTypeWrapper(proto::BytecodeType);

impl TryFrom<BytecodeTypeWrapper> for verification::BytecodeType {
    type Error = tonic::Status;

    fn try_from(value: BytecodeTypeWrapper) -> Result<Self, Self::Error> {
        match value.into_inner() {
            proto::BytecodeType::Unspecified => Err(tonic::Status::invalid_argument(
                "Bytecode type is not specified",
            )),
            proto::BytecodeType::CreationInput => Ok(verification::BytecodeType::CreationInput),
            proto::BytecodeType::DeployedBytecode => {
                Ok(verification::BytecodeType::DeployedBytecode)
            }
        }
    }
}

impl TryFrom<BytecodeTypeWrapper> for search::BytecodeType {
    type Error = tonic::Status;

    fn try_from(value: BytecodeTypeWrapper) -> Result<Self, Self::Error> {
        match value.into_inner() {
            proto::BytecodeType::Unspecified => Err(tonic::Status::invalid_argument(
                "Bytecode type is not specified",
            )),
            proto::BytecodeType::CreationInput => Ok(search::BytecodeType::CreationInput),
            proto::BytecodeType::DeployedBytecode => Ok(search::BytecodeType::DeployedBytecode),
        }
    }
}

/********** Source Type **********/

#[derive(Wrapper, From, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceTypeWrapper(proto::source::SourceType);

impl From<verification::SourceType> for SourceTypeWrapper {
    fn from(value: verification::SourceType) -> Self {
        match value {
            verification::SourceType::Solidity => {
                SourceTypeWrapper::from(proto::source::SourceType::Solidity)
            }
            verification::SourceType::Vyper => {
                SourceTypeWrapper::from(proto::source::SourceType::Vyper)
            }
            verification::SourceType::Yul => {
                SourceTypeWrapper::from(proto::source::SourceType::Yul)
            }
        }
    }
}

/********** Match Type **********/

#[derive(Wrapper, From, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MatchTypeWrapper(proto::source::MatchType);

impl From<verification::MatchType> for MatchTypeWrapper {
    fn from(value: verification::MatchType) -> Self {
        match value {
            verification::MatchType::Unknown => {
                MatchTypeWrapper::from(proto::source::MatchType::Unspecified)
            }
            verification::MatchType::Partial => {
                MatchTypeWrapper::from(proto::source::MatchType::Partial)
            }
            verification::MatchType::Full => MatchTypeWrapper::from(proto::source::MatchType::Full),
        }
    }
}

impl From<sourcify::MatchType> for MatchTypeWrapper {
    fn from(value: sourcify::MatchType) -> Self {
        match value {
            sourcify::MatchType::Full => MatchTypeWrapper::from(proto::source::MatchType::Full),
            sourcify::MatchType::Partial => {
                MatchTypeWrapper::from(proto::source::MatchType::Partial)
            }
        }
    }
}

/********** Tests **********/

#[cfg(test)]
mod bytecode_type_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use tonic::Code;

    #[rstest]
    #[case(
        proto::BytecodeType::CreationInput,
        verification::BytecodeType::CreationInput
    )]
    #[case(
        proto::BytecodeType::DeployedBytecode,
        verification::BytecodeType::DeployedBytecode
    )]
    fn try_from_proto_to_verification_success(
        #[case] proto_type: proto::BytecodeType,
        #[case] verification_type: verification::BytecodeType,
    ) {
        let wrapper: BytecodeTypeWrapper = proto_type.into();
        let result = verification::BytecodeType::try_from(wrapper);

        assert_eq!(
            result.expect("Valid type should not result in error"),
            verification_type,
            "Invalid verification type"
        );
    }

    #[test]
    fn try_from_proto_unspecified_to_verification() {
        let proto_type = proto::BytecodeType::Unspecified;

        let wrapper: BytecodeTypeWrapper = proto_type.into();
        let result = verification::BytecodeType::try_from(wrapper);

        let err = result.expect_err("Unspecified should result in error");
        assert_eq!(err.code(), Code::InvalidArgument, "Invalid error code");
    }
}

#[cfg(test)]
mod source_type_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(
        verification::SourceType::Solidity,
        proto::source::SourceType::Solidity
    )]
    #[case(verification::SourceType::Vyper, proto::source::SourceType::Vyper)]
    #[case(verification::SourceType::Yul, proto::source::SourceType::Yul)]
    fn from_verification_to_proto(
        #[case] verification_type: verification::SourceType,
        #[case] proto_type: proto::source::SourceType,
    ) {
        let result = SourceTypeWrapper::from(verification_type).into_inner();
        assert_eq!(proto_type, result);
    }
}

#[cfg(test)]
mod match_type_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[case(
        verification::MatchType::Unknown,
        proto::source::MatchType::Unspecified
    )]
    #[case(verification::MatchType::Partial, proto::source::MatchType::Partial)]
    #[case(verification::MatchType::Full, proto::source::MatchType::Full)]
    fn from_verification_to_proto(
        #[case] verification_type: verification::MatchType,
        #[case] proto_type: proto::source::MatchType,
    ) {
        let result = MatchTypeWrapper::from(verification_type).into_inner();
        assert_eq!(proto_type, result);
    }

    #[rstest]
    #[case(sourcify::MatchType::Partial, proto::source::MatchType::Partial)]
    #[case(sourcify::MatchType::Full, proto::source::MatchType::Full)]
    fn from_sourcify_to_proto(
        #[case] verification_type: sourcify::MatchType,
        #[case] proto_type: proto::source::MatchType,
    ) {
        let result = MatchTypeWrapper::from(verification_type).into_inner();
        assert_eq!(proto_type, result);
    }
}
