use crate::{
    entity::subgraph::domain::DomainWithAddress,
    protocols::{hash_name::hex, AddressResolveTechnique, DomainName, Protocol},
    subgraph::{sql, sql::DbErr},
};
use alloy::primitives::Address;
use nonempty::{nonempty, NonEmpty};
use sqlx::PgPool;
use std::{
    collections::{hash_map::Entry, HashMap},
    str::FromStr,
};

pub async fn resolve_addresses(
    pool: &PgPool,
    protocols: NonEmpty<&Protocol>,
    addresses: Vec<Address>,
) -> Result<Vec<DomainWithAddress>, DbErr> {
    let mut result = vec![];
    for (technique, protocols) in
        grouping_by(protocols, |p| p.info.address_resolve_technique.clone())
    {
        let found_domains = match technique {
            AddressResolveTechnique::AllDomains => {
                resolve_all_domains_cached(pool, &protocols, &addresses).await?
            }
            AddressResolveTechnique::ReverseRegistry => {
                resolve_addr_reverse_cached(pool, &protocols, &addresses).await?
            }
            AddressResolveTechnique::Addr2Name => {
                resolve_addr2name(pool, &protocols, &addresses).await?
            }
        };
        result.extend(found_domains);
    }
    Ok(result)
}

fn grouping_by<T, K, F>(iter: impl IntoIterator<Item = T>, mut key: F) -> HashMap<K, NonEmpty<T>>
where
    F: FnMut(&T) -> K,
    K: Eq + std::hash::Hash,
{
    let mut map: HashMap<K, NonEmpty<T>> = HashMap::new();
    for item in iter {
        match map.entry(key(&item)) {
            Entry::Occupied(mut o) => {
                o.get_mut().push(item);
            }
            Entry::Vacant(v) => {
                v.insert(nonempty![item]);
            }
        };
    }
    map
}

async fn resolve_all_domains_cached(
    pool: &PgPool,
    protocols: &NonEmpty<&Protocol>,
    addresses: &[Address],
) -> Result<Vec<DomainWithAddress>, DbErr> {
    let addresses_str: Vec<String> = addresses.iter().map(hex).collect();
    sql::AddressNamesView::batch_search_addresses(pool, protocols, &addresses_str).await
}

async fn resolve_addr_reverse_cached(
    pool: &PgPool,
    protocols: &NonEmpty<&Protocol>,
    addresses: &[Address],
) -> Result<Vec<DomainWithAddress>, DbErr> {
    let addr_reverse_hashes = addresses
        .iter()
        .map(|addr| DomainName::addr_reverse(addr).id)
        .collect::<Vec<String>>();
    let addr_reverse_domains =
        sql::AddrReverseNamesView::batch_search_addresses(pool, protocols, &addr_reverse_hashes)
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

async fn resolve_addr2name(
    pool: &PgPool,
    protocols: &NonEmpty<&Protocol>,
    addresses: &[Address],
) -> Result<Vec<DomainWithAddress>, DbErr> {
    let addresses_str: Vec<String> = addresses.iter().map(hex).collect();
    sql::Addr2NameTable::batch_search_addreses(pool, protocols, &addresses_str).await
}

// async fn resolve_addr_reverse(
//     pool: &PgPool,
//     protocols: &NonEmpty<&Protocol>,
//     addresses: &Vec<Address>,
// ) -> Result<Vec<DomainWithAddress>, DbErr> {
//     let addr_reverse_hashes = addresses
//         .iter()
//         .map(|addr| DomainName::addr_reverse(addr).id)
//         .collect::<Vec<String>>();
//
//     // mapping of
//     // hash(`{addr}.addr.reverse`) -> domain name
//     let reversed_names: HashMap<String, DomainNameOnProtocol> =
//         sql::batch_search_addr_reverse_names(
//             pool,
//             protocols,
//             &addr_reverse_hashes,
//         )
//         .await?
//         .into_iter()
//         .filter_map(|reverse_record| {
//             match DomainNameOnProtocol::new(&reverse_record.reversed_name, protocols.head) {
//                 Ok(name ) => Some((reverse_record.addr_reverse_id, name)),
//                 Err(err) => {
//                     tracing::warn!(err =? err, "failed to hash reversed name '{}', skip", reverse_record.reversed_name);
//                     None
//                 }
//             }
//         })
//         .collect();
//
//     let find_domains_input = FindDomainsInput::Names(reversed_names.values().cloned().collect());
//     // mapping of
//     // hash(name(`{addr}.addr.reverse`)) -> Domain of name(`{addr}.addr.reverse`)
//     let reversed_domains: HashMap<String, Domain> =
//         sql::find_domains(pool, find_domains_input, true, None)
//             .await?
//             .into_iter()
//             .map(|domain| (domain.id.clone(), domain))
//             .collect();
//
//     let domains = addresses
//         .into_iter()
//         .filter_map(|addr| {
//             let addr_reverse = DomainName::addr_reverse(&addr);
//             let reversed_name = &reversed_names.get(&addr_reverse.id)?.inner;
//             let reversed_domain = reversed_domains.get(&reversed_name.id)?;
//             if let Some(resolved_address) = &reversed_domain.resolved_address {
//                 if Address::from_str(resolved_address).ok()? == *addr {
//                     return Some(DomainWithAddress {
//                         id: reversed_name.id.clone(),
//                         domain_name: reversed_name.name.clone(),
//                         resolved_address: resolved_address.clone(),
//                     });
//                 }
//             }
//             None
//         })
//         .collect::<Vec<_>>();
//
//     Ok(domains)
// }
