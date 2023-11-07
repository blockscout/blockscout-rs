use std::str::FromStr;

use super::ConversionError;
use crate::conversion::order_direction_from_inner;
use bens_logic::{
    entity::subgraph::domain::{DetailedDomain, Domain},
    hash_name::hex,
    subgraphs_reader::{
        BatchResolveAddressNamesInput, DomainSort, GetDomainInput, LookupAddressInput,
        LookupDomainInput,
    },
};
use bens_proto::blockscout::bens::v1 as proto;
use ethers::types::Address;

pub fn get_domain_input_from_inner(
    inner: proto::GetDomainRequest,
) -> Result<GetDomainInput, ConversionError> {
    Ok(GetDomainInput {
        network_id: inner.chain_id,
        name: inner.name,
        only_active: true,
    })
}

pub fn lookup_domain_name_from_inner(
    inner: proto::LookupDomainNameRequest,
) -> Result<LookupDomainInput, ConversionError> {
    let sort = domain_sort_from_inner(&inner.sort)?;
    let order = order_direction_from_inner(inner.order());
    Ok(LookupDomainInput {
        network_id: inner.chain_id,
        name: inner.name,
        only_active: inner.only_active,
        sort,
        order,
    })
}

pub fn lookup_address_from_inner(
    inner: proto::LookupAddressRequest,
) -> Result<LookupAddressInput, ConversionError> {
    let sort = domain_sort_from_inner(&inner.sort)?;
    let order = order_direction_from_inner(inner.order());
    let address = address_from_str_inner(&inner.address)?;
    Ok(LookupAddressInput {
        network_id: inner.chain_id,
        address,
        resolved_to: inner.resolved_to,
        owned_by: inner.owned_by,
        only_active: inner.only_active,
        sort,
        order,
    })
}

pub fn domain_sort_from_inner(inner: &str) -> Result<DomainSort, ConversionError> {
    match inner {
        "" | "registration_date" => Ok(DomainSort::RegistrationDate),
        _ => Err(ConversionError::UserRequest(format!(
            "unknow sort field '{inner}'"
        ))),
    }
}

pub fn batch_resolve_from_inner(
    inner: proto::BatchResolveAddressNamesRequest,
) -> Result<BatchResolveAddressNamesInput, ConversionError> {
    let addresses = inner
        .addresses
        .iter()
        .map(|addr| address_from_str_inner(addr))
        .collect::<Result<_, _>>()?;
    Ok(BatchResolveAddressNamesInput {
        network_id: inner.chain_id,
        addresses,
    })
}

pub fn detailed_domain_from_logic(
    d: DetailedDomain,
) -> Result<proto::DetailedDomain, ConversionError> {
    let owner = Some(proto::Address { hash: d.owner });
    let resolved_address = d.resolved_address.map(|resolved_address| proto::Address {
        hash: resolved_address,
    });
    let registrant = d
        .registrant
        .map(|registrant| proto::Address { hash: registrant });
    Ok(proto::DetailedDomain {
        id: d.id,
        name: d.name.unwrap_or_default(),
        token_id: d.labelhash.map(hex).unwrap_or_default(),
        owner,
        resolved_address,
        registrant,
        expiry_date: d.expiry_date.map(date_from_logic),
        registration_date: date_from_logic(d.registration_date),
        other_addresses: d.other_addresses.0.into_iter().collect(),
    })
}

pub fn domain_from_logic(d: Domain) -> Result<proto::Domain, ConversionError> {
    let owner = Some(proto::Address { hash: d.owner });
    let resolved_address = d.resolved_address.map(|resolved_address| proto::Address {
        hash: resolved_address,
    });
    Ok(proto::Domain {
        id: d.id,
        name: d.name.unwrap_or_default(),
        owner,
        resolved_address,
        expiry_date: d.expiry_date.map(date_from_logic),
        registration_date: date_from_logic(d.registration_date),
    })
}

fn address_from_str_inner(addr: &str) -> Result<Address, ConversionError> {
    let address = blockscout_display_bytes::Bytes::from_str(addr)
        .map_err(|_| ConversionError::UserRequest(format!("invalid address '{addr}'")))?;
    Ok(Address::from_slice(&address))
}

fn date_from_logic(d: chrono::DateTime<chrono::Utc>) -> String {
    d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
