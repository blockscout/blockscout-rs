use blockscout_display_bytes::Bytes as DisplayBytes;
use eth_bytecode_db::verification::{MatchType, SourceType};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_proto_v2;
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Debug, PartialEq)]
pub struct TestInputData {
    pub verifier_response: smart_contract_verifier_proto_v2::VerifyResponse,
    pub eth_bytecode_db_response: eth_bytecode_db_v2::VerifyResponse,
}

impl TestInputData {
    pub fn creation_input(&self) -> Option<String> {
        let bytes = self
            .verifier_response
            .extra_data
            .as_ref()?
            .local_creation_input_parts
            .iter()
            .flat_map(|part| DisplayBytes::from_str(&part.data).unwrap().to_vec())
            .collect::<Vec<_>>();
        Some(DisplayBytes::from(bytes).to_string())
    }

    pub fn deployed_bytecode(&self) -> Option<String> {
        let bytes = self
            .verifier_response
            .extra_data
            .as_ref()?
            .local_deployed_bytecode_parts
            .iter()
            .flat_map(|part| DisplayBytes::from_str(&part.data).unwrap().to_vec())
            .collect::<Vec<_>>();
        Some(DisplayBytes::from(bytes).to_string())
    }

    pub fn set_creation_input_metadata_hash(&mut self, metadata_hash: &str) {
        let in_bytes = hex::decode(metadata_hash).expect("Invalid metadata hash");
        assert_eq!(34, in_bytes.len(), "Invalid metadata hash length");
        self.verifier_response
            .extra_data
            .as_mut()
            .expect("creation input is missing")
            .local_creation_input_parts
            .iter_mut()
            .for_each(|part| {
                (part.r#type == "meta")
                    .then(|| part.data.replace_range(18..18 + 68, metadata_hash));
            });
    }

    pub fn add_source_file(&mut self, file_name: String, content: String) {
        self.verifier_response
            .source
            .as_mut()
            .unwrap()
            .source_files
            .insert(file_name.clone(), content.clone());
        self.eth_bytecode_db_response
            .source
            .as_mut()
            .unwrap()
            .source_files
            .insert(file_name, content);
    }
}

pub fn basic(source_type: SourceType, match_type: MatchType) -> TestInputData {
    let smart_contract_verifier_match_type = match match_type {
        MatchType::Unknown => smart_contract_verifier_proto_v2::source::MatchType::Unspecified,
        MatchType::Partial => smart_contract_verifier_proto_v2::source::MatchType::Partial,
        MatchType::Full => smart_contract_verifier_proto_v2::source::MatchType::Full,
    };
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
            compiler_settings: "{\"language\":\"Solidity\"}".to_string(),
            source_type: smart_contract_verifier_proto_v2::source::SourceType::from(source_type)
                .into(),
            constructor_arguments: None,
            abi: Some("[]".to_string()),
            match_type: smart_contract_verifier_match_type.into(),
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
                        data: "0xa2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033".to_string(),
                    },
                ],
                local_deployed_bytecode_parts: vec![
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "main".to_string(),
                        data: "0x89ab".to_string(),
                    },
                    smart_contract_verifier_proto_v2::verify_response::extra_data::BytecodePart {
                        r#type: "meta".to_string(),
                        data: "0xa2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033".to_string(),
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
    let eth_bytecode_db_match_type = match match_type {
        MatchType::Unknown => eth_bytecode_db_v2::source::MatchType::Unspecified,
        MatchType::Partial => eth_bytecode_db_v2::source::MatchType::Partial,
        MatchType::Full => eth_bytecode_db_v2::source::MatchType::Full,
    };
    let eth_bytecode_db_response = eth_bytecode_db_v2::VerifyResponse {
        message: "OK".to_string(),
        status: eth_bytecode_db_v2::verify_response::Status::Success.into(),
        source: Some(eth_bytecode_db_v2::Source {
            file_name: "source_file1.sol".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "compiler_version".to_string(),
            compiler_settings: "{\"language\":\"Solidity\"}".to_string(),
            source_type: eth_bytecode_db_source_type.into(),
            source_files: BTreeMap::from([
                ("source_file1.sol".into(), "content1".into()),
                ("source_file2.sol".into(), "content2".into()),
            ]),
            abi: Some("[]".to_string()),
            constructor_arguments: None,
            match_type: eth_bytecode_db_match_type.into(),
        }),
    };

    TestInputData {
        verifier_response,
        eth_bytecode_db_response,
    }
}
