use crate::conversion::{
    self, batch_resolve_from_inner, batch_resolve_from_logic, pagination_from_logic,
    ConversionError,
};
use async_trait::async_trait;
use bens_logic::subgraphs_reader::{LookupOutput, SubgraphReadError, SubgraphReader};
use bens_proto::blockscout::bens::v1::{domains_extractor_server::DomainsExtractor, *};
use std::sync::Arc;

pub struct DomainsExtractorService {
    pub subgraph_reader: Arc<SubgraphReader>,
}

impl DomainsExtractorService {
    pub fn new(subgraph_reader: Arc<SubgraphReader>) -> Self {
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
        let chain_id = request.chain_id;
        let input =
            conversion::get_domain_input_from_inner(request).map_err(map_convertion_error)?;
        let domain = self
            .subgraph_reader
            .get_domain(input)
            .await
            .map_err(map_subgraph_error)?
            .map(|d| conversion::detailed_domain_from_logic(d, chain_id))
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
        let chain_id = request.chain_id;
        let input =
            conversion::list_domain_events_from_inner(request).map_err(map_convertion_error)?;
        let items: Vec<DomainEvent> = self
            .subgraph_reader
            .get_domain_history(input)
            .await
            .map_err(map_subgraph_error)?
            .into_iter()
            .map(|e| conversion::event_from_logic(e, chain_id))
            .collect::<Result<_, _>>()
            .map_err(map_convertion_error)?;
        let response = ListDomainEventsResponse { items };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_domain_name(
        &self,
        request: tonic::Request<LookupDomainNameRequest>,
    ) -> Result<tonic::Response<LookupDomainNameResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input =
            conversion::lookup_domain_name_from_inner(request).map_err(map_convertion_error)?;
        let page_size = input.pagination.page_size;
        let result = self
            .subgraph_reader
            .lookup_domain_name(input)
            .await
            .map_err(map_subgraph_error)?;
        let domains = from_resolved_domains_result(result.items, chain_id)?;
        let response = LookupDomainNameResponse {
            items: domains,
            next_page_params: pagination_from_logic(result.next_page_token, page_size),
        };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_address(
        &self,
        request: tonic::Request<LookupAddressRequest>,
    ) -> Result<tonic::Response<LookupAddressResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::lookup_address_from_inner(request).map_err(map_convertion_error)?;
        let page_size = input.pagination.page_size;
        let result = self
            .subgraph_reader
            .lookup_address(input)
            .await
            .map_err(map_subgraph_error)?;
        let items = from_resolved_domains_result(result.items, chain_id)?;
        let response = LookupAddressResponse {
            items,
            next_page_params: pagination_from_logic(result.next_page_token, page_size),
        };
        Ok(tonic::Response::new(response))
    }

    async fn get_address(
        &self,
        request: tonic::Request<GetAddressRequest>,
    ) -> Result<tonic::Response<GetAddressResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::get_address_from_inner(request).map_err(map_convertion_error)?;

        let domain = self
            .subgraph_reader
            .get_address(input.clone())
            .await
            .map_err(map_subgraph_error)?
            .map(|d| conversion::detailed_domain_from_logic(d, chain_id))
            .transpose()
            .map_err(map_convertion_error)?;

        let resolved_domains_count = self
            .subgraph_reader
            .count_domains_by_address(chain_id, input.address, true, false)
            .await
            .map_err(map_subgraph_error)? as i32;
        Ok(tonic::Response::new(GetAddressResponse {
            domain,
            resolved_domains_count,
        }))
    }

    async fn batch_resolve_address_names(
        &self,
        request: tonic::Request<BatchResolveAddressNamesRequest>,
    ) -> Result<tonic::Response<BatchResolveAddressNamesResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = batch_resolve_from_inner(request).map_err(map_convertion_error)?;
        let names = self
            .subgraph_reader
            .batch_resolve_address_names(input)
            .await
            .map_err(map_subgraph_error)?;
        let response = batch_resolve_from_logic(names, chain_id).map_err(map_convertion_error)?;
        Ok(tonic::Response::new(response))
    }

    async fn get_protocols(
        &self,
        request: tonic::Request<GetProtocolsRequest>,
    ) -> Result<tonic::Response<GetProtocolsResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let protocols = self.subgraph_reader.protocols_of_network(chain_id);
        let response = GetProtocolsResponse {
            items: protocols
                .into_iter()
                .cloned()
                .map(conversion::protocol_from_logic)
                .collect(),
        };
        Ok(tonic::Response::new(response))
    }
}

fn map_subgraph_error(err: SubgraphReadError) -> tonic::Status {
    match err {
        SubgraphReadError::Protocol(_) => tonic::Status::invalid_argument(err.to_string()),
        SubgraphReadError::DbErr(_) | SubgraphReadError::Internal(_) => {
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
    result: impl IntoIterator<Item = LookupOutput>,
    chain_id: i64,
) -> Result<Vec<Domain>, tonic::Status> {
    result
        .into_iter()
        .map(|output| conversion::domain_from_logic(output, chain_id))
        .collect::<Result<_, _>>()
        .map_err(map_convertion_error)
}
