use eth_bytecode_db::verification::SourceType;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_proto_v2;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct TestInputData {
    pub verifier_response: smart_contract_verifier_proto_v2::VerifyResponse,
    pub eth_bytecode_db_response: eth_bytecode_db_v2::VerifyResponse,
}

pub fn input_data_1(source_type: SourceType) -> TestInputData {
    let verifier_response = smart_contract_verifier_proto_v2::VerifyResponse {
        message: "Ok".to_string(),
        status: smart_contract_verifier_proto_v2::verify_response::Status::Success.into(),
        source: Some(smart_contract_verifier_proto_v2::Source {
            file_name: "source_file1.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            source_files: BTreeMap::from([
                ("source_file1.sol".into(), "content1".into()),
                ("source_file2.sol".into(), "content2".into()),
            ]),
            compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
            source_type: smart_contract_verifier_proto_v2::source::SourceType::from(source_type)
                .into(),
            constructor_arguments: Some("cafe".to_string()),
            abi: Some("{ \"abi\": \"metadata\" }".to_string()),
            match_type: smart_contract_verifier_proto_v2::source::MatchType::Partial.into(),
        }),
        extra_data: Some(
            smart_contract_verifier_proto_v2::verify_response::ExtraData {
                local_creation_input_parts: vec![
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "main".to_string(),
                        data: "0x0123".to_string(),
                    },
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "meta".to_string(),
                        data: "0x4567".to_string(),
                    },
                ],
                local_deployed_bytecode_parts: vec![
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "main".to_string(),
                        data: "0x89ab".to_string(),
                    },
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "meta".to_string(),
                        data: "0xcdef".to_string(),
                    },
                ],
            },
        ),
    };

    let eth_bytecode_db_source_type = match source_type {
        SourceType::Solidity => eth_bytecode_db_v2::source::SourceType::Solidity,
        SourceType::Vyper => eth_bytecode_db_v2::source::SourceType::Vyper,
        SourceType::Yul => eth_bytecode_db_v2::source::SourceType::Yul,
    };
    let eth_bytecode_db_response = eth_bytecode_db_v2::VerifyResponse {
        message: "OK".to_string(),
        status: eth_bytecode_db_v2::verify_response::Status::Success.into(),
        source: Some(eth_bytecode_db_v2::Source {
            file_name: "source_file1.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "{ \"language\": \"Solidity\" }".to_string(),
            source_type: eth_bytecode_db_source_type.into(),
            source_files: BTreeMap::from([
                ("source_file1.sol".into(), "content1".into()),
                ("source_file2.sol".into(), "content2".into()),
            ]),
            abi: Some("{ \"abi\": \"metadata\" }".to_string()),
            constructor_arguments: Some("cafe".to_string()),
            match_type: eth_bytecode_db_v2::source::MatchType::Partial.into(),
        }),
    };

    TestInputData {
        verifier_response,
        eth_bytecode_db_response,
    }
}
