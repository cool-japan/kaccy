//! WebSocket routes for real-time updates
//!
//! Provides:
//! - Real-time price updates for tokens
//! - Order status notifications
//! - Chat for token holder communities

use axum::{
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::state::AppState;

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum WsMessage {
    /// Price update for a token
    PriceUpdate(PriceUpdate),
    /// Order status change
    OrderUpdate(OrderUpdate),
    /// Trade executed
    TradeExecuted(TradeNotification),
    /// Chat message
    ChatMessage(ChatMessage),
    /// Subscription confirmation
    Subscribed {
        channel: String,
    },
    /// Error message
    Error {
        message: String,
    },
    /// Ping/pong for keepalive
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub token_id: String,
    pub symbol: String,
    pub price_btc: f64,
    pub change_24h: f64,
    pub volume_24h: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    pub order_id: String,
    pub status: String,
    pub message: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeNotification {
    pub trade_id: String,
    pub token_id: String,
    pub symbol: String,
    pub amount: f64,
    pub price_btc: f64,
    pub trade_type: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub token_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: i64,
}

/// Client subscribe/unsubscribe messages
#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe { channel: String },
    Unsubscribe { channel: String },
    Chat { content: String },
    Ping,
}

/// Incoming chat message from client
#[derive(Debug, Deserialize)]
pub struct IncomingChatMessage {
    pub content: String,
}

/// Query params for WebSocket connection
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    /// JWT token for authentication (optional for public streams)
    pub token: Option<String>,
}

/// WebSocket handler for token price updates
pub async fn token_prices_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(_query): Query<WsQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_price_stream(socket, state))
}

