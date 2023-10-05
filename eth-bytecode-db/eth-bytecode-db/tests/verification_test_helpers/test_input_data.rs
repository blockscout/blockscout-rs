use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::verification::{BytecodePart, MatchType, Source, SourceType};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Debug, PartialEq)]
pub struct TestInputData<T> {
    pub eth_bytecode_db_request: T,
    pub verifier_response: smart_contract_verifier_v2::VerifyResponse,
    pub eth_bytecode_db_source: Source,
}

impl<T> TestInputData<T> {
    pub fn from_verifier_source_and_extra_data(
        eth_bytecode_db_request: T,
        verifier_source: smart_contract_verifier_v2::Source,
        verifier_extra_data: smart_contract_verifier_v2::verify_response::ExtraData,
    ) -> Self {
        let source_type = Self::from_verifier_source_type(verifier_source.source_type);
        let match_type = Self::from_verifier_match_type(verifier_source.match_type);
        let creation_input_parts = Self::from_verifier_bytecode_parts(
            verifier_extra_data.local_creation_input_parts.clone(),
        );
        let deployed_bytecode_parts = Self::from_verifier_bytecode_parts(
            verifier_extra_data.local_deployed_bytecode_parts.clone(),
        );
        let eth_bytecode_db_source = Source {
            file_name: verifier_source.file_name.clone(),
            contract_name: verifier_source.contract_name.clone(),
            compiler_version: verifier_source.compiler_version.clone(),
            compiler_settings: verifier_source.compiler_settings.clone(),
            source_type,
            source_files: verifier_source.source_files.clone(),
            abi: verifier_source.abi.clone(),
            constructor_arguments: verifier_source.constructor_arguments.clone(),
            match_type,
            compilation_artifacts: verifier_source.compilation_artifacts.clone(),
            creation_input_artifacts: verifier_source.creation_input_artifacts.clone(),
            deployed_bytecode_artifacts: verifier_source.deployed_bytecode_artifacts.clone(),
            raw_creation_input: Self::raw_bytecode_input_from_bytecode_parts(
                creation_input_parts.clone(),
            ),
            raw_deployed_bytecode: Self::raw_bytecode_input_from_bytecode_parts(
                deployed_bytecode_parts.clone(),
            ),
            creation_input_parts,
            deployed_bytecode_parts,
        };

        let verifier_response = smart_contract_verifier_v2::VerifyResponse {
            message: "Ok".to_string(),
            status: smart_contract_verifier_v2::verify_response::Status::Success.into(),
            source: Some(verifier_source),
            extra_data: Some(verifier_extra_data),
        };

        Self {
            eth_bytecode_db_request,
            verifier_response,
            eth_bytecode_db_source,
        }
    }

    fn from_verifier_source_type(verifier_source_type: i32) -> SourceType {
        match smart_contract_verifier_v2::source::SourceType::from_i32(verifier_source_type)
            .expect("Invalid source type in smart contract verifier `Source`")
        {
            smart_contract_verifier_v2::source::SourceType::Unspecified => {
                panic!("Unspecified source type in smart contract verifier `Source`")
            }
            smart_contract_verifier_v2::source::SourceType::Solidity => SourceType::Solidity,
            smart_contract_verifier_v2::source::SourceType::Vyper => SourceType::Vyper,
            smart_contract_verifier_v2::source::SourceType::Yul => SourceType::Yul,
        }
    }

    fn from_verifier_match_type(verifier_match_type: i32) -> MatchType {
        match smart_contract_verifier_v2::source::MatchType::from_i32(verifier_match_type)
            .expect("Invalid match type in smart contract verifier `Source`")
        {
            smart_contract_verifier_v2::source::MatchType::Unspecified => MatchType::Unknown,
            smart_contract_verifier_v2::source::MatchType::Partial => MatchType::Partial,
            smart_contract_verifier_v2::source::MatchType::Full => MatchType::Full,
        }
    }

    fn from_verifier_bytecode_parts(
        bytecode_parts: Vec<smart_contract_verifier_v2::verify_response::extra_data::BytecodePart>,
    ) -> Vec<BytecodePart> {
        bytecode_parts
            .into_iter()
            .map(|part| {
                let data = DisplayBytes::from_str(&part.data)
                    .expect("Bytecode part data is invalid hex")
                    .to_vec();
                match part.r#type.as_str() {
                    "main" => BytecodePart::Main { data },
                    "meta" => BytecodePart::Meta { data },
                    bytecode_type => panic!("Bytecode part has an invalid type: {bytecode_type}"),
                }
            })
            .collect()
    }

    fn raw_bytecode_input_from_bytecode_parts(bytecode_parts: Vec<BytecodePart>) -> Vec<u8> {
        bytecode_parts
            .into_iter()
            .flat_map(|part| part.data_owned())
            .collect()
    }
}

pub fn input_data_1<T>(request: T, source_type: SourceType) -> TestInputData<T> {
    let verifier_source = smart_contract_verifier_v2::Source {
        file_name: "source_file1.sol".to_string(),
        contract_name: "contract_name".to_string(),
        compiler_version: "compiler_version".to_string(),
        source_files: BTreeMap::from([
            ("source_file1.sol".into(), "content1".into()),
            ("source_file2.sol".into(), "content2".into()),
        ]),
        compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
        source_type: smart_contract_verifier_v2::source::SourceType::from(source_type).into(),
        constructor_arguments: Some("cafe".to_string()),
        abi: Some("{ \"abi\": \"metadata\" }".to_string()),
        match_type: smart_contract_verifier_v2::source::MatchType::Partial.into(),
        compilation_artifacts: Some("{ \"userdoc\": {\"kind\":\"user\"} }".to_string()),
        creation_input_artifacts: Some(
            "{ \"sourceMap\": \"1:2:3:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
        ),
        deployed_bytecode_artifacts: Some(
            "{ \"sourceMap\": \"10:11:12:-:0;;;;;;;;;;;;;;;;;;;\" }".to_string(),
        ),
    };

    let verifier_extra_data = smart_contract_verifier_v2::verify_response::ExtraData {
        local_creation_input_parts: vec![
            smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                r#type: "main".to_string(),
                data: "0x0123".to_string(),
            },
            smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                r#type: "meta".to_string(),
                data: "0x4567".to_string(),
            },
        ],
        local_deployed_bytecode_parts: vec![
            smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                r#type: "main".to_string(),
                data: "0x89ab".to_string(),
            },
            smart_contract_verifier_v2::verify_response::extra_data::BytecodePart {
                r#type: "meta".to_string(),
                data: "0xcdef".to_string(),
            },
        ],
    };

    TestInputData::from_verifier_source_and_extra_data(
        request,
        verifier_source,
        verifier_extra_data,
    )
}
