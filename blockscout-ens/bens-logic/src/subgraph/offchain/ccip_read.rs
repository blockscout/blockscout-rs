use crate::protocols::{hash_name::CustomDomainIdGenerator, DeployedProtocol};
use alloy::{
    providers::{ProviderBuilder, RootProvider},
    transports::BoxTransport,
};
use alloy_ccip_read::CCIPReader;

pub type Reader = CCIPReader<RootProvider<BoxTransport>, CustomDomainIdGenerator>;

pub fn reader_from_protocol(d: &DeployedProtocol) -> Reader {
    let domain_id_provider =
        CustomDomainIdGenerator::new(d.protocol.info.protocol_specific.empty_label_hash());

    let provider = ProviderBuilder::new()
        .on_http(d.deployment_network.rpc_url())
        .boxed();
    let builder = alloy_ccip_read::CCIPReader::builder()
        .with_provider(provider)
        .with_domain_id_provider(domain_id_provider);

    builder.build().expect("provider passed")
}
