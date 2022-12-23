use super::{
    candidates::{get_matches_by_candidates, BytecodeCandidate},
    types::BytecodeRemote,
    MatchContract,
};
use entity::{bytecode_parts, bytecodes, parts};
use sea_orm::{
    entity::prelude::*, ConnectionTrait, FromQueryResult, QueryOrder, QuerySelect, Statement,
};

pub async fn find_partial_match_contracts<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait,
{
    let candidates = {
        let now = std::time::Instant::now();
        let candidates = find_bytecode_candidates(db, remote).await?;
        tracing::debug!(
            candidates_len = candidates.len(),
            elapsed = now.elapsed().as_secs_f64(),
            "finished bytecode partial candidates search",
        );
        candidates
    };
    get_matches_by_candidates(db, candidates, remote).await
}

#[derive(Debug, FromQueryResult)]
struct PartCandidate {
    id: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum QueryAs {
    BytecodeId,
}

async fn find_bytecode_candidates<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<BytecodeCandidate>, DbErr>
where
    C: ConnectionTrait,
{
    let data = hex::encode(&remote.data);
    let r#type = remote.bytecode_type.clone();

    let part_ids = find_part_id_candidates(db, &data).await?;
    let bytecode_ids: Vec<i64> = bytecodes::Entity::find()
        .join(
            sea_orm::JoinType::LeftJoin,
            bytecodes::Relation::BytecodeParts.def(),
        )
        .filter(bytecode_parts::Column::PartId.is_in(part_ids))
        .filter(bytecode_parts::Column::Order.eq(0))
        .filter(bytecodes::Column::BytecodeType.eq(r#type))
        .select_only()
        .column_as(bytecodes::Column::Id, QueryAs::BytecodeId)
        .into_values::<_, QueryAs>()
        .all(db)
        .await?;

    let bytecodes_parts = bytecodes::Entity::find()
        .filter(bytecodes::Column::Id.is_in(bytecode_ids))
        .find_with_related(parts::Entity)
        // order by bytecode_parts::Order is important during bytecodes comparison
        .order_by_asc(bytecode_parts::Column::Order)
        .all(db)
        .await?;

    Ok(bytecodes_parts
        .into_iter()
        .map(|(bytecode, parts)| BytecodeCandidate { bytecode, parts })
        .collect())
}

async fn find_part_id_candidates<C>(db: &C, data: &str) -> Result<Vec<i64>, DbErr>
where
    C: ConnectionTrait,
{
    let part_ids = PartCandidate::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT *
        FROM parts
        WHERE 
        $1
        LIKE encode("data", 'hex') || '%'
        AND parts.part_type = 'main';"#,
        vec![data.into()],
    ))
    .all(db)
    .await?
    .into_iter()
    .map(|p| p.id)
    .collect();

    Ok(part_ids)
}
