use crate::{
    entity::subgraph::domain::{CreationAddr2Name, CreationDomain, Domain},
    protocols::{DomainName, DomainNameOnProtocol},
    subgraph::ResolverInSubgraph,
};
use alloy::primitives::Address;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct DomainInfoFromOffchainResolution {
    pub id: String,
    pub name: String,
    pub addr: Address,
    pub resolver_address: Address,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
    pub expiry_date: Option<DateTime<Utc>>,
    pub addr2name: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolveResult {
    pub domain: CreationDomain,
    pub maybe_reverse_record: Option<CreationAddr2Name>,
}

pub fn offchain_resolution_to_resolve_result(
    from_user: &DomainNameOnProtocol<'_>,
    ccip_read_info: DomainInfoFromOffchainResolution,
    maybe_existing_domain: Option<Domain>,
) -> ResolveResult {
    let parent = from_user.inner.iter_parents_with_self().nth(1);
    let resolver =
        ResolverInSubgraph::new(ccip_read_info.resolver_address, ccip_read_info.id.clone());
    let now = chrono::Utc::now();

    let resolved_address =
        non_zero_address(ccip_read_info.addr).map(|a| a.to_string().to_lowercase());
    let domain = CreationDomain {
        vid: maybe_existing_domain.map(|d| d.vid),
        id: ccip_read_info.id.clone(),
        name: Some(ccip_read_info.name.clone()),
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
    };

    let maybe_addr2name = ccip_read_info.addr2name.as_ref().and_then(|name| {
        DomainName::new(
            name,
            from_user
                .deployed_protocol
                .protocol
                .info
                .protocol_specific
                .empty_label_hash(),
        )
        .ok()
    });
    let maybe_reverse_record = match (&domain.resolved_address, maybe_addr2name) {
        (Some(addr), Some(name)) => Some(CreationAddr2Name {
            resolved_address: addr.clone(),
            domain_id: Some(name.id),
            domain_name: Some(name.name),
        }),
        (Some(addr), None) => Some(CreationAddr2Name {
            resolved_address: addr.clone(),
            domain_id: None,
            domain_name: None,
        }),
        (None, _) => None,
    };

    ResolveResult {
        domain,
        maybe_reverse_record,
    }
}

fn non_zero_address(addr: Address) -> Option<Address> {
    match addr {
        Address::ZERO => None,
        addr => Some(addr),
    }
}
