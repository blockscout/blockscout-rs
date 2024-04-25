use super::{matches::find_match_contracts, BytecodeRemote, MatchContract};
use crate::{metrics, verification::MatchType};
use bytes::Bytes;
use entity::sea_orm_active_enums::BytecodeType;
use sea_orm::{ConnectionTrait, TransactionTrait};

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
        if let Some(parsed_blueprint_code) = check_blueprint_creation_code(remote.data.clone()) {
            remote.data = parsed_blueprint_code
        }
    }

    if let BytecodeType::DeployedBytecode = remote.bytecode_type {
        if let Some(parsed_blueprint_code) = check_blueprint_runtime_code(remote.data.clone()) {
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

fn check_blueprint_runtime_code(code: Bytes) -> Option<Bytes> {
    let prefix = [0xfe, 0x71, 0x00];
    code.starts_with(&prefix)
        .then_some(code.slice(prefix.len()..))
}

fn check_blueprint_creation_code(code: Bytes) -> Option<Bytes> {
    if code.len() < 10 {
        return None;
    }

    let deploy_bytecode_prefix = [
        0x61, code[1], code[2], 0x3d, 0x81, 0x60, 0x0a, 0x3d, 0x39, 0xf3,
    ];
    if code.starts_with(&deploy_bytecode_prefix) {
        let len_bytes = code[1] as usize * 256 + code[2] as usize;
        let blueprint_bytecode = code.slice(deploy_bytecode_prefix.len()..);
        if blueprint_bytecode.len() == len_bytes {
            return check_blueprint_runtime_code(blueprint_bytecode);
        }
    }

    None
}
