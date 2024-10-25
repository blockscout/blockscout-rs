mod d3;
mod ens;

use super::sql;
use crate::{
    entity::subgraph::domain::CreationDomain,
    protocols::{DomainNameOnProtocol, OffchainStrategy},
};
use cached::proc_macro::cached;
use sqlx::PgPool;

pub async fn offchain_resolve(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> Result<(), anyhow::Error> {
    let protocol = from_user.deployed_protocol.protocol;
    let maybe_domain_cached = check_if_need_to_save_domain_cached(db, from_user).await;
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
                "found domain with offchain resolution, save it"
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

#[cached(
    key = "String",
    convert = r#"{
            format!("{}-{}",  from_user.deployed_protocol.protocol.info.slug, from_user.inner.id)
        }"#,
    time = 14400, // 4 * 60 * 60 seconds = 4 hours
    size = 500,
    sync_writes = true,
    with_cached_flag = true,
)]
async fn check_if_need_to_save_domain_cached(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> cached::Return<Option<CreationDomain>> {
    let result = match from_user.deployed_protocol.protocol.info.offchain_strategy {
        OffchainStrategy::EnsWildcard => ens::maybe_wildcard_resolution(db, from_user).await,
        OffchainStrategy::D3Connect => d3::maybe_offchain_resolution(db, from_user).await,
        _ => None,
    };

    cached::Return::new(result)
}
