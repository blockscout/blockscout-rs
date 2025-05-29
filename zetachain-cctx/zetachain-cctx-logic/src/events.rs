use async_trait::async_trait;
use crate::models::{CompleteCctx};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CctxListItem as CctxListItemProto;

#[async_trait]
pub trait EventBroadcaster: Send + Sync {
    /// Broadcast that a CCTX has been updated
    async fn broadcast_cctx_update(&self, cctx_index: String, cctx_data: CompleteCctx);
    
    /// Broadcast that new CCTXs have been imported
    async fn broadcast_new_cctxs(&self, cctxs: Vec<CctxListItemProto>);
}

/// No-op implementation for when WebSocket events are disabled
pub struct NoOpBroadcaster;

#[async_trait]
impl EventBroadcaster for NoOpBroadcaster {
    async fn broadcast_cctx_update(&self, _cctx_index: String, _cctx_data: CompleteCctx) {
        // No-op
    }
    
    async fn broadcast_new_cctxs(&self, _cctxs: Vec<CctxListItemProto>) {
        // No-op
    }
} 