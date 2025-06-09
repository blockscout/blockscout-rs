use actix_web::{App, HttpServer};
use phoenix_channel::{
    actix_handler::configure_channel_websocket_route, channel::ChannelCentral, conn::ChannelConn,
    event::ChannelEvent, handler::ChannelHandler,
};
use std::sync::Arc;

pub struct Channel;

#[async_trait::async_trait]
impl ChannelHandler for Channel {
    async fn join_channel(&self, conn: &ChannelConn, event: ChannelEvent) {
        if event.topic() == "echo" {
            conn.client().allow_join(&event, &()).await;
            conn.client().broadcast(("echo", "joined"));
        }
    }

    async fn incoming_message(&self, conn: &ChannelConn, event: ChannelEvent) {
        if event.topic() == "echo" {
            conn.client().broadcast(event);
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let channel = Arc::new(ChannelCentral::new(Channel));

    HttpServer::new(move || {
        App::new().configure(|cfg| {
            configure_channel_websocket_route(cfg, channel.clone());
        })
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
