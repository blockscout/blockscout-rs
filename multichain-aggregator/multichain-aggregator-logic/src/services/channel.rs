use trillium_channels::{ChannelConn, ChannelEvent, ChannelHandler};

pub const NEW_BLOCKS_TOPIC: &str = "blocks:new_blocks";
pub const NEW_INTEROP_MESSAGES_TOPIC: &str = "interop_messages:new_messages";

pub struct Channel;

#[trillium::async_trait]
impl ChannelHandler for Channel {
    async fn join_channel(&self, conn: ChannelConn<'_>, event: ChannelEvent) {
        match event.topic() {
            NEW_BLOCKS_TOPIC | NEW_INTEROP_MESSAGES_TOPIC => {
                conn.allow_join(&event, &()).await;
            }
            _ => {}
        }
    }
}
