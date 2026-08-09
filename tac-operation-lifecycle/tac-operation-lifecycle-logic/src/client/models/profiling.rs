// SPDX-License-Identifier: LicenseRef-Blockscout

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Deserialize)]
pub struct StageProfilingV1ApiResponse {
    pub response: HashMap<String, V1OperationData>,
}

#[derive(Debug, Deserialize)]
pub struct StageProfilingV2ApiResponse {
    pub response: HashMap<String, V2OperationData>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct V1OperationData {
    pub operation_type: LegacyOperationType,
    pub meta_info: Option<OperationMetaInfo>,
    #[serde(flatten)]
    pub stages: HashMap<StageType, Stage>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct V2OperationData {
    pub operation_type: OperationRoute,
    pub status: Option<OperationStatus>,
    pub finalized: bool,
    pub rollback: bool,
    pub meta_info: Option<OperationMetaInfo>,
    #[serde(flatten)]
    pub stages: HashMap<StageType, Stage>,
}

#[derive(Clone, Debug)]
pub enum ProfilingResponse {
    V1(HashMap<String, V1OperationData>),
    V2(HashMap<String, V2OperationData>),
}

#[derive(Clone, Debug)]
pub enum SourceOperationData {
    V1(V1OperationData),
    V2(V2OperationData),
}

impl SourceOperationData {
    pub fn meta_info(&self) -> Option<&OperationMetaInfo> {
        match self {
            Self::V1(data) => data.meta_info.as_ref(),
            Self::V2(data) => data.meta_info.as_ref(),
        }
    }

    pub fn stages(&self) -> &HashMap<StageType, Stage> {
        match self {
            Self::V1(data) => &data.stages,
            Self::V2(data) => &data.stages,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LegacyOperationType {
    Pending,
    #[serde(rename = "TON-TAC-TON")]
    TonTacTon,
    #[serde(rename = "TAC-TON")]
    TacTon,
    #[serde(rename = "TON-TAC")]
    TonTac,
    Rollback,
    // This is an artificial operation type.
    // It cannot be returned by API
    // but it can be derived during parsing operation stages data
    #[serde(rename = "INSUFFICIENT-FEE")]
    InsufficientFee,
    Unknown,
    #[serde(other)]
    ErrorType,
}

/// Upstream v2 route. Deliberately not `Serialize`: the value reaches the
/// database and the v1 projection through [`Display`], and a derived
/// `Serialize` would emit a different, externally-tagged shape than
/// [`Deserialize`] accepts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationRoute {
    TonTacTon,
    TacTon,
    TonTac,
    Unknown,
    Unrecognized(String),
}

impl<'de> Deserialize<'de> for OperationRoute {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        // Infallible: unknown routes are preserved as `Unrecognized`.
        Ok(Self::from_str(&value).unwrap_or(Self::Unknown))
    }
}

impl std::fmt::Display for OperationRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TonTacTon => "TON-TAC-TON",
            Self::TacTon => "TAC-TON",
            Self::TonTac => "TON-TAC",
            Self::Unknown => "UNKNOWN",
            Self::Unrecognized(value) => value,
        })
    }
}

impl FromStr for OperationRoute {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value {
            "TON-TAC-TON" => Self::TonTacTon,
            "TAC-TON" => Self::TacTon,
            "TON-TAC" => Self::TonTac,
            "UNKNOWN" => Self::Unknown,
            value => Self::Unrecognized(value.to_string()),
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationStatus {
    Success,
    Failed,
}

impl std::fmt::Display for OperationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Success => "success",
            Self::Failed => "failed",
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationMetaInfo {
    pub initial_caller: Option<Address>,
    #[serde(default, deserialize_with = "deserialize_valid_executors")]
    pub valid_executors: HashMap<BlockchainType, Option<Vec<String>>>,
    #[serde(default, deserialize_with = "deserialize_fee_info")]
    pub fee_info: HashMap<BlockchainType, Option<FeeValue>>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeValue {
    pub protocol_fee: String,
    pub executor_fee: String,
    pub token_fee_symbol: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum StageType {
    CollectedInTAC,
    IncludedInTACConsensus,
    ExecutedInTAC,
    CollectedInTON,
    IncludedInTONConsensus,
    ExecutedInTON,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub exists: bool,
    pub stage_data: Option<StageData>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StageData {
    pub success: bool,
    pub timestamp: u64,
    #[serde(default)]
    pub transactions: Option<Vec<Transaction>>,
    #[serde(default, deserialize_with = "deserialize_note_to_string")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum BlockchainType {
    Tac,
    Ton,
    #[serde(other)]
    Unknown,
}

impl FromStr for BlockchainType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tac" => Ok(BlockchainType::Tac),
            "ton" => Ok(BlockchainType::Ton),
            _ => Err(()),
        }
    }
}

const SUPPORTED_BLOCKCHAIN_TYPE_NAMES: [&str; 2] = ["tac", "ton"];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: String,
    pub blockchain_type: BlockchainType,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub address: String,
    pub blockchain_type: BlockchainType,
}

fn deserialize_note_to_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let val: Option<Value> = Option::deserialize(deserializer)?;
    Ok(val.map(|v| match v {
        Value::String(s) => s,
        other => other.to_string(),
    }))
}

fn deserialize_fee_info<'de, D>(
    deserializer: D,
) -> Result<HashMap<BlockchainType, Option<FeeValue>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, serde_json::Value> = HashMap::deserialize(deserializer)?;
    Ok(SUPPORTED_BLOCKCHAIN_TYPE_NAMES
        .into_iter()
        .filter_map(|k| {
            let key = k.parse().ok()?;
            let val = map.get(k).and_then(|v| {
                if !v.is_null() {
                    serde_json::from_value(v.clone()).ok()
                } else {
                    None
                }
            });
            Some((key, val))
        })
        .collect())
}

fn deserialize_valid_executors<'de, D>(
    deserializer: D,
) -> Result<HashMap<BlockchainType, Option<Vec<String>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, serde_json::Value> = HashMap::deserialize(deserializer)?;
    Ok(SUPPORTED_BLOCKCHAIN_TYPE_NAMES
        .into_iter()
        .filter_map(|k| {
            let key = k.parse().ok()?;
            let val = map.get(k).and_then(|v| {
                if !v.is_null() {
                    serde_json::from_value(v.clone()).ok()
                } else {
                    None
                }
            });
            Some((key, val))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_v2_lifecycle_independently() {
        let response: StageProfilingV2ApiResponse = serde_json::from_value(serde_json::json!({
            "response": {
                "op": {
                    "operationType": "TON-TAC",
                    "status": "failed",
                    "finalized": false,
                    "rollback": true,
                    "metaInfo": null
                }
            }
        }))
        .unwrap();
        let op = &response.response["op"];
        assert_eq!(op.operation_type, OperationRoute::TonTac);
        assert_eq!(op.status, Some(OperationStatus::Failed));
        assert!(!op.finalized);
        assert!(op.rollback);
    }

    #[test]
    fn unknown_v2_route_is_preserved() {
        let response: StageProfilingV2ApiResponse = serde_json::from_value(serde_json::json!({
            "response": {
                "op": {
                    "operationType": "NEW-ROUTE",
                    "finalized": true,
                    "rollback": false,
                    "metaInfo": null
                }
            }
        }))
        .unwrap();
        assert_eq!(
            response.response["op"].operation_type,
            OperationRoute::Unrecognized("NEW-ROUTE".to_string())
        );
    }
}
