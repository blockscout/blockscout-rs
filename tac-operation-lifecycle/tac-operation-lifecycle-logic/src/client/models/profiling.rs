use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct StageProfilingApiResponse {
    pub response: HashMap<String, OperationData>,
}

#[derive(Debug, Deserialize)]
pub struct OperationData {
    #[serde(rename = "operationType")]
    pub operation_type: OperationType,
    #[serde(flatten)]
    pub stages: HashMap<StageType, Stage>,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
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

#[derive(Debug, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum StageType {
    CollectedInTAC,
    IncludedInTACConsensus,
    ExecutedInTAC,
    CollectedInTON,
    IncludedInTONConsensus,
    ExecutedInTON,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub exists: bool,
    pub stage_data: Option<StageData>,
}

#[derive(Debug, Deserialize)]
pub struct StageData {
    pub success: bool,
    pub timestamp: u64,
    pub transactions: Vec<Transaction>,
    #[serde(default, deserialize_with = "deserialize_note_to_string")]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlockchainType {
    Tac,
    Ton,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub hash: String,
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
