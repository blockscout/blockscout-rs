use super::{matches::find_match_contracts, BytecodeRemote, MatchContract};
use crate::verification::MatchType;
use sea_orm::ConnectionTrait;

pub async fn find_contract<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait,
{
    let mut matches = find_match_contracts(db, remote).await?;
    // If there is at least full match, we do not return any partially matched contract.
    if matches
        .iter()
        .any(|source| source.match_type == MatchType::Full)
    {
        matches.retain(|source| source.match_type == MatchType::Full);
    }

    Ok(matches)
}
