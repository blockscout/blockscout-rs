use crate::{
    protocols::{DomainNameOnProtocol, EnsLikeProtocol},
    subgraph::offchain::{ccip_read::reader_from_protocol, DomainInfoFromOffchainResolution},
};
use alloy::primitives::Address;
use anyhow::Context;
use tracing::instrument;

#[instrument(
    skip_all,
    fields(name = %name.inner.name, resolver = %resolver_address),
    ret(level = "DEBUG"),
    level = "INFO",
)]
pub async fn call_to_resolver(
    name: &DomainNameOnProtocol<'_>,
    resolver_address: Address,
) -> Result<DomainInfoFromOffchainResolution, anyhow::Error> {
    let name_str = &name.inner.name;
    let reader = reader_from_protocol(&name.deployed_protocol);
    let result =
        alloy_ccip_read::ens::resolve_name_with_resolver(&reader, name_str, resolver_address)
            .await
            .context("perform ccip call to with resolver")?;

    Ok(DomainInfoFromOffchainResolution {
        id: name.inner.id.clone(),
        addr: result.addr.value,
        resolver_address,
        name: name.inner.name.clone(),
        stored_offchain: result.ccip_read_used,
        resolved_with_wildcard: result.wildcard_used,
        expiry_date: None,
        addr2name: None,
    })
}

#[instrument(
    skip_all,
    fields(name = %name.inner.name),
    ret(level = "DEBUG"),
    level = "INFO",
)]
pub async fn get_resolver(
    name: &DomainNameOnProtocol<'_>,
    ens: &EnsLikeProtocol,
) -> Result<Address, anyhow::Error> {
    let reader = reader_from_protocol(&name.deployed_protocol);
    alloy_ccip_read::ens::get_resolver_wildcarded(&reader, ens.registry_contract, &name.inner.name)
        .await
        .context("get resolver")
}
