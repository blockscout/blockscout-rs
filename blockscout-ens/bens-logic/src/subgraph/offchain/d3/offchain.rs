use super::get_metadata;
use crate::{
    entity::subgraph::domain::Domain,
    metrics,
    protocols::{AddressResolveTechnique, D3ConnectProtocol, DomainName, DomainNameOnProtocol},
    subgraph::{
        self,
        offchain::{
            offchain_resolution_to_resolve_result, reader_from_protocol,
            DomainInfoFromOffchainResolution, Reader, ResolveResult,
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
    address_resolve_technique: &AddressResolveTechnique,
) -> Option<ResolveResult> {
    metrics::D3_OFFCHAIN_RESOLVE_ATTEMPTS.inc();
    match resolve_d3_name(db, name, d3, address_resolve_technique).await {
        Ok(result) => {
            metrics::D3_OFFCHAIN_RESOLVE_SUCCESS.inc();
            Some(result)
        }
        Err(err) => {
            tracing::error!(
                name = name.inner.name(),
                error = err.to_string(),
                "failed to resolve d3 name"
            );
            None
        }
    }
}

async fn resolve_d3_name(
    db: &PgPool,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
    address_resolve_technique: &AddressResolveTechnique,
) -> Result<ResolveResult, anyhow::Error> {
    let reader = reader_from_protocol(&name.deployed_protocol);

    let default_resolver = d3.resolver_contract;

    let (resolver_address, maybe_existing_domain_vid) =
        match subgraph::sql::get_domain(db, name, true).await? {
            Some(detailed_domain) => {
                let domain = Domain::from(detailed_domain);
                let resolver = domain
                    .resolver
                    .as_ref()
                    .and_then(|r| ResolverInSubgraph::from_str(r).ok())
                    .map(|r| r.resolver_address)
                    .unwrap_or(default_resolver);
                (resolver, Some(domain.vid))
            }
            None => (default_resolver, None),
        };

    let offchain_resolution = get_offchain_resolution(
        &reader,
        resolver_address,
        name,
        d3,
        address_resolve_technique,
    )
    .await?;
    tracing::debug!(data =? offchain_resolution, "fetched offchain resolution");
    let creation_domain =
        offchain_resolution_to_resolve_result(name, offchain_resolution, maybe_existing_domain_vid);
    Ok(creation_domain)
}

async fn get_offchain_resolution(
    reader: &Reader,
    resolver_address: Address,
    name: &DomainNameOnProtocol<'_>,
    d3: &D3ConnectProtocol,
    address_resolve_technique: &AddressResolveTechnique,
) -> Result<DomainInfoFromOffchainResolution, anyhow::Error> {
    let resolve_result =
        alloy_ccip_read::d3::resolve_d3_name(reader, resolver_address, name.inner.name(), "")
            .await?;
    let addr = resolve_result.addr.into_value();
    let addr_to_name = match address_resolve_technique {
        AddressResolveTechnique::Addr2Name | AddressResolveTechnique::PrimaryNameRecord => {
            let reverse_resolve_result =
                alloy_ccip_read::d3::reverse_resolve_d3_name(reader, addr, resolver_address, "")
                    .await?;

            DomainName::new_from_name_and_protocol(
                &reverse_resolve_result.name.value,
                &name.deployed_protocol.protocol.info.protocol_specific,
            )
            .map(|name| name.name().to_string())
            .ok()
        }
        AddressResolveTechnique::ReverseRegistry | AddressResolveTechnique::AllDomains => None,
    };
    let metadata = get_metadata(reader, name, d3).await?;
    let expiry_date = metadata.get_expiration_date();

    Ok(DomainInfoFromOffchainResolution {
        id: name.inner.id().to_string(),
        name: name.inner.name().to_string(),
        addr,
        resolver_address,
        expiry_date,
        stored_offchain: true,
        resolved_with_wildcard: false,
        addr_to_name,
    })
}
