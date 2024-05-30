use bens_logic::protocols::Protocol;
use bens_proto::blockscout::bens::v1 as proto;

pub fn protocol_from_logic(p: Protocol) -> proto::ProtocolInfo {
    proto::ProtocolInfo {
        id: p.info.slug,
        short_name: p.info.meta.short_name,
        title: p.info.meta.title,
        description: p.info.meta.description,
        icon_url: p.info.meta.icon_url,
        docs_url: p.info.meta.docs_url,
        tld_list: p.info.tld_list.into_iter().map(|tld| tld.0).collect(),
    }
}
