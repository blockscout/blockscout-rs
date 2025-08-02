use actix::{Actor, AsyncContext, Handler, Message, Recipient, StreamHandler};
use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use tonic::async_trait;
use tracing::instrument;
use zetachain_cctx_logic::events::EventBroadcaster;
use std::collections::HashMap;
use uuid::Uuid;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{CrossChainTx, CctxListItem as CctxListItemProto};
use actix::ActorContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WebSocketEvent {
    CctxStatusUpdate {
        cctx_index: String,
        cctx_data: CrossChainTx,
    },
    NewCctxImported {
        cctxs: Vec<CctxListItemProto>,
    },
}

#[derive(Debug, Clone)]
pub enum SubscriptionType {
    CctxUpdates(String), // CCTX index to subscribe to
    NewCctxs,           // Subscribe to new CCTX imports
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Subscribe {
    pub client_id: Uuid,
    pub subscription_type: SubscriptionType,
    pub recipient: Recipient<WebSocketMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Unsubscribe {
    pub client_id: Uuid,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct BroadcastEvent {
    pub event: WebSocketEvent,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct WebSocketMessage {
    pub content: String,
}

pub struct WebSocketManager {
    // Map of client_id -> (subscription_type, recipient)
    clients: HashMap<Uuid, (SubscriptionType, Recipient<WebSocketMessage>)>,
    // Index of CCTX subscriptions: cctx_index -> Vec<client_id>
    cctx_subscriptions: HashMap<String, Vec<Uuid>>,
    // List of clients subscribed to new CCTX imports
    new_cctx_subscribers: Vec<Uuid>,
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self {
            clients: HashMap::new(),
            cctx_subscriptions: HashMap::new(),
            new_cctx_subscribers: Vec::new(),
        }
    }
}

impl Actor for WebSocketManager {
    type Context = actix::Context<Self>;
}

impl Handler<Subscribe> for WebSocketManager {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, _ctx: &mut Self::Context) -> Self::Result {
        tracing::info!("Client {} subscribing to {:?}", msg.client_id, msg.subscription_type);
        
        match &msg.subscription_type {
            SubscriptionType::CctxUpdates(cctx_index) => {
                self.cctx_subscriptions
                    .entry(cctx_index.clone())
                    .or_insert_with(Vec::new)
                    .push(msg.client_id);
            }
            SubscriptionType::NewCctxs => {
                self.new_cctx_subscribers.push(msg.client_id);
                tracing::info!("New CCTX subscriber: {:?} added to new_cctx_subscribers", msg.client_id);
                tracing::info!("new_cctx_subscribers: {:?}", self.new_cctx_subscribers);
            }
        }
        
        self.clients.insert(msg.client_id, (msg.subscription_type, msg.recipient));
    }
}

impl Handler<Unsubscribe> for WebSocketManager {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, _ctx: &mut Self::Context) -> Self::Result {
        tracing::info!("Client {} unsubscribing", msg.client_id);
        
        if let Some((subscription_type, _)) = self.clients.remove(&msg.client_id) {
            match subscription_type {
                SubscriptionType::CctxUpdates(cctx_index) => {
                    if let Some(subscribers) = self.cctx_subscriptions.get_mut(&cctx_index) {
                        subscribers.retain(|&id| id != msg.client_id);
                        if subscribers.is_empty() {
                            self.cctx_subscriptions.remove(&cctx_index);
                        }
                    }
                }
                SubscriptionType::NewCctxs => {
                    self.new_cctx_subscribers.retain(|&id| id != msg.client_id);
                }
            }
        }
    }
}

impl Handler<BroadcastEvent> for WebSocketManager {
    type Result = ();

