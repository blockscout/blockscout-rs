use crate::conversion::{
    self, batch_resolve_from_inner, batch_resolve_from_logic, from_resolved_domains_result,
    map_convertion_error, map_protocol_error, map_subgraph_error, pagination_from_logic,
};
use async_trait::async_trait;
use bens_logic::subgraph::SubgraphReader;
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
            .map(|d| conversion::detailed_domain_from_logic(d, Some(chain_id)))
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
            .map(|e| conversion::event_from_logic(e, Some(chain_id)))
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
        let domains = from_resolved_domains_result(result.items, Some(chain_id))?;
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
        let items = from_resolved_domains_result(result.items, Some(chain_id))?;
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
            .map(|d| conversion::detailed_domain_from_logic(d, Some(chain_id)))
            .transpose()
            .map_err(map_convertion_error)?;

        let resolved_domains_count = self
            .subgraph_reader
            .count_domains_by_address(input.address, true, false, Some(chain_id), input.protocols)
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
        let response =
            batch_resolve_from_logic(names, Some(chain_id)).map_err(map_convertion_error)?;
        Ok(tonic::Response::new(response))
    }

    async fn get_protocols(
        &self,
        request: tonic::Request<GetProtocolsRequest>,
    ) -> Result<tonic::Response<GetProtocolsResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let protocols = self
            .subgraph_reader
            .protocols_of_network(chain_id)
            .map_err(map_protocol_error)?;
        let response = GetProtocolsResponse {
            items: protocols
                .into_iter()
                .map(|p| {
                    conversion::protocol_from_logic(
                        p.protocol.clone(),
                        p.deployment_network.clone(),
                    )
                })
                .collect(),
        };
        Ok(tonic::Response::new(response))
    }
}
