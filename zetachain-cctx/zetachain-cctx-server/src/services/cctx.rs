use std::sync::Arc;

use tonic::{Request, Response, Status};

use zetachain_cctx_logic::database::{ZetachainCctxDatabase};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    cctx_info_service_server::CctxInfoService, CallOptions, CrossChainTx, GetCctxInfoRequest, InboundParams, ListCctxsRequest, ListCctxsResponse, OutboundParams, RevertOptions, Status as CCTXStatus
};




pub struct CctxService {
    database: Arc<ZetachainCctxDatabase>,
}

impl CctxService {
    pub fn new(database: Arc<ZetachainCctxDatabase>) -> Self {
        Self { database }
    }
}

#[async_trait::async_trait]
impl CctxInfoService for CctxService {


    async fn list_cctxs(&self, request: Request<ListCctxsRequest>) -> Result<Response<ListCctxsResponse>, Status> {
        let request = request.into_inner();

        let cctx_items = self.database.list_cctxs(request.limit, request.status).await.map_err(|e| Status::internal(e.to_string()))?;

        // Convert CctxListItem to CrossChainTx with only required fields
        let cctxs: Vec<CrossChainTx> = cctx_items.into_iter().map(|item| {
            CrossChainTx {
                creator: String::new(), // Not required for list view
                index: item.index,
                zeta_fees: String::new(), // Not required for list view
                relayed_message: String::new(), // Not required for list view
                cctx_status: Some(CCTXStatus {
                    status: item.status.into(),
                    status_message: String::new(),
                    error_message: String::new(),
                    last_update_timestamp: 0,
                    is_abort_refunded: false,
                    created_timestamp: 0,
                    error_message_abort: String::new(),
                    error_message_revert: String::new(),
                }),
                inbound_params: Some(InboundParams {
                    sender: String::new(),
                    sender_chain_id: item.source_chain_id.parse().unwrap_or(0),
                    tx_origin: String::new(),
                    coin_type: 0.into(), // Default to Zeta
                    asset: String::new(),
                    amount: String::new(),
                    observed_hash: String::new(),
                    observed_external_height: 0,
                    ballot_index: String::new(),
                    finalized_zeta_height: 0,
                    tx_finalization_status: 0.into(),
                    is_cross_chain_call: false,
                    status: 0.into(),
                    confirmation_mode: 0.into(),
                }),
                outbound_params: vec![OutboundParams {
                    receiver: String::new(),
                    receiver_chain_id: item.target_chain_id.parse().unwrap_or(0),
                    coin_type: 0.into(), // Default to Zeta
                    amount: item.amount,
                    tss_nonce: 0,
                    gas_limit: 0,
                    gas_price: String::new(),
                    gas_priority_fee: String::new(),
                    hash: String::new(),
                    ballot_index: String::new(),
                    observed_external_height: 0,
                    gas_used: 0,
                    effective_gas_price: String::new(),
                    effective_gas_limit: 0,
                    tss_pubkey: String::new(),
                    tx_finalization_status: 0.into(),
                    call_options: Some(CallOptions {
                        gas_limit: 0,
                        is_arbitrary_call: false,
                    }),
                    confirmation_mode: 0.into(),
                }],
                protocol_contract_version: 0.into(),
                revert_options: Some(RevertOptions {
                    revert_address: String::new(),
                    call_on_revert: false,
                    abort_address: String::new(),
                    revert_message: vec![].into(),
                    revert_gas_limit: String::new(),
                }),
            }
        }).collect();

        Ok(Response::new(ListCctxsResponse {
            cctxs
        }))
    }
    async fn get_cctx_info(&self, request: Request<GetCctxInfoRequest>) -> Result<Response<CrossChainTx>, Status> {

        let request = request.into_inner();

        let complete_cctx = self
        .database
        .get_complete_cctx(request.cctx_id)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        if complete_cctx.is_none() {
            return Err(Status::not_found("CCTX not found"));
        }

        let entity = complete_cctx.unwrap();

        let cctx= 
            CrossChainTx {
                creator: entity.cctx.creator,
                index: entity.cctx.index,
                zeta_fees: entity.cctx.zeta_fees,
                relayed_message: entity.cctx.relayed_message.unwrap_or_default(),
                cctx_status: Some(CCTXStatus {
                    status: entity.status.status.into(),
                    status_message: entity.status.status_message.unwrap_or_default(),
                    error_message: entity.status.error_message.unwrap_or_default(),
                    last_update_timestamp: entity.status.last_update_timestamp.and_utc().timestamp(),
                    is_abort_refunded: entity.status.is_abort_refunded,
                    created_timestamp: entity.status.created_timestamp,
                    error_message_abort: entity.status.error_message_abort.unwrap_or_default(),
                    error_message_revert: entity.status.error_message_revert.unwrap_or_default(),
                }),
                inbound_params: Some(InboundParams {
                    sender: entity.inbound.sender,
                    sender_chain_id: entity.inbound.sender_chain_id.parse().unwrap_or(0),
                    tx_origin: entity.inbound.tx_origin,
                    coin_type: entity.inbound.coin_type.into(),
                    asset: entity.inbound.asset.unwrap_or_default(),
                    amount: entity.inbound.amount,
                    observed_hash: entity.inbound.observed_hash,
                    observed_external_height: entity.inbound.observed_external_height.parse().unwrap_or(0),
                    ballot_index: entity.inbound.ballot_index,
                    finalized_zeta_height: entity.inbound.finalized_zeta_height.parse().unwrap_or(0),
                    tx_finalization_status: entity.inbound.tx_finalization_status.into(),
                    is_cross_chain_call: entity.inbound.is_cross_chain_call,
                    status: entity.inbound.status.into(),   
                    confirmation_mode: entity.inbound.confirmation_mode.into(),
                }),
                outbound_params: entity.outbounds.into_iter().map(|o| OutboundParams {
                    receiver: o.receiver,
                    receiver_chain_id: o.receiver_chain_id.parse().unwrap_or(0),
                    coin_type: o.coin_type.into(),
                    amount: o.amount,
                    tss_nonce: o.tss_nonce.parse().unwrap_or(0),
                    gas_limit: o.gas_limit.parse().unwrap_or(0),
                    gas_price: o.gas_price.unwrap_or_default(),
                    gas_priority_fee: o.gas_priority_fee.unwrap_or_default(),
                    hash: o.hash,
                    ballot_index: o.ballot_index.unwrap_or_default(),
                    observed_external_height: o.observed_external_height.parse().unwrap_or(0),
                    gas_used: o.gas_used.parse().unwrap_or(0),
                    effective_gas_price: o.effective_gas_price,
                    effective_gas_limit: o.effective_gas_limit.parse().unwrap_or(0),
                    tss_pubkey: o.tss_pubkey,
                    tx_finalization_status: o.tx_finalization_status.into(),
                    call_options: Some(CallOptions {
                        gas_limit: o.call_options_gas_limit.unwrap_or("0".to_string()).parse().unwrap(),
                        is_arbitrary_call: o.call_options_is_arbitrary_call.unwrap_or_default(),
                    }),
                    confirmation_mode: o.confirmation_mode.into(),
                }).collect(),
                protocol_contract_version: entity.cctx.protocol_contract_version.into(),
                revert_options: Some(RevertOptions{
                    revert_address: entity.revert.revert_address.unwrap_or_default(),
                    call_on_revert: entity.revert.call_on_revert,
                    abort_address: entity.revert.abort_address.unwrap_or_default(),
                    revert_message: entity.revert.revert_message.unwrap_or_default().as_bytes().to_vec().into(),
                    revert_gas_limit: entity.revert.revert_gas_limit,
                }),
            };


        
        Ok(Response::new(
            cctx,
        ))
    }
}






