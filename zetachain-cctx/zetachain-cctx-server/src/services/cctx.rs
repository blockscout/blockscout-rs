use std::sync::Arc;

use tonic::{Request, Response, Status};

use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_logic::models::{ Filters};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::Direction;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    cctx_info_server::CctxInfo, CrossChainTx, GetCctxInfoRequest, ListCctxsRequest, ListCctxsResponse,
};

pub struct CctxService {
    database: Arc<ZetachainCctxDatabase>,
}

impl CctxService {
    pub fn new(database: Arc<ZetachainCctxDatabase>) -> Self {
        Self { database }
    }
}

/// Transforms a CompleteCctx from the database into a CrossChainTx for the API response
// pub fn transform_complete_cctx_to_cross_chain_tx(
//     entity: CompleteCctx,
// ) -> Result<CrossChainTx, String> {
//     let status:i32 = entity.status.status.into();
//     let status = CctxStatus::try_from(status).map_err(|e| e.to_string())?;
//     let status_reduced = reduce_status(status);
//     let cctx = CrossChainTx {
//         creator: entity.cctx.creator,
//         index: entity.cctx.index,
//         zeta_fees: entity.cctx.zeta_fees,
//         relayed_message: entity.cctx.relayed_message.unwrap_or_default(),
//         cctx_status: Some(CCTXStatus {
//             status: status.into(),
//             status_message: entity.status.status_message.unwrap_or_default(),
//             error_message: entity.status.error_message.unwrap_or_default(),
//             last_update_timestamp: entity.status.last_update_timestamp.and_utc().timestamp(),
//             is_abort_refunded: entity.status.is_abort_refunded,
//             created_timestamp: entity.status.created_timestamp,
//             error_message_abort: entity.status.error_message_abort.unwrap_or_default(),
//             error_message_revert: entity.status.error_message_revert.unwrap_or_default(),
//         }),
//         cctx_status_reduced: status_reduced.into(),
//         inbound_params: Some(InboundParams {
//             sender: entity.inbound.sender,
//             sender_chain_id: entity.inbound.sender_chain_id.parse().unwrap_or(0),
//             tx_origin: entity.inbound.tx_origin,
//             coin_type: entity.inbound.coin_type.into(),
//             asset: entity.inbound.asset.unwrap_or_default(),
//             amount: entity.inbound.amount,
//             observed_hash: entity.inbound.observed_hash,
//             observed_external_height: entity.inbound.observed_external_height.parse().unwrap_or(0),
//             ballot_index: entity.inbound.ballot_index,
//             finalized_zeta_height: entity.inbound.finalized_zeta_height.parse().unwrap_or(0),
//             tx_finalization_status: entity.inbound.tx_finalization_status.into(),
//             is_cross_chain_call: entity.inbound.is_cross_chain_call,
//             status: entity.inbound.status.into(),
//             confirmation_mode: entity.inbound.confirmation_mode.into(),
//         }),
//         outbound_params: entity
//             .outbounds
//             .into_iter()
//             .map(|o| OutboundParams {
//                 receiver: o.receiver,
//                 receiver_chain_id: o.receiver_chain_id.parse().unwrap_or(0),
//                 coin_type: o.coin_type.into(),
//                 amount: o.amount,
//                 tss_nonce: o.tss_nonce.parse().unwrap_or(0),
//                 gas_limit: o.gas_limit.parse().unwrap_or(0),
//                 gas_price: o.gas_price.unwrap_or_default(),
//                 gas_priority_fee: o.gas_priority_fee.unwrap_or_default(),
//                 hash: o.hash.unwrap_or_default(),
//                 ballot_index: o.ballot_index.unwrap_or_default(),
//                 observed_external_height: o.observed_external_height.parse().unwrap_or(0),
//                 gas_used: o.gas_used.parse().unwrap_or(0),
//                 effective_gas_price: o.effective_gas_price,
//                 effective_gas_limit: o.effective_gas_limit.parse().unwrap_or(0),
//                 tss_pubkey: o.tss_pubkey,
//                 tx_finalization_status: o.tx_finalization_status.into(),
//                 call_options: Some(CallOptions {
//                     gas_limit: o
//                         .call_options_gas_limit
//                         .unwrap_or("0".to_string())
//                         .parse()
//                         .unwrap(),
//                     is_arbitrary_call: o.call_options_is_arbitrary_call.unwrap_or_default(),
//                 }),
//                 confirmation_mode: o.confirmation_mode.into(),
//             })
//             .collect(),
//         protocol_contract_version: entity.cctx.protocol_contract_version.into(),
//         revert_options: Some(RevertOptions {
//             revert_address: entity.revert.revert_address.unwrap_or_default(),
//             call_on_revert: entity.revert.call_on_revert,
//             abort_address: entity.revert.abort_address.unwrap_or_default(),
//             revert_message: entity
//                 .revert
//                 .revert_message
//                 .unwrap_or_default()
//                 .as_bytes()
//                 .to_vec()
//                 .into(),
//             revert_gas_limit: entity.revert.revert_gas_limit,
//         }),
//         related_cctxs: entity
//             .related
//             .into_iter()
//             .map(|r| -> Result<RelatedCctx, String> {
//                 Ok(RelatedCctx {
//                     index: r.index.clone(),
//                     depth: r.depth,
//                     source_chain_id: r.source_chain_id,
//                     status: CctxStatus::from_str_name(r.status.as_str())
//                         .ok_or(format!(
//                             "RelatedCctx: Index: {}, Invalid status: {}",
//                             &r.index, r.status
//                         ))?
//                         .into(),
//                     inbound_amount: r.inbound_amount,
//                     inbound_coin_type: CoinType::from_str_name(r.inbound_coin_type.as_str())
//                         .ok_or(format!(
//                             "RelatedCctx: Index: {}, Invalid coin type: {}",
//                             &r.index, r.inbound_coin_type
//                         ))?
//                         .into(),
//                     outbound_params: r
//                         .outbound_params
//                         .into_iter()
//                         .map(|o| {
//                             Ok(RelatedOutboundParams {
//                                 amount: o.amount,
//                                 chain_id: o.chain_id,
//                                 coin_type: CoinType::from_str_name(o.coin_type.as_str())
//                                     .ok_or(format!(
//                                         "RelatedOutbound: Index: {}, Invalid coin type: {}",
//                                         &r.index, o.coin_type
//                                     ))?
//                                     .into(),
//                                 gas_used: o.gas_used.parse().unwrap_or(0),
//                                 token: o.token.unwrap_or_default(),
//                             })
//                         })
//                         .collect::<Result<Vec<RelatedOutboundParams>, String>>()?,
//                 })
//             })
//             .collect::<Result<Vec<RelatedCctx>, String>>()?,
//     };

