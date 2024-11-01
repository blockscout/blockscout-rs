use super::get_metadata;
use crate::{
    entity::subgraph::domain::{CreationDomain, Domain},
    protocols::{D3ConnectProtocol, DomainNameOnProtocol},
    subgraph::{
        self,
        offchain::{
            creation_domain_from_offchain_resolution, reader_from_protocol,
            DomainInfoFromOffchainResolution, Reader,
        },
        ResolverInSubgraph,
    },
};
use alloy::primitives::Address;
use sqlx::PgPool;
use std::str::FromStr;

pub async fn maybe_offchain_resolution(
    db: &PgPool,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
) -> Option<CreationDomain> {
    resolve_d3_name(db, name, d3).await.ok()
}

async fn resolve_d3_name(
    db: &PgPool,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
) -> Result<CreationDomain, anyhow::Error> {
    let reader = reader_from_protocol(&name.deployed_protocol);

    let default_resolver = d3.resolver_contract;

    let (resolver_address, maybe_existing_domain) =
        match subgraph::sql::get_domain(db, name, true).await? {
            Some(detailed_domain) => {
                let domain = Domain::from(detailed_domain);
                let resolver = domain
                    .resolver
                    .as_ref()
                    .and_then(|r| ResolverInSubgraph::from_str(r).ok())
                    .map(|r| r.resolver_address)
                    .unwrap_or(default_resolver);
                (resolver, Some(domain))
            }
            None => (default_resolver, None),
        };

    let offchain_resolution = get_offchain_resolution(&reader, resolver_address, name, d3).await?;
    tracing::debug!(data =? offchain_resolution, "fetched offchain resolution");
    let creation_domain =
        creation_domain_from_offchain_resolution(name, offchain_resolution, maybe_existing_domain);
    Ok(creation_domain)
}

async fn get_offchain_resolution(
    reader: &Reader,
    resolver_address: Address,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
) -> Result<DomainInfoFromOffchainResolution, anyhow::Error> {
    let resolve_result =
        alloy_ccip_read::d3::resolve_d3_name(reader, resolver_address, &name.inner.name, "")
            .await?;
    let metadata = get_metadata(reader, name, d3).await?;
    let expiry_date = metadata.get_expiration_date();
    Ok(DomainInfoFromOffchainResolution {
        id: name.inner.id.clone(),
        name: name.inner.name.clone(),
        addr: resolve_result.addr.into_value(),
        resolver_address,
        expiry_date,
        stored_offchain: true,
        resolved_with_wildcard: false,
    })
}
