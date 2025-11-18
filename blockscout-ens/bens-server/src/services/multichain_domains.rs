use crate::conversion::{self};
use async_trait::async_trait;
use bens_logic::subgraph::SubgraphReader;
use bens_proto::blockscout::bens::v1::{multichain_domains_server::MultichainDomains, *};
use std::sync::Arc;

#[derive(derive_new::new)]
pub struct MultichainDomainsService {
    pub subgraph_reader: Arc<SubgraphReader>,
}

#[async_trait]
impl MultichainDomains for MultichainDomainsService {
    async fn get_domain_name_multichain(
        &self,
        request: tonic::Request<GetDomainNameMultichainRequest>,
    ) -> Result<tonic::Response<DetailedDomain>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_get_domain_input_from_proto(request)
            .map_err(conversion::map_convertion_error)?;
        let domain = self
            .subgraph_reader
            .get_domain(input)
            .await
            .map_err(conversion::map_subgraph_error)?
            .map(|d| conversion::detailed_domain_from_logic(d, chain_id))
            .transpose()
            .map_err(conversion::map_convertion_error)?
            .ok_or_else(|| tonic::Status::not_found("domain not found"))?;
        Ok(tonic::Response::new(domain))
    }

    async fn list_domain_events_multichain(
        &self,
        request: tonic::Request<ListDomainEventsMultichainRequest>,
    ) -> Result<tonic::Response<ListDomainEventsResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_list_domain_events_from_proto(request)
            .map_err(conversion::map_convertion_error)?;
        let items: Vec<DomainEvent> = self
            .subgraph_reader
            .get_domain_history(input)
            .await
            .map_err(conversion::map_subgraph_error)?
            .into_iter()
            .map(|e| conversion::event_from_logic(e, chain_id))
            .collect::<Result<_, _>>()
            .map_err(conversion::map_convertion_error)?;
        let response = ListDomainEventsResponse { items };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_domain_name_multichain(
        &self,
        request: tonic::Request<LookupDomainNameMultichainRequest>,
    ) -> Result<tonic::Response<LookupDomainNameResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_lookup_domain_name_from_proto(request)
            .map_err(conversion::map_convertion_error)?;
        let page_size = input.pagination.page_size;
        let result = self
            .subgraph_reader
            .lookup_domain_name(input)
            .await
            .map_err(conversion::map_subgraph_error)?;
        let domains = conversion::from_resolved_domains_result(result.items, chain_id)?;
        let response = LookupDomainNameResponse {
            items: domains,
            next_page_params: conversion::pagination_from_logic(result.next_page_token, page_size),
        };
        Ok(tonic::Response::new(response))
    }

    async fn lookup_address_multichain(
        &self,
        request: tonic::Request<LookupAddressMultichainRequest>,
    ) -> Result<tonic::Response<LookupAddressResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_lookup_address_from_proto(request)
            .map_err(conversion::map_convertion_error)?;
        let page_size = input.pagination.page_size;
        let result = self
            .subgraph_reader
            .lookup_address(input)
            .await
            .map_err(conversion::map_subgraph_error)?;
        let domains = conversion::from_resolved_domains_result(result.items, chain_id)?;
        let response = LookupAddressResponse {
            items: domains,
            next_page_params: conversion::pagination_from_logic(result.next_page_token, page_size),
        };
        Ok(tonic::Response::new(response))
    }

    async fn get_address_multichain(
        &self,
        request: tonic::Request<GetAddressMultichainRequest>,
    ) -> Result<tonic::Response<GetAddressResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_get_address_from_proto(request)
            .map_err(conversion::map_convertion_error)?;

        let domain = self
            .subgraph_reader
            .get_address(input.clone())
            .await
            .map_err(conversion::map_subgraph_error)?
            .map(|d| conversion::detailed_domain_from_logic(d, chain_id))
            .transpose()
            .map_err(conversion::map_convertion_error)?;

        let resolved_domains_count = self
            .subgraph_reader
            .count_domains_by_address(input.address, true, false, chain_id, input.protocols)
            .await
            .map_err(conversion::map_subgraph_error)? as i32;
        Ok(tonic::Response::new(GetAddressResponse {
            domain,
            resolved_domains_count,
        }))
    }

    async fn batch_resolve_addresses_multichain(
        &self,
        request: tonic::Request<BatchResolveAddressesMultichainRequest>,
    ) -> Result<tonic::Response<BatchResolveAddressNamesResponse>, tonic::Status> {
        let request = request.into_inner();
        let chain_id = request.chain_id;
        let input = conversion::multichain_batch_resolve_addresses_from_proto(request)
            .map_err(conversion::map_convertion_error)?;
        let names = self
            .subgraph_reader
            .batch_resolve_address_names(input)
            .await
            .map_err(conversion::map_subgraph_error)?;
        let response = conversion::batch_resolve_from_logic(names, chain_id)
            .map_err(conversion::map_convertion_error)?;
        Ok(tonic::Response::new(response))
    }

    async fn get_mutlichain_protocols(
        &self,
        request: tonic::Request<GetProtocolsMultichainRequest>,
    ) -> Result<tonic::Response<GetProtocolsResponse>, tonic::Status> {
        let request = request.into_inner();
        let protocols = match request.chain_id {
            Some(chain_id) => self
                .subgraph_reader
                .protocols_of_network(chain_id)
                .map_err(conversion::map_protocol_error)?
                .into_iter()
                .collect::<Vec<_>>(),
            None => self
                .subgraph_reader
                .iter_deployed_protocols()
                .collect::<Vec<_>>(),
        };
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
