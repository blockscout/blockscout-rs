use super::ccip_read;
use crate::{
    entity::subgraph::domain::{CreationDomain, Domain},
    metrics,
    protocols::{DomainNameOnProtocol, EnsLikeProtocol},
    subgraph::{self, offchain::creation_domain_from_offchain_resolution, ResolverInSubgraph},
};
use alloy::primitives::Address;
use anyhow::Context;
use sqlx::PgPool;
use std::str::FromStr;

/// Check if `name` can be resolved using https://docs.ens.domains/ensip/10
/// Iterates over suffixed names and tries to find a resolver
/// Then resolve the name using CCIP-read
pub async fn maybe_wildcard_resolution(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
    ens: &EnsLikeProtocol,
) -> Option<CreationDomain> {
    metrics::WILDCARD_RESOLVE_ATTEMPTS.inc();
    match try_wildcard_resolution(db, from_user, ens).await {
        Ok(result) => {
            if result.is_some() {
                metrics::WILDCARD_RESOLVE_SUCCESS.inc();
            }
            result
        }
        Err(err) => {
            tracing::error!(
                name = from_user.inner.name,
                error = ?err,
                "error while trying wildcard resolution"
            );
            None
        }
    }
}

async fn try_wildcard_resolution(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
    ens: &EnsLikeProtocol,
) -> Result<Option<CreationDomain>, anyhow::Error> {
    let Some((resolver_address, maybe_existing_domain)) = any_resolver(db, from_user, ens).await?
    else {
        return Ok(None);
    };
    let result = ccip_read::call_to_resolver(from_user, resolver_address)
        .await
        .context("resolve using ccip call")?;
    if !result.addr.is_zero() {
        let maybe_domain_to_update = maybe_existing_domain.and_then(|domain| {
            if domain.id == from_user.inner.id {
                Some(domain)
            } else {
                None
            }
        });
        Ok(Some(creation_domain_from_offchain_resolution(
            from_user,
            result,
            maybe_domain_to_update,
        )))
    } else {
        Ok(None)
    }
}

async fn any_resolver(
    db: &PgPool,
    from_user: &DomainNameOnProtocol<'_>,
    ens: &EnsLikeProtocol,
) -> Result<Option<(Address, Option<Domain>)>, anyhow::Error> {
    let name_options = from_user
        .inner
        .iter_parents_with_self()
        .map(|name| DomainNameOnProtocol::new(name, from_user.deployed_protocol))
        .collect::<Vec<_>>();
    // try to find resolver in db
    let maybe_domain_with_resolver = any_resolver_in_db(db, name_options.clone()).await?;
    if let Some((resolver, found_domain)) = maybe_domain_with_resolver {
        return Ok(Some((resolver.resolver_address, Some(found_domain))));
    } else if ens.registry_contract.is_some() {
        // try to find resolver on chain.
        // if custom registry is set, we can try to find resolver in registry
        if let Some(resolver_address) = any_resolver_rpc(name_options, ens).await {
            return Ok(Some((resolver_address, None)));
        }
    };

    Ok(None)
}

async fn any_resolver_rpc(
    names: Vec<DomainNameOnProtocol<'_>>,
    ens: &EnsLikeProtocol,
) -> Option<Address> {
    for name in names {
        let resolver = ccip_read::get_resolver(&name, ens).await.ok()?;
        if !resolver.is_zero() {
            return Some(resolver);
        }
    }
    None
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
