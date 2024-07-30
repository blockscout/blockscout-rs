use super::{ccip_read, DomainInfoFromCcipRead};
use crate::{
    entity::subgraph::domain::{CreationDomain, Domain},
    protocols::DomainNameOnProtocol,
    subgraph,
    subgraph::ResolverInSubgraph,
};
use alloy::primitives::Address;
use anyhow::Context;
use cached::proc_macro::cached;

use crate::metrics;
use sqlx::PgPool;
use std::str::FromStr;

/// Check if `name` can be resolved using https://docs.ens.domains/ensip/10
/// Iterates over suffixed names and tries to find a resolver
/// Then resolve the name using CCIP-read
#[cached(
    key = "String",
    convert = r#"{
            from_user.inner.id.to_string()
        }"#,
    time = 3600, // 60 * 60 seconds = 1 hour
    size = 500,
    sync_writes = true,
    with_cached_flag = true,
)]
pub async fn maybe_wildcard_resolution_with_cache(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> cached::Return<Option<CreationDomain>> {
    metrics::WILDCARD_RESOLVE_ATTEMPTS.inc();
    match try_wildcard_resolution(db, from_user).await {
        Ok(result) => {
            if result.is_some() {
                metrics::WILDCARD_RESOLVE_SUCCESS.inc();
            }
            cached::Return::new(result)
        }
        Err(err) => {
            tracing::error!(
                name = from_user.inner.name,
                error = ?err,
                "error while trying wildcard resolution"
            );
            cached::Return::new(None)
        }
    }
}

async fn try_wildcard_resolution(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> Result<Option<CreationDomain>, anyhow::Error> {
    let Some((resolver_address, maybe_existing_domain)) = any_resolver(db, from_user).await? else {
        return Ok(None);
    };

    let maybe_found_domain = maybe_existing_domain.and_then(|domain| {
        if domain.id == from_user.inner.id {
            Some(domain)
        } else {
            None
        }
    });

    if let Some(domain) = &maybe_found_domain {
        if !domain.resolved_with_wildcard {
            // we found domain that resolved by graph node already, skip it
            return Ok(None);
        } else {
            // domain is resolved with wildcard, so we need to recheck it
            tracing::info!(
                domain_id = domain.id,
                domain_name = domain.name,
                "found domain with wildcard-resolution, rechecking it"
            )
        }
    };

    let result = ccip_read::call_to_resolver(from_user, resolver_address)
        .await
        .context("resolve using ccip call")?;

    if !result.addr.is_zero() {
        Ok(Some(creation_domain_from_rpc_resolution(
            from_user,
            result,
            maybe_found_domain,
        )))
    } else {
        Ok(None)
    }
}

async fn any_resolver(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
) -> Result<Option<(Address, Option<Domain>)>, anyhow::Error> {
    let protocol = from_user.deployed_protocol.protocol;
    let name_options = from_user
        .inner
        .iter_parents_with_self()
        .map(|name| DomainNameOnProtocol::new(name, from_user.deployed_protocol))
        .collect::<Vec<_>>();
    // try to find resolver in db
    let maybe_domain_with_resolver = any_resolver_in_db(db, name_options.clone()).await?;
    if let Some((resolver, found_domain)) = maybe_domain_with_resolver {
        return Ok(Some((resolver.resolver_address, Some(found_domain))));
    } else if protocol.info.registry_contract.is_some() {
        // try to find resolver on chain.
        // if custom registry is set, we can try to find resolver in registry
        if let Some(resolver_address) = any_resolver_rpc(name_options).await {
            return Ok(Some((resolver_address, None)));
        }
    };

    Ok(None)
}

async fn any_resolver_in_db(
    db: &PgPool,
    names: Vec<DomainNameOnProtocol<'_>>,
) -> Result<Option<(ResolverInSubgraph, Domain)>, anyhow::Error> {
    let found_parent_domains = {
        let input = subgraph::sql::FindDomainsInput::Names(names.clone());
        let only_active = false;
        let pagination = None;
        subgraph::sql::find_domains(db, input, only_active, pagination)
            .await
            .context("searching parents in db")?
    };

    for name in names {
        let maybe_domain = found_parent_domains
            .iter()
            .find(|d| d.name.as_ref() == Some(&name.inner.name));
        if let Some(domain) = maybe_domain {
            let maybe_resolver = domain
                .resolver
                .as_ref()
                .and_then(|r| ResolverInSubgraph::from_str(r).ok());
            if let Some(resolver) = maybe_resolver {
                return Ok(Some((resolver, domain.clone())));
            }
        }
    }
    Ok(None)
}

async fn any_resolver_rpc(names: Vec<DomainNameOnProtocol<'_>>) -> Option<Address> {
    for name in names {
        let resolver = ccip_read::get_resolver(&name).await.ok()?;
        if !resolver.is_zero() {
            return Some(resolver);
        }
    }
    None
}
fn creation_domain_from_rpc_resolution(
    from_user: &DomainNameOnProtocol<'_>,
    ccip_read_info: DomainInfoFromCcipRead,
    maybe_existing_domain: Option<Domain>,
) -> CreationDomain {
    let parent = from_user.inner.iter_parents_with_self().nth(1);
    let resolver =
        ResolverInSubgraph::new(ccip_read_info.resolver_address, ccip_read_info.id.clone());
    let now = chrono::Utc::now();
    CreationDomain {
        vid: maybe_existing_domain.map(|d| d.vid),
        id: ccip_read_info.id,
        name: Some(ccip_read_info.name),
        label_name: Some(from_user.inner.label_name.clone()),
        labelhash: Some(from_user.inner.labelhash().to_vec()),
        parent: parent.map(|p| p.id),
        subdomain_count: 0,
        resolved_address: Some(ccip_read_info.addr.to_string().to_lowercase()),
        resolver: Some(resolver.to_string()),
        is_migrated: true,
        stored_offchain: ccip_read_info.stored_offchain,
        resolved_with_wildcard: true,
        created_at: now.timestamp().into(),
        owner: Address::ZERO.to_string(),
        wrapped_owner: None,
        expiry_date: None,
        is_expired: false,
    }
}
