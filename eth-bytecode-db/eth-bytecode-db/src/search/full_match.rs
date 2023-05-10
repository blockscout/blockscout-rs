use super::{BytecodeRemote, MatchContract};
use crate::verification::MatchType;
use entity::{sea_orm_active_enums::BytecodeType, sources};
use sea_orm::{entity::prelude::*, ConnectionTrait, FromQueryResult, Statement};

pub async fn find_full_match_contract<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait,
{
    let candidates = find_source_candidates(db, remote).await?;
    let mut matches = vec![];
    for candidate in candidates.iter() {
        let match_contract =
            MatchContract::build(db, candidate.id, remote, MatchType::Full).await?;
        matches.push(match_contract);
    }
    if matches.len() > 1 {
        let ids: Vec<i64> = candidates.iter().map(|c| c.id).collect();
        tracing::error!(ids = ?ids, "Full match candidates contains more than one item");
    };
    Ok(matches)
}

#[derive(Debug, FromQueryResult)]
struct SourceCandidate {
    id: i64,
}

async fn find_source_candidates<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<SourceCandidate>, DbErr>
where
    C: ConnectionTrait,
{
    let data = hex::encode(&remote.data);
    let bytecode_type = remote.bytecode_type.clone();
    let bytecode_column = match bytecode_type {
        BytecodeType::CreationInput => sources::Column::RawCreationInputText,
        BytecodeType::DeployedBytecode => sources::Column::RawDeployedBytecodeText,
    }
    .to_string();

    // Here we make use of the index we have on the first 500 symbols
    // of the "raw_creation_input_text" and "raw_deployed_bytecode_text" columns
    let sql = format!(
        r#"
            SELECT "sources"."id"
            FROM "sources"
            WHERE LENGTH("sources"."{bytecode_column}") >= 500
              AND LEFT($1, 500) = LEFT("sources"."{bytecode_column}", 500)
              AND $1 LIKE "sources"."{bytecode_column}" || '%'
            UNION
            SELECT "sources"."id"
            FROM "sources"
            WHERE LENGTH("sources"."{bytecode_column}") < 500
              AND $1 LIKE "sources"."{bytecode_column}" || '%';
        "#
    );
    SourceCandidate::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        &sql,
        vec![data.into()],
    ))
    .all(db)
    .await
}
