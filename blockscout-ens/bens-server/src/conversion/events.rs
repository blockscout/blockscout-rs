use bens_logic::{
    entity::subgraph::domain_event::DomainEvent,
    hash_name::hex,
    subgraphs_reader::{EventSort, GetDomainHistoryInput},
};
use bens_proto::blockscout::bens::v1 as proto;

use super::{order_direction_from_inner, ConversionError};

pub fn list_domain_events_from_inner(
    inner: proto::ListDomainEventsRequest,
) -> Result<GetDomainHistoryInput, ConversionError> {
    let sort = event_sort_from_inner(&inner.sort)?;
    let order = order_direction_from_inner(inner.order());
    Ok(GetDomainHistoryInput {
        network_id: inner.chain_id,
        name: inner.name,
        sort,
        order,
    })
}

pub fn event_from_logic(e: DomainEvent) -> Result<proto::DomainEvent, ConversionError> {
    let from_address = Some(proto::Address {
        hash: hex(e.from_address),
    });
    Ok(proto::DomainEvent {
        transaction_hash: hex(e.transaction_hash),
        timestamp: e.timestamp,
        from_address,
        action: e.method,
    })
}

pub fn event_sort_from_inner(inner: &str) -> Result<EventSort, ConversionError> {
    match inner {
        "" | "timestamp" => Ok(EventSort::BlockNumber),
        _ => Err(ConversionError::UserRequest(format!(
            "unknow sort field '{inner}'"
        ))),
    }
}
