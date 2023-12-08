use std::str::FromStr;

use super::ConversionError;
use crate::proto;
use ethers::types::Address;

use metadata_logic::{
    GetMetadataRequest, GetMetadataResponse, NotesRequest, ReputationRequest, TagsRequest,
};

pub fn get_metadata_request_from_inner(
    inner: proto::GetMetadataRequest,
) -> Result<GetMetadataRequest, ConversionError> {
    let address = address_from_str(&inner.address)?;
    Ok(GetMetadataRequest {
        address,
        chain_id: inner.chain_id,
        tags: inner.tags.map(|tags| TagsRequest { limit: tags.limit }),
        notes: inner.notes.map(|notes| NotesRequest { limit: notes.limit }),
        reputation: inner.reputation.map(|_| ReputationRequest {}),
    })
}

pub fn get_metadata_response_from_logic(
    response: GetMetadataResponse,
) -> Result<proto::GetMetadataResponse, ConversionError> {
    Ok(proto::GetMetadataResponse {
        tags: response.tags.map(|tags| proto::TagsResponse {
            values: tags
                .into_iter()
                .map(|tag| proto::Tag {
                    slug: tag.slug,
                    name: tag.name,
                    tag_type: tag.tag_type,
                    ordinal: tag.ordinal,
                    meta: tag.meta.to_string(),
                })
                .collect(),
        }),
        notes: response.notes.map(|notes| proto::NotesResponse {
            values: notes
                .into_iter()
                .map(|note| proto::Note {
                    text: note.text,
                    severity: note.severity,
                    ordinal: note.ordinal,
                    meta: note.meta.to_string(),
                })
                .collect(),
        }),
        reputation: response
            .reputation
            .map(|reputation| proto::ReputationResponse {
                score: reputation.score,
            }),
    })
}

fn address_from_str(addr: &str) -> Result<Address, ConversionError> {
    Address::from_str(addr)
        .map_err(|_| ConversionError::UserRequest(format!("invalid address '{addr}'")))
}
