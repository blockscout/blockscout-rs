// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::types::ChainId;
use actix_phoenix_channel::{ChannelConn, ChannelEvent, ChannelHandler};
use serde::Serialize;

pub const NEW_BLOCKS_TOPIC: &str = "blocks:new_blocks";

pub struct Channel;

#[async_trait::async_trait]
impl ChannelHandler for Channel {
    async fn join_channel(&self, conn: &ChannelConn, event: ChannelEvent) {
        if event.topic() == NEW_BLOCKS_TOPIC {
            conn.client().allow_join(&event, &()).await;
        }
    }
}

#[derive(Serialize)]
pub struct LatestBlockUpdateMessage {
    pub chain_id: ChainId,
    pub block_number: i64,
}
