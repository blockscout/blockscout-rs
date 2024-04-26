use super::{matches::find_match_contracts, BytecodeRemote, MatchContract};
use crate::{metrics, verification::MatchType};
use entity::sea_orm_active_enums::BytecodeType;
use sea_orm::{ConnectionTrait, TransactionTrait};
use verification_common::blueprint_contracts;

pub async fn find_contract<C>(
    db: &C,
    mut remote: BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait + TransactionTrait,
{
    let bytecode_type = remote.bytecode_type.to_string();
    let label_values = &[bytecode_type.as_str()];

    if let BytecodeType::CreationInput = remote.bytecode_type {
        if let Some(parsed_blueprint_code) =
            blueprint_contracts::from_creation_code(remote.data.clone())
        {
            remote.data = parsed_blueprint_code
        }
    }

    if let BytecodeType::DeployedBytecode = remote.bytecode_type {
        if let Some(parsed_blueprint_code) =
            blueprint_contracts::from_runtime_code(remote.data.clone())
        {
            remote.data = parsed_blueprint_code;
            remote.bytecode_type = BytecodeType::CreationInput;
        }
    }

    let mut matches = {
        let _timer = metrics::MATCHES_SEARCH_TIME
            .with_label_values(label_values)
            .start_timer();
        find_match_contracts(db, &remote).await?
    };
    metrics::ALL_MATCHES_COUNT
        .with_label_values(label_values)
        .observe(matches.len() as f64);

    // If there is at least full match, we do not return any partially matched contract.
    {
        let _timer = metrics::FULL_MATCHES_CHECK_TIME
            .with_label_values(label_values)
            .start_timer();
        if matches
            .iter()
            .any(|source| source.match_type == MatchType::Full)
        {
            matches.retain(|source| source.match_type == MatchType::Full);
        }
    }

    metrics::FULL_MATCHES_COUNT
        .with_label_values(label_values)
        .observe(
            matches
                .first()
                .filter(|&contract| contract.match_type == MatchType::Full)
                .map(|_| matches.len() as f64)
                .unwrap_or_default(),
        );

    Ok(matches)
}
