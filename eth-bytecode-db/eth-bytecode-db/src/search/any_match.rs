use super::{
    find_full_match_contract, find_partial_match_contracts, BytecodeRemote, MatchContract,
};
use sea_orm::ConnectionTrait;

pub async fn find_contract<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait,
{
    let full_matches = find_full_match_contract(db, remote).await?;
    if !full_matches.is_empty() {
        return Ok(full_matches);
    };

    let partial_matches = find_partial_match_contracts(db, remote).await?;
    if !partial_matches.is_empty() {
        return Ok(partial_matches);
    };

    Ok(vec![])
}