/// Handle price stream WebSocket connection
async fn handle_price_stream(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to price updates
    let mut price_rx = state.subscribe_prices();

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: "prices".to_string(),
    };
    if sender
        .send(Message::Text(
            serde_json::to_string(&confirm).unwrap().into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    // Spawn task to forward price updates to client
    let send_task = tokio::spawn(async move {
        while let Ok(update) = price_rx.recv().await {
            let msg = WsMessage::PriceUpdate(update);
            if sender
                .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages (ping/pong, unsubscribe)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Ping(data) => {
                    // Pong is handled automatically by axum
                    tracing::debug!("Received ping: {:?}", data);
                }
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        match client_msg {
                            ClientMessage::Ping => {
                                tracing::debug!("Client ping");
                            }
                            ClientMessage::Unsubscribe { .. } => {
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// WebSocket handler for specific token updates
pub async fn token_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
    Query(_query): Query<WsQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_token_stream(socket, state, token_id))
}

/// Handle single token WebSocket connection
async fn handle_token_stream(socket: WebSocket, state: AppState, token_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to token-specific updates
    let mut trade_rx = state.subscribe_trades(token_id);

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: format!("token:{}", token_id),
    };
    if sender
        .send(Message::Text(
            serde_json::to_string(&confirm).unwrap().into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    // Forward trade notifications
    let send_task = tokio::spawn(async move {
        while let Ok(trade) = trade_rx.recv().await {
            let msg = WsMessage::TradeExecuted(trade);
            if sender
                .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// WebSocket handler for user order updates (requires auth)
pub async fn user_orders_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
) -> impl IntoResponse {
    // Verify JWT token
    let user_id = match query.token {
        Some(token) => match state.verify_token(&token) {
            Some(uid) => uid,
            None => {
                return ws.on_upgrade(|socket| async {
                    let (mut sender, _) = socket.split();
                    let msg = WsMessage::Error {
                        message: "Invalid or expired token".to_string(),
                    };
                    let _ = sender
                        .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                        .await;
                });
            }
        },
        None => {
            return ws.on_upgrade(|socket| async {
                let (mut sender, _) = socket.split();
                let msg = WsMessage::Error {
                    message: "Authentication required".to_string(),
                };
                let _ = sender
                    .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                    .await;
            });
        }
    };

    ws.on_upgrade(move |socket| handle_user_orders_stream(socket, state, user_id))
}

/// Handle user orders WebSocket connection
async fn handle_user_orders_stream(socket: WebSocket, state: AppState, user_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to user's order updates
    let mut order_rx = state.subscribe_user_orders(user_id);

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: format!("orders:{}", user_id),
    };
    if sender
        .send(Message::Text(
            serde_json::to_string(&confirm).unwrap().into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    // Forward order updates
    let send_task = tokio::spawn(async move {
        while let Ok(update) = order_rx.recv().await {
            let msg = WsMessage::OrderUpdate(update);
            if sender
                .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// WebSocket handler for token chat (requires auth and token holdings)
pub async fn token_chat_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Path(token_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
) -> impl IntoResponse {
    // Verify JWT token
    let user_id = match query.token {
        Some(token) => match state.verify_token(&token) {
            Some(uid) => uid,
            None => {
                return ws.on_upgrade(|socket| async {
                    let (mut sender, _) = socket.split();
                    let msg = WsMessage::Error {
                        message: "Invalid or expired token".to_string(),
                    };
                    let _ = sender
                        .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                        .await;
                });
            }
        },
        None => {
            return ws.on_upgrade(|socket| async {
                let (mut sender, _) = socket.split();
                let msg = WsMessage::Error {
                    message: "Authentication required for chat".to_string(),
                };
                let _ = sender
                    .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                    .await;
            });
        }
    };

    ws.on_upgrade(move |socket| handle_chat_stream(socket, state, token_id, user_id))
}

/// Chat user info for display
#[derive(Clone)]
struct ChatUser {
    user_id: Uuid,
    display_name: String,
}

/// Handle chat WebSocket connection
async fn handle_chat_stream(socket: WebSocket, state: AppState, token_id: Uuid, user_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Get user info for chat messages
    let user_info = match get_user_display_name(&state.pool, user_id).await {
        Ok(name) => ChatUser {
            user_id,
            display_name: name,
        },
        Err(_) => ChatUser {
            user_id,
            display_name: "Anonymous".to_string(),
        },
    };

    // Subscribe to chat messages for this token
    let mut chat_rx = state.ws_broadcaster.subscribe_chat(token_id);

    // Send subscription confirmation
    let confirm = WsMessage::Subscribed {
        channel: format!("chat:{}", token_id),
    };
    if sender
        .send(Message::Text(
            serde_json::to_string(&confirm).unwrap().into(),
        ))
        .await
        .is_err()
    {
        return;
    }

    let broadcaster = state.ws_broadcaster.clone();
    let user_info_send = user_info.clone();

    // Forward chat messages to client
    let send_task = tokio::spawn(async move {
        while let Ok(message) = chat_rx.recv().await {
            let msg = WsMessage::ChatMessage(message);
            if sender
                .send(Message::Text(serde_json::to_string(&msg).unwrap().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages (chat messages from this user)
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Close(_) => break,
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        match client_msg {
                            ClientMessage::Chat { content } => {
                                // Validate message content
                                let content = content.trim();
                                if content.is_empty() || content.len() > 1000 {
                                    continue;
                                }

                                // Create and broadcast chat message
                                let chat_msg = ChatMessage {
                                    token_id: token_id.to_string(),
                                    sender_id: user_info_send.user_id.to_string(),
                                    sender_name: user_info_send.display_name.clone(),
                                    content: content.to_string(),
                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                };

                                broadcaster.broadcast_chat(token_id, chat_msg);
                            }
                            ClientMessage::Unsubscribe { .. } => {
                                break;
                            }
                            ClientMessage::Ping => {
                                tracing::debug!("Client ping");
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

/// Get user display name from database
async fn get_user_display_name(pool: &sqlx::PgPool, user_id: Uuid) -> Result<String, sqlx::Error> {
    let result: Option<(Option<String>, String)> =
        sqlx::query_as("SELECT display_name, username FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    Ok(result
        .map(|(display_name, username)| display_name.unwrap_or(username))
        .unwrap_or_else(|| "Anonymous".to_string()))
}

/// Broadcast channels for WebSocket updates
#[derive(Clone)]
pub struct WsBroadcaster {
    price_tx: Arc<broadcast::Sender<PriceUpdate>>,
    trade_channels: Arc<dashmap::DashMap<Uuid, broadcast::Sender<TradeNotification>>>,
    order_channels: Arc<dashmap::DashMap<Uuid, broadcast::Sender<OrderUpdate>>>,
    chat_channels: Arc<dashmap::DashMap<Uuid, broadcast::Sender<ChatMessage>>>,
}

impl Default for WsBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl WsBroadcaster {
    pub fn new() -> Self {
        let (price_tx, _) = broadcast::channel(1000);
        Self {
            price_tx: Arc::new(price_tx),
            trade_channels: Arc::new(dashmap::DashMap::new()),
            order_channels: Arc::new(dashmap::DashMap::new()),
            chat_channels: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Broadcast a price update
    pub fn broadcast_price(&self, update: PriceUpdate) {
        let _ = self.price_tx.send(update);
    }

    /// Broadcast a trade notification for a token
    pub fn broadcast_trade(&self, token_id: Uuid, trade: TradeNotification) {
        if let Some(tx) = self.trade_channels.get(&token_id) {
            let _ = tx.send(trade);
        }
    }

    /// Broadcast an order update for a user
    pub fn broadcast_order(&self, user_id: Uuid, update: OrderUpdate) {
        if let Some(tx) = self.order_channels.get(&user_id) {
            let _ = tx.send(update);
        }
    }

    /// Broadcast a chat message for a token
    pub fn broadcast_chat(&self, token_id: Uuid, message: ChatMessage) {
        if let Some(tx) = self.chat_channels.get(&token_id) {
            let _ = tx.send(message);
        }
    }

    /// Subscribe to price updates
    pub fn subscribe_prices(&self) -> broadcast::Receiver<PriceUpdate> {
        self.price_tx.subscribe()
    }

    /// Subscribe to trade updates for a token
    pub fn subscribe_trades(&self, token_id: Uuid) -> broadcast::Receiver<TradeNotification> {
        self.trade_channels
            .entry(token_id)
            .or_insert_with(|| broadcast::channel(100).0)
            .subscribe()
    }

    /// Subscribe to order updates for a user
    pub fn subscribe_user_orders(&self, user_id: Uuid) -> broadcast::Receiver<OrderUpdate> {
        self.order_channels
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(100).0)
            .subscribe()
    }

    /// Subscribe to chat messages for a token
    pub fn subscribe_chat(&self, token_id: Uuid) -> broadcast::Receiver<ChatMessage> {
        self.chat_channels
            .entry(token_id)
            .or_insert_with(|| broadcast::channel(500).0)
            .subscribe()
    }
}