    fn handle(&mut self, msg: BroadcastEvent, _ctx: &mut Self::Context) -> Self::Result {
        tracing::info!("Broadcasting event: {:?}", msg.event);
        let event_json = serde_json::to_string(&msg.event).unwrap_or_else(|e| {
            tracing::error!("Failed to serialize event: {}", e);
            String::from("{\"error\":\"serialization_failed\"}")
        });

        match &msg.event {
            WebSocketEvent::CctxStatusUpdate { cctx_index, .. } => {
                if let Some(subscribers) = self.cctx_subscriptions.get(cctx_index) {
                    for &client_id in subscribers {
                        if let Some((_, recipient)) = self.clients.get(&client_id) {
                            let _ = recipient.try_send(WebSocketMessage {
                                content: event_json.clone(),
                            });
                        }
                    }
                    tracing::debug!("Broadcasted CCTX update for {} to {} subscribers", cctx_index, subscribers.len());
                }
            }
            WebSocketEvent::NewCctxImported { cctxs } => {
                tracing::info!("Broadcasting new CCTX imports: {:?}", cctxs.iter().map(|cctx| cctx.index.clone()).collect::<Vec<String>>());
                for &client_id in &self.new_cctx_subscribers {
                    tracing::info!("Searching for client: {:?}", client_id);
                    if let Some((_, recipient)) = self.clients.get(&client_id) {
                        tracing::info!("Found client: {:?}", client_id);
                        let _ = recipient.try_send(WebSocketMessage {
                            content: event_json.clone(),
                        });
                    } else {
                        tracing::info!("Client not found: {:?}", client_id);
                    }
                }
                tracing::debug!("Broadcasted new CCTX imports {:?} to {} subscribers", cctxs.iter().map(|cctx| cctx.index.clone()).collect::<Vec<String>>(), self.new_cctx_subscribers.len());
            }
        }
    }
}

pub struct WebSocketClient {
    pub client_id: Uuid,
    pub manager: actix::Addr<WebSocketManager>,
    pub subscription_type: Option<SubscriptionType>,
}

impl Actor for WebSocketClient {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        tracing::info!("WebSocket client {} connected", self.client_id);
        
        if let Some(subscription_type) = self.subscription_type.clone() {
            self.manager.do_send(Subscribe {
                client_id: self.client_id,
                subscription_type,
                recipient: ctx.address().recipient(),
            });
        }
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        tracing::info!("WebSocket client {} disconnected", self.client_id);
        self.manager.do_send(Unsubscribe {
            client_id: self.client_id,
        });
    }
}

impl Handler<WebSocketMessage> for WebSocketClient {
    type Result = ();

    fn handle(&mut self, msg: WebSocketMessage, ctx: &mut Self::Context) -> Self::Result {
        ctx.text(msg.content);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketClient {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                // Handle client messages if needed (e.g., subscription changes)
                tracing::debug!("Received message from client {}: {}", self.client_id, text);
            }
            Ok(ws::Message::Binary(_)) => {
                tracing::debug!("Received binary message from client {}", self.client_id);
            }
            Ok(ws::Message::Close(reason)) => {
                tracing::info!("Client {} closing connection: {:?}", self.client_id, reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => (),
        }
    }
}

#[derive(Clone)]
pub struct WebSocketEventBroadcaster {
    manager: actix::Addr<WebSocketManager>,
}

impl WebSocketEventBroadcaster {
    pub fn new(manager: actix::Addr<WebSocketManager>) -> Self {
        Self { manager }
    }

    #[allow(unused)]
    #[instrument(level="debug",skip_all)]
    pub fn broadcast_cctx_update(&self, cctx_index: String, cctx_data: CrossChainTx) {
        self.manager.do_send(BroadcastEvent {
            event: WebSocketEvent::CctxStatusUpdate {
                cctx_index,
                cctx_data,
            },
        });
    }

    #[allow(unused)]
    #[instrument(level="debug",skip_all)]
    pub fn broadcast_new_cctxs(&self, cctxs: Vec<CctxListItemProto>) {
        self.manager.do_send(BroadcastEvent {
            event: WebSocketEvent::NewCctxImported { cctxs },
        });
    }
} 

#[async_trait]
impl EventBroadcaster for WebSocketEventBroadcaster {
    async fn broadcast_cctx_update(&self, cctx_index: String, cctx_data: CrossChainTx) {

        
        self.manager.do_send(BroadcastEvent {
            event: WebSocketEvent::CctxStatusUpdate {
                cctx_index,
                cctx_data,    
                },
            });
        
    }

    async fn broadcast_new_cctxs(&self, cctxs: Vec<CctxListItemProto>) {
        self.manager.do_send(BroadcastEvent {
            event: WebSocketEvent::NewCctxImported { cctxs },
        });
    }
}