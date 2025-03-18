use super::ResolveResult;
use crate::{
    protocols::{AddressResolveTechnique, DomainNameOnProtocol, ProtocolSpecific},
    subgraph::{
        offchain::{d3, ens},
        sql,
    },
};
use cached::proc_macro::cached;
use sqlx::PgPool;

pub async fn offchain_resolve(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> Result<(), anyhow::Error> {
    let protocol = from_user.deployed_protocol.protocol;
    let maybe_domain_cached = offchain_resolve_cached(db, from_user).await;
    match maybe_domain_cached {
        cached::Return {
            value: Some(result),
            was_cached: false,
            ..
        } => {
            tracing::info!(
                id = result.domain.id,
                name = result.domain.name,
                vid =? result.domain.vid,
                "found domain with offchain resolution, save it"
            );
            sql::create_or_update_domain(db, result.domain, protocol).await?;
            if protocol.info.address_resolve_technique == AddressResolveTechnique::Addr2Name {
                if let Some(reverse_record) = result.maybe_reverse_record {
                    sql::create_or_update_reverse_record(db, reverse_record, protocol).await?;
                }
            }
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
    time = 900, // 15 * 60 seconds = 15 minutes
    size = 500,
    sync_writes = true,
    with_cached_flag = true,
)]
async fn offchain_resolve_cached(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> cached::Return<Option<ResolveResult>> {
    let info = &from_user.deployed_protocol.protocol.info;
    let result = match &info.protocol_specific {
        ProtocolSpecific::EnsLike(ens) => ens::maybe_wildcard_resolution(db, from_user, ens).await,
        ProtocolSpecific::D3Connect(d3) => {
            d3::maybe_offchain_resolution(db, from_user, d3, &info.address_resolve_technique).await
        }
    };
    cached::Return::new(result)
}
