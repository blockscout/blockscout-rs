use super::ConversionError;
use crate::conversion::order_direction_from_inner;
use bens_logic::{
    entity::subgraph::domain::Domain,
    subgraphs_reader::{
        BatchResolveAddressNamesInput, DomainPaginationInput, DomainSortField, DomainToken,
        DomainTokenType, GetDomainInput, GetDomainOutput, LookupAddressInput, LookupDomainInput,
    },
};
use bens_proto::blockscout::bens::v1 as proto;
use ethers::types::Address;
use std::str::FromStr;

const DEFAULT_PAGE_SIZE: u32 = 50;

pub fn get_domain_input_from_inner(
    inner: proto::GetDomainRequest,
) -> Result<GetDomainInput, ConversionError> {
    let name = name_from_inner(inner.name)?;
    Ok(GetDomainInput {
        network_id: inner.chain_id,
        name,
        only_active: inner.only_active,
    })
}

pub fn lookup_domain_name_from_inner(
    inner: proto::LookupDomainNameRequest,
) -> Result<LookupDomainInput, ConversionError> {
    let sort = domain_sort_from_inner(&inner.sort)?;
    let order = order_direction_from_inner(inner.order());
    let name = inner.name.map(name_from_inner).transpose()?;
    Ok(LookupDomainInput {
        network_id: inner.chain_id,
        name,
        only_active: inner.only_active,
        pagination: DomainPaginationInput {
            sort,
            order,
            page_size: page_size_from_inner(inner.page_size),
            page_token: inner.page_token,
        },
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
        pagination: DomainPaginationInput {
            sort,
            order,
            page_size: page_size_from_inner(inner.page_size),
            page_token: inner.page_token,
        },
    })
}

pub fn domain_sort_from_inner(inner: &str) -> Result<DomainSortField, ConversionError> {
    match inner {
        "" | "registration_date" | "registrationDate" => Ok(DomainSortField::RegistrationDate),
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
    output: GetDomainOutput,
) -> Result<proto::DetailedDomain, ConversionError> {
    let domain = output.domain;
    let owner = Some(proto::Address { hash: domain.owner });
    let resolved_address = domain
        .resolved_address
        .map(|resolved_address| proto::Address {
            hash: resolved_address,
        });
    let wrapped_owner = domain.wrapped_owner.map(|wrapped_owner| proto::Address {
        hash: wrapped_owner,
    });
    let registrant = domain
        .registrant
        .map(|registrant| proto::Address { hash: registrant });
    let tokens = output
        .tokens
        .into_iter()
        .map(domain_token_from_logic)
        .collect();
    Ok(proto::DetailedDomain {
        id: domain.id,
        name: domain.name.unwrap_or_default(),
        owner,
        resolved_address,
        registrant,
        wrapped_owner,
        expiry_date: domain.expiry_date.map(date_from_logic),
        registration_date: date_from_logic(domain.registration_date),
        other_addresses: domain.other_addresses.0.into_iter().collect(),
        tokens,
    })
}

pub fn domain_from_logic(d: Domain) -> Result<proto::Domain, ConversionError> {
    let owner = Some(proto::Address { hash: d.owner });
    let resolved_address = d.resolved_address.map(|resolved_address| proto::Address {
        hash: resolved_address,
    });
    let wrapped_owner = d.wrapped_owner.map(|wrapped_owner| proto::Address {
        hash: wrapped_owner,
    });
    Ok(proto::Domain {
        id: d.id,
        name: d.name.unwrap_or_default(),
        owner,
        wrapped_owner,
        resolved_address,
        expiry_date: d.expiry_date.map(date_from_logic),
        registration_date: date_from_logic(d.registration_date),
    })
}

pub fn pagination_from_logic(
    page_token: Option<String>,
    page_size: u32,
) -> Option<proto::Pagination> {
    page_token.map(|page_token| proto::Pagination {
        page_size,
        page_token,
    })
}

pub fn address_from_str_inner(addr: &str) -> Result<Address, ConversionError> {
    Address::from_str(addr)
        .map_err(|e| ConversionError::UserRequest(format!("invalid address '{addr}': {e}")))
}

fn name_from_inner(name: String) -> Result<String, ConversionError> {
    let name = name.trim_matches('.');
    if name.is_empty() {
        return Err(ConversionError::UserRequest(
            "empty name provided".to_string(),
        ));
    };
    Ok(name.to_string())
}

fn page_size_from_inner(page_size: Option<u32>) -> u32 {
    page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, 100)
}

fn date_from_logic(d: chrono::DateTime<chrono::Utc>) -> String {
    d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn domain_token_from_logic(t: DomainToken) -> proto::Token {
    proto::Token {
        id: t.id,
        contract_hash: format!("{:#x}", t.contract),
        r#type: domain_token_type_from_logic(t._type).into(),
    }
}

fn domain_token_type_from_logic(t: DomainTokenType) -> proto::TokenType {
    match t {
        DomainTokenType::Native => proto::TokenType::NativeDomainToken,
        DomainTokenType::Wrapped => proto::TokenType::WrappedDomainToken,
    }
}
