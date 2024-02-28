use super::{domain_name::DomainName, sql};
use crate::{
    coin_type::Coin,
    entity::subgraph::domain::{DetailedDomain, Domain},
};
use sqlx::postgres::PgPool;
use std::sync::Arc;

pub fn patch_domain(
    pool: Arc<PgPool>,
    schema: &str,
    mut domain: Domain,
    domain_name: &DomainName,
) -> Domain {
    if domain.name.as_ref() != Some(&domain_name.name) && domain.id == domain_name.id {
        tracing::warn!(
            domain_id = domain.id,
            input_name = domain_name.name,
            domain_name = domain.name,
            "domain has invalid name, creating task to fix to"
        );
        domain.name = Some(domain_name.name.clone());
        update_domain_name_in_background(pool, schema, domain_name);
    };
    domain
}

pub fn patch_detailed_domain(
    pool: Arc<PgPool>,
    schema: &str,
    mut domain: DetailedDomain,
    domain_name: &DomainName,
) -> DetailedDomain {
    if domain.name.as_ref() != Some(&domain_name.name) && domain.id == domain_name.id {
        tracing::warn!(
            domain_id = domain.id,
            input_name = domain_name.name,
            domain_name = domain.name,
            "domain has invalid name, creating task to fix to"
        );
        domain.name = Some(domain_name.name.clone());
        domain.label_name = Some(domain_name.label_name.clone());
        update_domain_name_in_background(pool, schema, domain_name);
    };
    domain.other_addresses = sqlx::types::Json(
        domain
            .other_addresses
            .0
            .into_iter()
            .map(|(coin_type, address)| {
                let coin = Coin::find_or_unknown(&coin_type);
                let address = if let Some(encoding) = coin.encoding {
                    encoding.encode(&address).unwrap_or(address)
                } else {
                    address
                };
                (coin.name, address)
            })
            .collect(),
    );
    domain
}

fn update_domain_name_in_background(pool: Arc<PgPool>, schema: &str, domain_name: &DomainName) {
    let schema = schema.to_string();
    let domain_name = domain_name.clone();
    tokio::spawn(async move {
        match sql::update_domain_name(pool.as_ref(), &schema, &domain_name).await {
            Ok(r) => {
                tracing::info!(
                    rows_affected = r.rows_affected(),
                    name =? domain_name,
                    "successfuly updated domain name"
                );
            }
            Err(err) => {
                tracing::error!(name =? domain_name, "cannot update domain name: {err}")
            }
        }
    });
}
