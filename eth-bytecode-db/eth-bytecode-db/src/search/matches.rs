use super::{
    candidates::{get_matches_by_candidates, BytecodeCandidate},
    types::BytecodeRemote,
    MatchContract,
};
use crate::metrics;
use entity::{bytecode_parts, bytecodes, parts};
use sea_orm::{
    entity::prelude::*, ConnectionTrait, FromQueryResult, QueryOrder, QuerySelect, Statement,
    TransactionTrait,
};

pub async fn find_match_contracts<C>(
    db: &C,
    remote: &BytecodeRemote,
) -> Result<Vec<MatchContract>, anyhow::Error>
where
    C: ConnectionTrait + TransactionTrait,
{
    let bytecode_type = remote.bytecode_type.to_string();
    let label_values = &[bytecode_type.as_str()];

    let candidates = {
        let now = std::time::Instant::now();
        let candidates = {
            let _timer = metrics::BYTECODE_CANDIDATES_SEARCH_TIME
                .with_label_values(label_values)
                .start_timer();
            find_bytecode_candidates(db, remote).await?
        };
        tracing::debug!(
            candidates_len = candidates.len(),
            elapsed = now.elapsed().as_secs_f64(),
            "finished bytecode partial candidates search",
        );
        candidates
    };
    metrics::BYTECODE_CANDIDATES_COUNT
        .with_label_values(label_values)
        .observe(candidates.len() as f64);

    let matches = {
        let _timer = metrics::MATCHES_BY_CANDIDATES_GET_TIME
            .with_label_values(label_values)
            .start_timer();
        get_matches_by_candidates(db, candidates, remote).await?
    };
    Ok(matches)
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
    C: ConnectionTrait + TransactionTrait,
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
    C: ConnectionTrait + TransactionTrait,
{
    let txn = db.begin().await?;

    // Without that the database tends to scan tables sequentially instead of using indexes, for some reason.
    txn.execute_unprepared("SET LOCAL enable_seqscan = OFF;")
        .await?;

    // Here we make use of the index we have on the first 150 symbols of the "data_text" column
    let part_ids = PartCandidate::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
            SELECT "parts"."id"
            FROM parts
            WHERE LENGTH("data_text") >= 150
              AND LEFT($1, 150) = LEFT("data_text", 150)
              AND "part_type" = 'main'
              AND $1 LIKE "data_text" || '%'
            UNION
            SELECT "parts"."id"
            FROM parts
            WHERE LENGTH("data_text") < 150
              AND $1 LIKE "data_text" || '%'
              AND "part_type" = 'main';
        "#,
        vec![data.into()],
    ))
    .all(&txn)
    .await?
    .into_iter()
    .map(|p| p.id)
    .collect();

    Ok(part_ids)
}
