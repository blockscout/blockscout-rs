use crate::protocols::{
    hash_name::CustomDomainIdGenerator, DeployedProtocol, DomainNameOnProtocol,
};
use alloy::{
    primitives::Address,
    providers::{ProviderBuilder, RootProvider},
    transports::BoxTransport,
};
use alloy_ccip_read::CCIPReader;
use anyhow::Context;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct DomainInfoFromCcipRead {
    pub id: String,
    pub name: String,
    pub addr: Address,
    pub resolver_address: Address,
    pub stored_offchain: bool,
}

#[instrument(
    skip_all,
    fields(name = %name.inner.name, resolver = %resolver_address),
    ret(level = "DEBUG"),
    level = "INFO",
)]
pub async fn call_to_resolver(
    name: &DomainNameOnProtocol<'_>,
    resolver_address: Address,
) -> Result<DomainInfoFromCcipRead, anyhow::Error> {
    let name_str = &name.inner.name;
    let reader = reader_from_protocol(&name.deployed_protocol);
    let result = reader
        .resolve_name_with_resolver(name_str, resolver_address)
        .await
        .context("perform ccip call to with resolver")?;

    Ok(DomainInfoFromCcipRead {
        id: name.inner.id.clone(),
        addr: result.addr.value,
        resolver_address,
        name: name.inner.name.clone(),
        stored_offchain: result.ccip_read_used,
    })
}

#[instrument(
    skip_all,
    fields(name = %name.inner.name),
    ret(level = "DEBUG"),
    level = "INFO",
)]
pub async fn get_resolver(name: &DomainNameOnProtocol<'_>) -> Result<Address, anyhow::Error> {
    let reader = reader_from_protocol(&name.deployed_protocol);
    reader
        .get_resolver(&name.inner.name)
        .await
        .context("get resolver")
}

type Reader = CCIPReader<RootProvider<BoxTransport>, CustomDomainIdGenerator>;

fn reader_from_protocol(d: &DeployedProtocol) -> Reader {
    let domain_id_provider = CustomDomainIdGenerator::new(d.protocol.info.empty_label_hash);

    let provider = ProviderBuilder::new()
        .on_http(d.deployment_network.rpc_url())
        .boxed();
    let mut builder = alloy_ccip_read::CCIPReader::builder()
        .with_provider(provider)
        .with_domain_id_provider(domain_id_provider);

    if let Some(registry) = d.protocol.info.registry_contract {
        builder = builder.with_ens_address(registry);
    }

    builder.build().expect("provider passed")
}
