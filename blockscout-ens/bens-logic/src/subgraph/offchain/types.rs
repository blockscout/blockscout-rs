use alloy::primitives::Address;
use chrono::{DateTime, Utc};

use crate::{
    entity::subgraph::domain::{CreationDomain, Domain},
    protocols::DomainNameOnProtocol,
    subgraph::ResolverInSubgraph,
};

#[derive(Debug, Clone)]
pub struct DomainInfoFromOffchainResolution {
    pub id: String,
    pub name: String,
    pub addr: Address,
    pub resolver_address: Address,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
    pub expiry_date: Option<DateTime<Utc>>,
}

pub fn creation_domain_from_offchain_resolution(
    from_user: &DomainNameOnProtocol<'_>,
    ccip_read_info: DomainInfoFromOffchainResolution,
    maybe_existing_domain: Option<Domain>,
) -> CreationDomain {
    let parent = from_user.inner.iter_parents_with_self().nth(1);
    let resolver =
        ResolverInSubgraph::new(ccip_read_info.resolver_address, ccip_read_info.id.clone());
    let now = chrono::Utc::now();

    let resolved_address = match ccip_read_info.addr {
        Address::ZERO => None,
        addr => Some(addr.to_string().to_lowercase()),
    };
    CreationDomain {
        vid: maybe_existing_domain.map(|d| d.vid),
        id: ccip_read_info.id,
        name: Some(ccip_read_info.name),
        label_name: Some(from_user.inner.label_name.clone()),
        labelhash: Some(from_user.inner.labelhash().to_vec()),
        parent: parent.map(|p| p.id),
        subdomain_count: 0,
        resolved_address,
        resolver: Some(resolver.to_string()),
        is_migrated: true,
        stored_offchain: ccip_read_info.stored_offchain,
        resolved_with_wildcard: ccip_read_info.resolved_with_wildcard,
        created_at: now.timestamp().into(),
        owner: Address::ZERO.to_string(),
        wrapped_owner: None,
        expiry_date: ccip_read_info.expiry_date.map(|d| d.timestamp().into()),
        is_expired: false,
    }
}
