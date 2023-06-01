use super::{types::BytecodeRemote, MatchContract};
use crate::{
    search::bytecodes_comparison::{compare, CompareError, LocalBytecode},
    verification::MatchType,
};
use bytes::Bytes;
use entity::{bytecodes, parts};
use sea_orm::ConnectionTrait;

pub struct BytecodeCandidate {
    pub bytecode: bytecodes::Model,
    pub parts: Vec<parts::Model>,
}

impl BytecodeCandidate {
    /// Compare self with remote bytecode.
    /// Return Ok(()) if this candidate meets the requirements
    pub fn is_match(&self, remote_data: &Bytes) -> Result<MatchType, CompareError> {
        let local =
            LocalBytecode::new(&self.parts).expect("local bytecode should contain valid metadata");
        let result = compare(remote_data, &local);
        if result.is_err() {
            tracing::debug!(error = ?result, "bytecode mismatch");
        };
        result
    }
}

pub async fn get_matches_by_candidates<C>(
    db: &C,
    candidates: Vec<BytecodeCandidate>,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait,
{
    let filtered_bytecodes: Vec<_> = candidates
        .into_iter()
        .filter_map(|c| {
            c.is_match(&remote.data)
                .ok()
                .map(|match_type| (c, match_type))
        })
        .collect();
    if !filtered_bytecodes.is_empty() {
        let ids: Vec<i64> = filtered_bytecodes
            .iter()
            .map(|(b, _)| b.bytecode.id)
            .collect();
        tracing::debug!(ids = ?ids, "found filtered bytecodes");
    }
    let mut matches = vec![];
    for (bytecode, match_type) in filtered_bytecodes.iter() {
        if let Ok(contract_match) =
            MatchContract::build(db, bytecode.bytecode.source_id, remote, *match_type).await
        {
            matches.push(contract_match);
        }
    }
    Ok(matches)
}
