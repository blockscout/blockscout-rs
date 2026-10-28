use actix_phoenix_channel::{ChannelConn, ChannelEvent, ChannelHandler};

pub const NEW_CCTXS_TOPIC: &str = "cctxs:new_cctxs";
pub const CCTX_STATUS_UPDATE_TOPIC: &str = "cctxs:status_update";

pub struct Channel;

#[async_trait::async_trait]
impl ChannelHandler for Channel {
    async fn join_channel(&self, conn: &ChannelConn, event: ChannelEvent) {
        match event.topic() {
            NEW_CCTXS_TOPIC | CCTX_STATUS_UPDATE_TOPIC => {
                conn.client().allow_join(&event, &()).await;
            }
            _ => {}
        }
    }
}
