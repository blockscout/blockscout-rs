use bens_logic::protocols::Protocol;
use bens_proto::blockscout::bens::v1 as proto;

pub fn protocol_from_logic(p: &Protocol) -> proto::ProtocolInfo {
    proto::ProtocolInfo {
        id: p.info.slug.clone(),
        short_name: p.info.meta.short_name.clone(),
        title: p.info.meta.title.clone(),
        description: p.info.meta.description.clone(),
        icon_url: p.info.meta.icon_url.clone(),
        docs_url: p.info.meta.docs_url.clone(),
        tld_list: p.info.tld_list.iter().map(|tld| tld.0.clone()).collect(),
    }
}
