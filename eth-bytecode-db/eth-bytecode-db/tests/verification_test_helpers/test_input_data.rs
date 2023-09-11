use eth_bytecode_db::verification::{BytecodePart, MatchType, Source, SourceType};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    self, source, verify_response, verify_response::extra_data, VerifyResponse,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TestInputData<T> {
    pub request: T,
    pub response: VerifyResponse,
    pub source: Source,
}

pub fn input_data_1<T>(request: T, source_type: SourceType) -> TestInputData<T> {
    let verify_response = VerifyResponse {
        message: "Ok".to_string(),
        status: verify_response::Status::Success.into(),
        source: Some(v2::Source {
            file_name: "source_file1.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1.sol".into(), "content1".into()),
                ("source_file2.sol".into(), "content2".into()),
            ]),
            compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
            source_type: source::SourceType::from(source_type).into(),
            constructor_arguments: Some("cafe".to_string()),
            abi: Some("{ \"abi\": \"metadata\" }".to_string()),
            match_type: source::MatchType::Partial.into(),
            compilation_artifacts: Some("{ \"userdoc\": {\"kind\":\"user\"} }".to_string()),
            creation_input_artifacts: Some(
                "{ \"sourceMap\": \"1:2:3:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
            ),
            deployed_bytecode_artifacts: Some(
                "{ \"sourceMap\": \"10:11:12:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
            ),
        }),
        extra_data: Some(verify_response::ExtraData {
            local_creation_input_parts: vec![
                extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x0123".to_string(),
                },
                extra_data::BytecodePart {
                    r#type: "meta".to_string(),
                    data: "0x4567".to_string(),
                },
            ],
            local_deployed_bytecode_parts: vec![
                extra_data::BytecodePart {
                    r#type: "main".to_string(),
                    data: "0x89ab".to_string(),
                },
                extra_data::BytecodePart {
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
        compilation_artifacts: Some("{ \"userdoc\": {\"kind\":\"user\"} }".to_string()),
        creation_input_artifacts: Some(
            "{ \"sourceMap\": \"1:2:3:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
        ),
        deployed_bytecode_artifacts: Some(
            "{ \"sourceMap\": \"10:11:12:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
        ),
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
