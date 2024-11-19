use crate::{
    entity::subgraph::domain::{CreationAddr2Name, CreationDomain},
    protocols::Protocol,
    subgraph::sql::DbErr,
};
use sqlx::PgPool;

pub async fn create_or_update_domain(
    pool: &PgPool,
    domain: CreationDomain,
    protocol: &Protocol,
) -> Result<(), DbErr> {
    let schema = &protocol.subgraph_schema;
    match domain.vid {
        Some(vid) => {
            update_domain(pool, schema, &domain, vid).await?;
        }
        None => {
            create_domain(pool, schema, &domain).await?;
        }
    };

    Ok(())
}

pub async fn create_or_update_reverse_record(
    pool: &PgPool,
    reverse_record: CreationAddr2Name,
    protocol: &Protocol,
) -> Result<(), DbErr> {
    let schema = &protocol.subgraph_schema;
    sqlx::query(&format!(
        r#"
        INSERT INTO {schema}.addr2name (
            resolved_address,
            domain_id,
            domain_name
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (resolved_address)
        DO UPDATE SET
            domain_id = EXCLUDED.domain_id,
            domain_name = EXCLUDED.domain_name;
        "#
    ))
    .bind(&reverse_record.resolved_address)
    .bind(&reverse_record.domain_id)
    .bind(&reverse_record.domain_name)
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_domain(pool: &PgPool, schema: &str, domain: &CreationDomain) -> Result<(), DbErr> {
    sqlx::query(&format!(
        r#"
        INSERT INTO {schema}.domain (
            id,
            name,
            label_name,
            labelhash,
            parent,
            subdomain_count,
            resolved_address,
            resolver,
            is_migrated,
            owner,
            created_at,
            stored_offchain,
            resolved_with_wildcard,
            block_range
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, int4range(0, null))
        "#
    ))
    .bind(&domain.id)
    .bind(&domain.name)
    .bind(&domain.label_name)
    .bind(&domain.labelhash)
    .bind(&domain.parent)
    .bind(domain.subdomain_count)
    .bind(&domain.resolved_address)
    .bind(&domain.resolver)
    .bind(domain.is_migrated)
    .bind(&domain.owner)
    .bind(&domain.created_at)
    .bind(domain.stored_offchain)
    .bind(domain.resolved_with_wildcard)
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_domain(
    pool: &PgPool,
    schema: &str,
    domain: &CreationDomain,
    vid: i64,
) -> Result<(), DbErr> {
    sqlx::query(&format!(
        r#"
        UPDATE {schema}.domain
        SET
            resolved_address = $1,
            stored_offchain = $2,
            resolved_with_wildcard = $3,
            expiry_date = $4
        WHERE vid = $5
        "#
    ))
    .bind(&domain.resolved_address)
    .bind(domain.stored_offchain)
    .bind(domain.resolved_with_wildcard)
    .bind(&domain.expiry_date)
    .bind(vid)
    .execute(pool)
    .await?;
    Ok(())
}
