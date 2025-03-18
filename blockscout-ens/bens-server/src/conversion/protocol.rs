use bens_logic::protocols::{Network, Protocol};
use bens_proto::blockscout::bens::v1 as proto;

pub fn protocol_from_logic(p: Protocol, n: Network) -> proto::ProtocolInfo {
    proto::ProtocolInfo {
        id: p.info.slug,
        short_name: p.info.meta.short_name,
        title: p.info.meta.title,
        description: p.info.meta.description,
        deployment_blockscout_base_url: n.blockscout_client.url().to_string(),
        icon_url: p.info.meta.icon_url,
        docs_url: p.info.meta.docs_url,
        tld_list: p.info.tld_list.into_iter().map(|tld| tld.0).collect(),
    }
}
