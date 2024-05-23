use super::{
    matches::find_match_contracts,
    types::{BytecodeRemote, BytecodeType},
    MatchContract,
};
use crate::{metrics, verification::MatchType};
use entity::sea_orm_active_enums;
use sea_orm::{ConnectionTrait, TransactionTrait};
use verification_common::blueprint_contracts;

pub async fn find_contract<C>(
    db: &C,
    code_type: sea_orm_active_enums::BytecodeType,
    code: bytes::Bytes,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut remote = BytecodeRemote {
        bytecode_type: code_type.into(),
        data: code,
    };

    let bytecode_type = remote.bytecode_type.to_string();
    let label_values = &[bytecode_type.as_str()];

    let mut is_blueprint = false;
    if let BytecodeType::CreationCode = remote.bytecode_type {
        if let Some(blueprint_contract) =
            blueprint_contracts::from_creation_code(remote.data.clone())
        {
            remote.data = blueprint_contract.initcode;
            remote.bytecode_type = BytecodeType::CreationCodeWithoutConstructor;
            is_blueprint = true;
        }
    }

    if let BytecodeType::RuntimeCode = remote.bytecode_type {
        if let Some(blueprint_contract) =
            blueprint_contracts::from_runtime_code(remote.data.clone())
        {
            remote.data = blueprint_contract.initcode;
            remote.bytecode_type = BytecodeType::CreationCodeWithoutConstructor;
            is_blueprint = true;
        }
    }

    let mut matches = {
        let _timer = metrics::MATCHES_SEARCH_TIME
            .with_label_values(label_values)
            .start_timer();
        find_match_contracts(db, &remote).await?
    };
    matches
        .iter_mut()
        .for_each(|value| value.is_blueprint = is_blueprint);
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
