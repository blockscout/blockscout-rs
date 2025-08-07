use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Deserialize)]
pub struct StageProfilingApiResponse {
    pub response: HashMap<String, OperationData>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationData {
    pub operation_type: OperationType,
    pub meta_info: Option<OperationMetaInfo>,
    #[serde(flatten)]
    pub stages: HashMap<StageType, Stage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationType {
    Pending,
    #[serde(rename = "TON-TAC-TON")]
    TonTacTon,
    #[serde(rename = "TAC-TON")]
    TacTon,
    #[serde(rename = "TON-TAC")]
    TonTac,
    Rollback,
    Unknown,
    #[serde(other)]
    ErrorType,
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
    pub transactions: Vec<Transaction>,
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
            _ => Ok(BlockchainType::Unknown),
        }
    }
}

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
    Ok(["tac", "ton"]
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
    Ok(["tac", "ton"]
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
