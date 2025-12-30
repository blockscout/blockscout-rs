use crate::conversion::{
    address_from_str_inner, event_sort_from_inner, order_direction_from_inner,
    pagination_from_proto_by_fields, ConversionError,
};
use bens_logic::subgraph::{
    BatchResolveAddressNamesInput, GetAddressInput, GetDomainHistoryInput, GetDomainInput,
    LookupAddressInput, LookupDomainInput,
};
use bens_proto::blockscout::bens::v1 as proto;
use nonempty::NonEmpty;

pub fn multichain_get_domain_input_from_proto(
    proto: proto::GetDomainNameMultichainRequest,
) -> Result<GetDomainInput, ConversionError> {
    Ok(GetDomainInput {
        name: proto.name,
        only_active: proto.only_active,
        network_id: proto.chain_id,
        protocol_id: proto.protocol_id,
    })
}

pub fn multichain_lookup_domain_name_from_proto(
    proto: proto::LookupDomainNameMultichainRequest,
) -> Result<LookupDomainInput, ConversionError> {
    let pagination = pagination_from_proto_by_fields(
        &proto.sort,
        proto.order(),
        proto.page_size,
        proto.page_token,
    )?;
    let input = LookupDomainInput {
        name: proto.name,
        only_active: proto.only_active,
        network_id: proto.chain_id,
        protocols: split_protocols(proto.protocols),
        pagination,
    };
    Ok(input)
}

pub fn multichain_list_domain_events_from_proto(
    proto: proto::ListDomainEventsMultichainRequest,
) -> Result<GetDomainHistoryInput, ConversionError> {
    let order = order_direction_from_inner(proto.order());
    let sort = event_sort_from_inner(&proto.sort)?;
    let input = GetDomainHistoryInput {
        name: proto.name,
        network_id: proto.chain_id,
        protocol_id: proto.protocol_id,
        order,
        sort,
    };
    Ok(input)
}

pub fn multichain_lookup_address_from_proto(
    proto: proto::LookupAddressMultichainRequest,
) -> Result<LookupAddressInput, ConversionError> {
    let pagination = pagination_from_proto_by_fields(
        &proto.sort,
        proto.order(),
        proto.page_size,
        proto.page_token,
    )?;
    let input = LookupAddressInput {
        address: address_from_str_inner(&proto.address)?,
        resolved_to: proto.resolved_to,
        owned_by: proto.owned_by,
        only_active: proto.only_active,
        network_id: proto.chain_id,
        protocols: split_protocols(proto.protocols),
        pagination,
    };
    Ok(input)
}

pub fn multichain_get_address_from_proto(
    proto: proto::GetAddressMultichainRequest,
) -> Result<GetAddressInput, ConversionError> {
    let input = GetAddressInput {
        address: address_from_str_inner(&proto.address)?,
        network_id: proto.chain_id,
        protocols: split_protocols(proto.protocols),
    };
    Ok(input)
}

pub fn multichain_batch_resolve_addresses_from_proto(
    proto: proto::BatchResolveAddressesMultichainRequest,
) -> Result<BatchResolveAddressNamesInput, ConversionError> {
    let input = BatchResolveAddressNamesInput {
        addresses: proto
            .addresses
            .iter()
            .map(|addr| address_from_str_inner(addr))
            .collect::<Result<_, _>>()?,
        network_id: proto.chain_id,
        protocols: split_protocols(proto.protocols),
    };
    Ok(input)
}

fn split_protocols(protocols: Option<String>) -> Option<NonEmpty<String>> {
    if let Some(protocols) = protocols {
        NonEmpty::collect(protocols.split(',').map(|p| p.to_string()))
    } else {
        None
    }
}
