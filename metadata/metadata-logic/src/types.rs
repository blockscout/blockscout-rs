use ethers::types::Address;
use sea_orm::{FromQueryResult, JsonValue};

pub struct GetMetadataRequest {
    pub address: Address,
    pub chain_id: Option<i32>,
    pub tags: Option<TagsRequest>,
    pub notes: Option<NotesRequest>,
    pub reputation: Option<ReputationRequest>,
}

pub struct TagsRequest {
    pub limit: u64,
}
pub struct NotesRequest {
    pub limit: u64,
}
pub struct ReputationRequest {}

#[derive(Default)]
pub struct GetMetadataResponse {
    pub tags: Option<Vec<TagData>>,
    pub notes: Option<Vec<NoteData>>,
    pub reputation: Option<ReputationData>,
}

#[derive(FromQueryResult, Debug)]
pub struct TagData {
    pub slug: String,
    pub name: String,
    pub tag_type: String,
    pub ordinal: i32,
    pub meta: JsonValue,
}

#[derive(FromQueryResult, Debug)]
pub struct NoteData {
    pub text: String,
    pub severity: String,
    pub ordinal: i32,
    pub meta: JsonValue,
}

#[derive(FromQueryResult, Debug)]
pub struct ReputationData {
    pub score: i32,
}
