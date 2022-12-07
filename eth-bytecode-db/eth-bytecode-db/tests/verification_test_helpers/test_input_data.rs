use eth_bytecode_db::verification::{
    BytecodePart, MatchType, Source, SourceType, VerificationRequest,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    verify_response, verify_response::result, VerifyResponse,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TestInputData<T> {
    pub request: VerificationRequest<T>,
    pub response: VerifyResponse,
    pub source: Source,
}

pub fn input_data_1<T>(
    request: VerificationRequest<T>,
    source_type: SourceType,
) -> TestInputData<T> {
    let verify_response = VerifyResponse {
        message: "Ok".to_string(),
        status: "0".to_string(),
        result: Some(verify_response::Result {
            file_name: "source_file1.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            sources: BTreeMap::from([
                ("source_file1.sol".into(), "content1".into()),
                ("source_file2.sol".into(), "content2".into()),
            ]),
            evm_version: "london".to_string(),
            optimization: Some(true),
            optimization_runs: Some(200),
            contract_libraries: BTreeMap::from([("lib1".into(), "0xcafe".into())]),
            compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
            constructor_arguments: Some("cafe".to_string()),
            abi: Some("{ \"abi\": \"metadata\" }".to_string()),
            local_creation_input_parts: vec![
                result::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x0123".to_string(),
                },
                result::BytecodePart {
                    r#type: "meta".to_string(),
                    data: "0x4567".to_string(),
                },
            ],
            local_deployed_bytecode_parts: vec![
                result::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x89ab".to_string(),
                },
                result::BytecodePart {
                    r#type: "meta".to_string(),
                    data: "0xcdef".to_string(),
                },
            ],
        }),
    };

    let source = Source {
        file_name: "source_file1.sol".to_string(),
        contract_name: "contract_name".to_string(),
        compiler_version: "compiler_version".to_string(),
        compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
        source_type,
        source_files: BTreeMap::from([
            ("source_file1.sol".into(), "content1".into()),
            ("source_file2.sol".into(), "content2".into()),
        ]),
        abi: Some("{ \"abi\": \"metadata\" }".to_string()),
        constructor_arguments: Some("cafe".to_string()),
        match_type: MatchType::Partial,
        raw_creation_input: vec![0x01u8, 0x23u8, 0x45u8, 0x67u8],
        raw_deployed_bytecode: vec![0x89u8, 0xabu8, 0xcdu8, 0xefu8],
        creation_input_parts: vec![
            BytecodePart::Main {
                data: vec![0x01u8, 0x23u8],
            },
            BytecodePart::Meta {
                data: vec![0x45u8, 0x67u8],
            },
        ],
        deployed_bytecode_parts: vec![
            BytecodePart::Main {
                data: vec![0x89u8, 0xabu8],
            },
            BytecodePart::Meta {
                data: vec![0xcdu8, 0xefu8],
            },
        ],
    };

    TestInputData {
        request,
        response: verify_response,
        source,
    }
}
