use crate::types::ChainId;
use actix_phoenix_channel::{ChannelConn, ChannelEvent, ChannelHandler};
use serde::Serialize;

pub const NEW_BLOCKS_TOPIC: &str = "blocks:new_blocks";
pub const NEW_INTEROP_MESSAGES_TOPIC: &str = "interop_messages:new_messages";

pub struct Channel;

#[async_trait::async_trait]
impl ChannelHandler for Channel {
    async fn join_channel(&self, conn: &ChannelConn, event: ChannelEvent) {
        match event.topic() {
            NEW_BLOCKS_TOPIC | NEW_INTEROP_MESSAGES_TOPIC => {
                conn.client().allow_join(&event, &()).await;
            }
            _ => {}
        }
    }
}

#[derive(Serialize)]
pub struct LatestBlockUpdateMessage {
    pub chain_id: ChainId,
    pub block_number: i32,
}
