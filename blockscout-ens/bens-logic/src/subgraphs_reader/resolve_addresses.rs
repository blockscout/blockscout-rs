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
                sql::batch_search_addresses_cached(pool, &subgraph.schema_name, &addresses_str)
                    .await
            }
            false => sql::batch_search_addresses(pool, &subgraph.schema_name, &addresses_str).await,
        },
        AddressResolveTechnique::ReverseRegistry => {
            resolve_addresses_using_reverse_registry(pool, subgraph, addresses).await
        }
    }
}

async fn resolve_addresses_using_reverse_registry(
    pool: &PgPool,
    subgraph: &Subgraph,
    addresses: Vec<Address>,
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let addr_reverse_hashes = addresses
        .iter()
        .map(|addr| DomainName::addr_reverse(addr).id)
        .collect::<Vec<String>>();

    // hash(`{addr}.addr.reverse`) -> domain name
    let reversed_names: HashMap<String, DomainName> =
        sql::batch_search_addresses_reverse_registry(
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
