use crate::conversion::{self, batch_resolve_from_inner, ConversionError};
use async_trait::async_trait;
use bens_logic::{
    entity,
    subgraphs_reader::{SubgraphReadError, SubgraphReader},
};
use bens_proto::blockscout::bens::v1::{
    domains_extractor_server::DomainsExtractor, BatchResolveAddressNamesRequest,
    BatchResolveAddressNamesResponse, DetailedDomain, Domain, DomainEvent, GetDomainRequest,
    ListDomainEventsRequest, ListDomainEventsResponse, LookupAddressRequest, LookupAddressResponse,
    LookupDomainNameRequest, LookupDomainNameResponse, Pagination,
};

pub struct DomainsExtractorService {
    pub subgraph_reader: SubgraphReader,
}

impl DomainsExtractorService {
    pub fn new(subgraph_reader: SubgraphReader) -> Self {
        Self { subgraph_reader }
    }
}

#[async_trait]
impl DomainsExtractor for DomainsExtractorService {
    async fn get_domain(
        &self,
        request: tonic::Request<GetDomainRequest>,
    ) -> Result<tonic::Response<DetailedDomain>, tonic::Status> {
        let request = request.into_inner();
        let input =
            conversion::get_domain_input_from_inner(request).map_err(map_convertion_error)?;
        let domain = self
            .subgraph_reader
            .get_domain(input)
            .await
            .map_err(map_subgraph_error)?
            .map(conversion::detailed_domain_from_logic)
            .transpose()
            .map_err(map_convertion_error)?
            .ok_or_else(|| tonic::Status::not_found("domain not found"))?;
        Ok(tonic::Response::new(domain))
    }

    async fn list_domain_events(
        &self,
        request: tonic::Request<ListDomainEventsRequest>,
    ) -> Result<tonic::Response<ListDomainEventsResponse>, tonic::Status> {
        let request = request.into_inner();
        let input =
            conversion::list_domain_events_from_inner(request).map_err(map_convertion_error)?;
        let events: Vec<DomainEvent> = self
            .subgraph_reader
            .get_domain_history(input)
            .await
            .map_err(map_subgraph_error)?
            .into_iter()
            .map(conversion::event_from_logic)
            .collect::<Result<_, _>>()
            .map_err(map_convertion_error)?;
        let response = ListDomainEventsResponse {
            pagination: Some(Pagination {
                total_records: events.len() as u32,
            }),
            items: events,
        };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_domain_name(
        &self,
        request: tonic::Request<LookupDomainNameRequest>,
    ) -> Result<tonic::Response<LookupDomainNameResponse>, tonic::Status> {
        let request = request.into_inner();
        let input =
            conversion::lookup_domain_name_from_inner(request).map_err(map_convertion_error)?;
        let result = self.subgraph_reader.lookup_domain(input).await;
        let domains = from_resolved_domains_result(result)?;
        let response = LookupDomainNameResponse {
            pagination: Some(Pagination {
                total_records: domains.len() as u32,
            }),
            items: domains,
        };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_address(
        &self,
        request: tonic::Request<LookupAddressRequest>,
    ) -> Result<tonic::Response<LookupAddressResponse>, tonic::Status> {
        let request = request.into_inner();
        let input = conversion::lookup_address_from_inner(request).map_err(map_convertion_error)?;
        let result = self.subgraph_reader.lookup_address(input).await;
        let items = from_resolved_domains_result(result)?;
        let response = LookupAddressResponse {
            pagination: Some(Pagination {
                total_records: items.len() as u32,
            }),
            items,
        };
        Ok(tonic::Response::new(response))
    }

    async fn batch_resolve_address_names(
        &self,
        request: tonic::Request<BatchResolveAddressNamesRequest>,
    ) -> Result<tonic::Response<BatchResolveAddressNamesResponse>, tonic::Status> {
        let request = request.into_inner();
        let input = batch_resolve_from_inner(request).map_err(map_convertion_error)?;
        let names = self
            .subgraph_reader
            .batch_resolve_address_names(input)
            .await
            .map_err(map_subgraph_error)?;
        let response = BatchResolveAddressNamesResponse { names };
        Ok(tonic::Response::new(response))
    }
}

fn map_subgraph_error(err: SubgraphReadError) -> tonic::Status {
    match err {
        SubgraphReadError::NetworkNotFound(id) => {
            tonic::Status::invalid_argument(format!("network {id} not found"))
        }
        _ => {
            tracing::error!(err =? err, "error during request handle");
            tonic::Status::internal("internal error")
        }
    }
}

fn map_convertion_error(err: ConversionError) -> tonic::Status {
    match err {
        ConversionError::UserRequest(_) => tonic::Status::invalid_argument(err.to_string()),
        ConversionError::LogicOutput(_) => tonic::Status::internal(err.to_string()),
    }
}

fn from_resolved_domains_result(
    result: Result<Vec<entity::subgraph::domain::Domain>, SubgraphReadError>,
) -> Result<Vec<Domain>, tonic::Status> {
    result
        .map_err(map_subgraph_error)?
        .into_iter()
        .map(conversion::domain_from_logic)
        .collect::<Result<_, _>>()
        .map_err(map_convertion_error)
}
