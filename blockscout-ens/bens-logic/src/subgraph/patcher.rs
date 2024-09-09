use crate::{
    coin_type::Coin,
    entity::subgraph::domain::{DetailedDomain, Domain},
    protocols::DomainNameOnProtocol,
    subgraph::{ens::maybe_wildcard_resolution_with_cache, sql},
};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::instrument;

const MAX_LEVEL: usize = 5;

#[derive(Debug, Default)]
pub struct SubgraphPatcher {
    offchain_mutex: tokio::sync::Mutex<()>,
}

impl SubgraphPatcher {
    pub fn new() -> Self {
        Default::default()
    }

    #[instrument(skip_all, fields(name = %from_user.inner.name))]
    pub async fn handle_user_domain_names(
        &self,
        db: &PgPool,
        from_user: &DomainNameOnProtocol<'_>,
    ) -> Result<(), anyhow::Error> {
        let protocol = from_user.deployed_protocol.protocol;
        let level = from_user.inner.level();
        let range = 2..=MAX_LEVEL;
        let level_is_fine = range.contains(&level);
        if protocol.info.try_offchain_resolve && level_is_fine {
            let _lock = self.offchain_mutex.lock().await;
            offchain_resolve(db, from_user).await?
        };

        Ok(())
    }

    pub fn patched_domain(
        &self,
        pool: Arc<PgPool>,
        mut from_db: Domain,
        from_user: &DomainNameOnProtocol<'_>,
    ) -> Domain {
        if from_db.name.as_ref() != Some(&from_user.inner.name) && from_db.id == from_user.inner.id
        {
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

    pub fn patched_detailed_domain(
        &self,
        pool: Arc<PgPool>,
        mut from_db: DetailedDomain,
        from_user: &DomainNameOnProtocol<'_>,
    ) -> DetailedDomain {
        if from_db.name.as_ref() != Some(&from_user.inner.name) && from_db.id == from_user.inner.id
        {
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
}

async fn offchain_resolve(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> Result<(), anyhow::Error> {
    let protocol = from_user.deployed_protocol.protocol;
    let maybe_domain_cached = maybe_wildcard_resolution_with_cache(db, from_user).await;
    match maybe_domain_cached {
        cached::Return {
            value: Some(domain),
            was_cached: false,
            ..
        } => {
            tracing::info!(
                id = domain.id,
                name = domain.name,
                vid =? domain.vid,
                "found domain with wildcard resolution, save it"
            );
            sql::create_or_update_domain(db, domain, protocol).await?;
        }
        cached::Return {
            was_cached: true, ..
        } => {
            tracing::debug!(
                name = from_user.inner.name,
                "domain was cached by ram cache, skip it"
            );
        }
        cached::Return { value: None, .. } => {
            tracing::debug!("domain not found with wildcard resolution");
        }
    };
    Ok(())
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
