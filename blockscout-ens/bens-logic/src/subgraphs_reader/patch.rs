use super::sql;
use crate::{
    coin_type::Coin,
    entity::subgraph::domain::{DetailedDomain, Domain},
    protocols::DomainNameOnProtocol,
};
use sqlx::postgres::PgPool;
use std::sync::Arc;

pub fn patch_domain(
    pool: Arc<PgPool>,
    mut from_db: Domain,
    from_user: &DomainNameOnProtocol,
) -> Domain {
    if from_db.name.as_ref() != Some(&from_user.inner.name) && from_db.id == from_user.inner.id {
        tracing::warn!(
            domain_id = from_db.id,
            input_name = from_user.inner.name,
            domain_name = from_db.name,
            "domain has invalid name, creating task to fix to"
        );
        from_db.name = Some(from_user.inner.name.clone());
        update_domain_name_in_background(pool, from_user.clone());
    };
    from_db
}

pub fn patch_detailed_domain(
    pool: Arc<PgPool>,
    mut from_db: DetailedDomain,
    from_user: &DomainNameOnProtocol,
) -> DetailedDomain {
    if from_db.name.as_ref() != Some(&from_user.inner.name) && from_db.id == from_user.inner.id {
        tracing::warn!(
            domain_id = from_db.id,
            input_name = from_user.inner.name,
            domain_name = from_db.name,
            "domain has invalid name, creating task to fix to"
        );
        from_db.name = Some(from_user.inner.name.clone());
        from_db.label_name = Some(from_user.inner.label_name.clone());
        update_domain_name_in_background(pool, from_user.clone());
    };
    from_db.other_addresses = sqlx::types::Json(
        from_db
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
    from_db
}

fn update_domain_name_in_background(pool: Arc<PgPool>, domain_name: DomainNameOnProtocol) {
    let schema = domain_name
        .deployed_protocol
        .protocol
        .subgraph_schema
        .clone();
    let domain_name = domain_name.inner.clone();
    tokio::spawn(async move {
        let name = domain_name.name.clone();
        match sql::update_domain_name(pool.as_ref(), &schema, domain_name).await {
            Ok(r) => {
                tracing::info!(
                    rows_affected = r.rows_affected(),
                    name =? name,
                    "successfully updated domain name"
                );
            }
            Err(err) => {
                tracing::error!(name =? name, "cannot update domain name: {err}")
            }
        }
    });
}