//     Ok(cctx)
// }

#[async_trait::async_trait]
impl CctxInfo for CctxService {
    async fn list_cctxs(
        &self,
        request: Request<ListCctxsRequest>,
    ) -> Result<Response<ListCctxsResponse>, Status> {
        let request = request.into_inner();

        // Helper function to parse comma-separated values
        let parse_comma_separated = |opt_str: Option<String>| -> Vec<String> {
            match opt_str {
                Some(s) if !s.trim().is_empty() => s
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .filter(|item| !item.is_empty())
                    .collect(),
                _ => Vec::new(),
            }
        };

        let source_chain_id:Result<Vec<i32>, _> = parse_comma_separated(request.source_chain_id)
            .into_iter()
            .map(|s| s.parse::<i32>())
            .collect::<Result<Vec<i32>, _>>();

        let target_chain_id:Result<Vec<i32>, _> = parse_comma_separated(request.target_chain_id)
            .into_iter()
            .map(|s| s.parse::<i32>())
            .collect::<Result<Vec<i32>, _>>();
        
        let filters = Filters {
            status_reduced: parse_comma_separated(request.status_reduced),
            sender_address: parse_comma_separated(request.sender_address),
            receiver_address: parse_comma_separated(request.receiver_address),
            asset: parse_comma_separated(request.asset),
            coin_type: parse_comma_separated(request.coin_type),
            source_chain_id: source_chain_id.map_err(|e| Status::invalid_argument(e.to_string()))? ,
            target_chain_id: target_chain_id.map_err(|e| Status::invalid_argument(e.to_string()))?,
            token_symbol: parse_comma_separated(request.token_symbol),
            start_timestamp: request.start_timestamp,
            end_timestamp: request.end_timestamp,
            hash: None
        };

        let direction = Direction::try_from(request.direction).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let content =self.database
            .list_cctxs(request.limit, request.page_key, filters, direction)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(content))
    }
    async fn get_cctx_info(
        &self,
        request: Request<GetCctxInfoRequest>,
    ) -> Result<Response<CrossChainTx>, Status> {
        let request = request.into_inner();

        let cctx = self
            .database
            .get_complete_cctx(request.cctx_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or(Status::not_found("Cctx not found"))?;

        Ok(Response::new(cctx))
    }
}
