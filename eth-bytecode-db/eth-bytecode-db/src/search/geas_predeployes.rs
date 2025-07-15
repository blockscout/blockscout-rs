use crate::{
    search::MatchContract,
    verification::{MatchType, SourceType},
};
use blockscout_display_bytes::decode_hex;
use sea_orm::prelude::DateTime;
use std::collections::BTreeMap;
use strum::EnumIter;

#[derive(EnumIter, Clone, Debug, PartialEq)]
pub enum GeasPredeploy {
    BeaconRoots,
    HistoryStorage,
    WithdrawalRequest,
    ConsolidationRequest,
}

impl From<GeasPredeploy> for GeasPredeployDetails {
    fn from(value: GeasPredeploy) -> Self {
        match value {
            GeasPredeploy::BeaconRoots => GeasPredeployDetails::for_beacon_roots_predeploy(),
            GeasPredeploy::HistoryStorage => GeasPredeployDetails::for_history_storage_predeploy(),
            GeasPredeploy::WithdrawalRequest => {
                GeasPredeployDetails::for_withdrawal_request_predeploy()
            }
            GeasPredeploy::ConsolidationRequest => {
                GeasPredeployDetails::for_consolidation_request_predeploy()
            }
        }
    }
}

impl From<GeasPredeploy> for MatchContract {
    fn from(value: GeasPredeploy) -> Self {
        MatchContract::from(GeasPredeployDetails::from(value))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeasPredeployDetails {
    pub main_file_path: String,
    pub contract_name: String,
    pub creation_code: Vec<u8>,
    pub runtime_code: Vec<u8>,
    pub sources: BTreeMap<String, String>,
    pub compiler_version: semver::Version,
    pub abi: serde_json::Value,
}

impl GeasPredeployDetails {
    pub fn for_beacon_roots_predeploy() -> Self {
        Self {
            creation_code: decode_hex("0x60618060095f395ff33373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500").unwrap(),
            runtime_code: decode_hex("0x3373fffffffffffffffffffffffffffffffffffffffe14604d57602036146024575f5ffd5b5f35801560495762001fff810690815414603c575f5ffd5b62001fff01545f5260205ff35b5f5ffd5b62001fff42064281555f359062001fff015500").unwrap(),
            sources: BTreeMap::from([
                ("src/beacon_root/main.eas".into(), include_str!("geas_predeployes/beacon_root/main.eas").into()),
                ("src/beacon_root/ctor.eas".into(), include_str!("geas_predeployes/beacon_root/ctor.eas").into()),
            ]),
            main_file_path: "src/beacon_root/main.eas".to_string(),
            contract_name: "BeaconRootsPredeploy".to_string(),
            compiler_version: semver::Version::new(0, 2, 2),
            abi: serde_json::json!([{"type":"fallback","stateMutability":"view"}]),
        }
    }

    pub fn for_history_storage_predeploy() -> Self {
        Self {
            creation_code: decode_hex("0x60538060095f395ff33373fffffffffffffffffffffffffffffffffffffffe14604657602036036042575f35600143038111604257611fff81430311604257611fff9006545f5260205ff35b5f5ffd5b5f35611fff60014303065500").unwrap(),
            runtime_code: decode_hex("0x3373fffffffffffffffffffffffffffffffffffffffe14604657602036036042575f35600143038111604257611fff81430311604257611fff9006545f5260205ff35b5f5ffd5b5f35611fff60014303065500").unwrap(),
            sources: BTreeMap::from([
                ("src/execution_hash/main.eas".into(), include_str!("geas_predeployes/execution_hash/main.eas").into()),
                ("src/execution_hash/ctor.eas".into(), include_str!("geas_predeployes/execution_hash/ctor.eas").into()),
            ]),
            main_file_path: "src/execution_hash/main.eas".to_string(),
            contract_name: "HistoryStoragePredeploy".to_string(),
            compiler_version: semver::Version::new(0, 2, 2),
            abi: serde_json::json!([{"type":"fallback","stateMutability":"view"}]),
        }
    }

    pub fn for_withdrawal_request_predeploy() -> Self {
        Self {
            creation_code: decode_hex("0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff5f556101f880602d5f395ff33373fffffffffffffffffffffffffffffffffffffffe1460cb5760115f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff146101f457600182026001905f5b5f82111560685781019083028483029004916001019190604d565b909390049250505036603814608857366101f457346101f4575f5260205ff35b34106101f457600154600101600155600354806003026004013381556001015f35815560010160203590553360601b5f5260385f601437604c5fa0600101600355005b6003546002548082038060101160df575060105b5f5b8181146101835782810160030260040181604c02815460601b8152601401816001015481526020019060020154807fffffffffffffffffffffffffffffffff00000000000000000000000000000000168252906010019060401c908160381c81600701538160301c81600601538160281c81600501538160201c81600401538160181c81600301538160101c81600201538160081c81600101535360010160e1565b910180921461019557906002556101a0565b90505f6002555f6003555b5f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff14156101cd57505f5b6001546002828201116101e25750505f6101e8565b01600290035b5f555f600155604c025ff35b5f5ffd").unwrap(),
            runtime_code: decode_hex("0x3373fffffffffffffffffffffffffffffffffffffffe1460cb5760115f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff146101f457600182026001905f5b5f82111560685781019083028483029004916001019190604d565b909390049250505036603814608857366101f457346101f4575f5260205ff35b34106101f457600154600101600155600354806003026004013381556001015f35815560010160203590553360601b5f5260385f601437604c5fa0600101600355005b6003546002548082038060101160df575060105b5f5b8181146101835782810160030260040181604c02815460601b8152601401816001015481526020019060020154807fffffffffffffffffffffffffffffffff00000000000000000000000000000000168252906010019060401c908160381c81600701538160301c81600601538160281c81600501538160201c81600401538160181c81600301538160101c81600201538160081c81600101535360010160e1565b910180921461019557906002556101a0565b90505f6002555f6003555b5f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff14156101cd57505f5b6001546002828201116101e25750505f6101e8565b01600290035b5f555f600155604c025ff35b5f5ffd").unwrap(),
            sources: BTreeMap::from([
                ("src/withdrawals/main.eas".into(), include_str!("geas_predeployes/withdrawals/main.eas").into()),
                ("src/withdrawals/ctor.eas".into(), include_str!("geas_predeployes/withdrawals/ctor.eas").into()),
                ("src/common/fake_expo.eas".into(), include_str!("geas_predeployes/common/fake_expo.eas").into()),
            ]),
            main_file_path: "src/withdrawals/main.eas".to_string(),
            contract_name: "WithdrawalRequestPredeploy".to_string(),
            compiler_version: semver::Version::new(0, 2, 2),
            abi: serde_json::json!([{"type":"fallback","stateMutability":"payable"}]),
        }
    }

    pub fn for_consolidation_request_predeploy() -> Self {
        Self {
            creation_code: decode_hex("0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff5f5561019e80602d5f395ff33373fffffffffffffffffffffffffffffffffffffffe1460d35760115f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff1461019a57600182026001905f5b5f82111560685781019083028483029004916001019190604d565b9093900492505050366060146088573661019a573461019a575f5260205ff35b341061019a57600154600101600155600354806004026004013381556001015f358155600101602035815560010160403590553360601b5f5260605f60143760745fa0600101600355005b6003546002548082038060021160e7575060025b5f5b8181146101295782810160040260040181607402815460601b815260140181600101548152602001816002015481526020019060030154905260010160e9565b910180921461013b5790600255610146565b90505f6002555f6003555b5f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff141561017357505f5b6001546001828201116101885750505f61018e565b01600190035b5f555f6001556074025ff35b5f5ffd").unwrap(),
            runtime_code: decode_hex("0x3373fffffffffffffffffffffffffffffffffffffffe1460d35760115f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff1461019a57600182026001905f5b5f82111560685781019083028483029004916001019190604d565b9093900492505050366060146088573661019a573461019a575f5260205ff35b341061019a57600154600101600155600354806004026004013381556001015f358155600101602035815560010160403590553360601b5f5260605f60143760745fa0600101600355005b6003546002548082038060021160e7575060025b5f5b8181146101295782810160040260040181607402815460601b815260140181600101548152602001816002015481526020019060030154905260010160e9565b910180921461013b5790600255610146565b90505f6002555f6003555b5f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff141561017357505f5b6001546001828201116101885750505f61018e565b01600190035b5f555f6001556074025ff35b5f5ffd").unwrap(),
            sources: BTreeMap::from([
                ("src/consolidations/main.eas".into(), include_str!("geas_predeployes/consolidations/main.eas").into()),
                ("src/consolidations/ctor.eas".into(), include_str!("geas_predeployes/consolidations/ctor.eas").into()),
                ("src/common/fake_expo.eas".into(), include_str!("geas_predeployes/common/fake_expo.eas").into()),
            ]),
            main_file_path: "src/consolidations/main.eas".to_string(),
            contract_name: "ConsolidationRequestPredeploy".to_string(),
            compiler_version: semver::Version::new(0, 2, 2),
            abi: serde_json::json!([{"type":"fallback","stateMutability":"payable"}]),
        }
    }
}

impl From<GeasPredeployDetails> for MatchContract {
    fn from(value: GeasPredeployDetails) -> Self {
        Self {
            updated_at: DateTime::default(),
            file_name: value.main_file_path,
            contract_name: value.contract_name,
            compiler_version: format!("v{}", value.compiler_version),
            compiler_settings: serde_json::json!({}),
            source_type: SourceType::Geas,
            source_files: value.sources,
            abi: Some(value.abi.to_string()),
            constructor_arguments: None,
            match_type: MatchType::Partial,
            compilation_artifacts: Some("{}".to_string()),
            creation_input_artifacts: Some("{}".to_string()),
            deployed_bytecode_artifacts: Some("{}".to_string()),
            raw_creation_input: value.creation_code,
            raw_deployed_bytecode: value.runtime_code,
            is_blueprint: false,
            libraries: BTreeMap::new(),
        }
    }
}
