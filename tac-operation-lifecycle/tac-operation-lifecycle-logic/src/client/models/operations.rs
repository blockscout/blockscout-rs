use serde::Deserialize;

#[derive(Deserialize)]
pub struct OperationIdsApiResponse {
    pub response: ResponseData,
}

#[derive(Deserialize)]
pub struct ResponseData {
    pub total: u32,
    pub operations: Operations,
}

pub type Operations = Vec<Operation>;

#[derive(Deserialize, Debug)]
pub struct Operation {
    #[serde(rename = "operationId")]
    pub id: String,
    pub timestamp: u64,
}
