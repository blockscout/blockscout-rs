use crate::{
    entity::subgraph::domain::{Domain, DomainWithAddress},
    hash_name::hex,
    subgraphs_reader::{
        domain_name::DomainName, reader::Subgraph, sql, AddressResolveTechnique, SubgraphReadError,
    },
};
use ethers::types::Address;
use sqlx::PgPool;
use std::{collections::HashMap, str::FromStr};

pub async fn resolve_addresses(
    pool: &PgPool,
    subgraph: &Subgraph,
    addresses: Vec<Address>,
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let addresses_str: Vec<String> = addresses.iter().map(hex).collect();
    match subgraph.settings.address_resolve_technique {
        AddressResolveTechnique::AllDomains => match subgraph.settings.use_cache {
            true => {
                sql::AddressNamesView::batch_search_addresses(
                    pool,
                    &subgraph.schema_name,
                    &addresses_str,
                )
                .await
            }
            false => sql::batch_search_addresses(pool, &subgraph.schema_name, &addresses_str).await,
        },
        AddressResolveTechnique::ReverseRegistry => match subgraph.settings.use_cache {
            true => resolve_addr_reverse_cached(pool, subgraph, addresses).await,
            false => resolve_addr_reverse(pool, subgraph, addresses).await,
        },
    }
}

async fn resolve_addr_reverse_cached(
    pool: &PgPool,
    subgraph: &Subgraph,
    addresses: Vec<Address>,
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let addr_reverse_hashes = addresses
        .iter()
        .map(|addr| DomainName::addr_reverse(addr).id)
        .collect::<Vec<String>>();
    let addr_reverse_domains = sql::AddrReverseNamesView::batch_search_addresses(
        pool,
        &subgraph.schema_name,
        &addr_reverse_hashes,
    )
    .await?;

    let domains: Vec<DomainWithAddress> = addr_reverse_domains
        .into_iter()
        .filter_map(|row| {
            let addr = Address::from_str(&row.resolved_address).ok()?;
            let addr_reverse_id = DomainName::addr_reverse(&addr).id;
            if addr_reverse_id == row.reversed_domain_id {
                Some(DomainWithAddress {
                    id: row.domain_id,
                    domain_name: row.name,
                    resolved_address: row.resolved_address,
                })
            } else {
                None
            }
        })
        .collect();

    Ok(domains)
}

async fn resolve_addr_reverse(
    pool: &PgPool,
    subgraph: &Subgraph,
    addresses: Vec<Address>,
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let addr_reverse_hashes = addresses
        .iter()
        .map(|addr| DomainName::addr_reverse(addr).id)
        .collect::<Vec<String>>();

    // mapping of
    // hash(`{addr}.addr.reverse`) -> domain name
    let reversed_names: HashMap<String, DomainName> =
        sql::batch_search_addr_reverse_names(
            pool,
            &subgraph.schema_name,
            &addr_reverse_hashes,
        )
        .await?
        .into_iter()
        .filter_map(|reverse_record| {
            match DomainName::new(&reverse_record.reversed_name, subgraph.settings.empty_label_hash.to_owned()) {
                Ok(name ) => Some((reverse_record.addr_reverse_id, name)),
                Err(err) => {
                    tracing::warn!(err =? err, "failed to hash reversed name '{}', skip", reverse_record.reversed_name);
                    None
                }
            }
        })
        .collect();

    // mapping of
    // hash(name(`{addr}.addr.reverse`)) -> Domain of name(`{addr}.addr.reverse`)
    let reversed_domains: HashMap<String, Domain> = sql::find_domains(
        pool,
        &subgraph.schema_name,
        Some(reversed_names.values().collect()),
        true,
        None,
    )
    .await?
    .into_iter()
    .map(|domain| (domain.id.clone(), domain))
    .collect();

    let domains = addresses
        .into_iter()
        .filter_map(|addr| {
            let addr_reverse = DomainName::addr_reverse(&addr);
            let reversed_name = reversed_names.get(&addr_reverse.id)?;
            let reversed_domain = reversed_domains.get(&reversed_name.id)?;
            if let Some(resolved_address) = &reversed_domain.resolved_address {
                if Address::from_str(resolved_address).ok()? == addr {
                    return Some(DomainWithAddress {
                        id: reversed_name.id.clone(),
                        domain_name: reversed_name.name.clone(),
                        resolved_address: resolved_address.clone(),
                    });
                }
            }
            None
        })
        .collect::<Vec<_>>();

    Ok(domains)
}
