// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{
    match_contract::{find_source_details, find_source_files},
    types::BytecodeRemote,
    MatchContract,
};
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
    #[allow(clippy::result_large_err)]
    pub fn is_match(&self, remote_data: &Bytes) -> Result<MatchType, CompareError> {
        let local = LocalBytecode::new(&self.parts).map_err(|err| {
            tracing::warn!(
                bytecode_id = self.bytecode.id,
                error = ?err,
                "failed to parse local bytecode parts; skipping the candidate"
            );
            CompareError::MetadataParse(err.to_string())
        })?;
        let result = compare(remote_data, &local);
        if result.is_err() {
            tracing::debug!(error = ?result, "bytecode mismatch");
        };
        result
    }

    /// The length of the local bytecode, that is of its parts concatenated in order.
    /// It is the offset the comparison walks up to, so whatever the remote bytecode
    /// carries past it is taken to be the encoded constructor arguments.
    fn local_bytecode_len(&self) -> usize {
        self.parts.iter().map(|part| part.data.len()).sum()
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
    if filtered_bytecodes.is_empty() {
        return Ok(vec![]);
    }
    tracing::debug!(
        ids = ?filtered_bytecodes.iter().map(|(b, _)| b.bytecode.id).collect::<Vec<_>>(),
        "found filtered bytecodes"
    );

    // `bytecodes` is unique on (source_id, bytecode_type) and the candidate search filters
    // by a single type, so no two candidates here share a source.
    let source_ids: Vec<i64> = filtered_bytecodes
        .iter()
        .map(|(b, _)| b.bytecode.source_id)
        .collect();
    let mut sources = find_source_details(db, &source_ids).await?;
    let mut source_files = find_source_files(db, &source_ids).await?;

    let mut matches = Vec::with_capacity(filtered_bytecodes.len());
    for (candidate, match_type) in filtered_bytecodes.iter() {
        let source_id = candidate.bytecode.source_id;
        let Some(source) = sources.remove(&source_id) else {
            tracing::warn!(source_id, "bytecode doesn't have valid source_id");
            continue;
        };
        let source_files = source_files.remove(&source_id).unwrap_or_default();
        match MatchContract::build(
            source,
            source_files,
            candidate.local_bytecode_len(),
            remote,
            *match_type,
        ) {
            Ok(contract_match) => matches.push(contract_match),
            Err(error) => tracing::debug!(source_id, ?error, "skipping the candidate"),
        }
    }

    Ok(matches)
}
