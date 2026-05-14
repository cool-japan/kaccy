//! Real-time streaming infrastructure for WebSocket connections
//!
//! This module provides streaming capabilities for:
//! - Order book updates (snapshots and deltas)
//! - Market data (trades, tickers, candlesticks)
//! - User-specific updates (positions, orders, balances)
//!
//! # Features
//!
//! - Delta compression for efficient bandwidth usage
//! - Snapshot + delta synchronization
//! - Connection recovery and replay
//! - Multi-channel subscriptions
//! - Back-pressure handling

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, broadcast};
use uuid::Uuid;

use crate::trading::order_book::OrderSide;

/// Maximum number of messages to buffer per subscription
const MAX_BUFFER_SIZE: usize = 1000;

/// Type alias for stream sender with sequence number
type StreamSender = (broadcast::Sender<StreamMessage>, u64);

/// Type alias for candlestick stream key
type CandlestickKey = (Uuid, CandlestickInterval);

/// Order book snapshot for initial synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    /// Token this snapshot belongs to
    pub token_id: Uuid,
    /// Snapshot timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Monotonically increasing sequence number
    pub sequence_number: u64,
    /// Bid price levels ordered best-to-worst
    pub bids: Vec<PriceLevel>,
    /// Ask price levels ordered best-to-worst
    pub asks: Vec<PriceLevel>,
}

/// Price level in the order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price at this level
    pub price: Decimal,
    /// Total quantity resting at this price
    pub quantity: Decimal,
    /// Number of distinct orders at this price
    pub order_count: usize,
}

/// Order book delta update (incremental changes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookDelta {
    /// Token this delta belongs to
    pub token_id: Uuid,
    /// Delta timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Monotonically increasing sequence number
    pub sequence_number: u64,
    /// List of individual price-level changes
    pub changes: Vec<OrderBookChange>,
}

/// Individual order book change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderBookChange {
    /// Add or update a price level
    Upsert {
        /// Side of the order book (bid or ask)
        side: OrderSide,
        /// Price level being upserted
        price: Decimal,
        /// New total quantity at this price level
        quantity: Decimal,
        /// New number of orders at this price level
        order_count: usize,
    },
    /// Remove a price level (quantity became zero)
    Remove {
        /// Side of the order book (bid or ask)
        side: OrderSide,
        /// Price level being removed
        price: Decimal,
    },
}

/// Trade event for market data streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    /// Unique identifier of the trade
    pub trade_id: Uuid,
    /// Token that was traded
    pub token_id: Uuid,
    /// Trade timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Execution price
    pub price: Decimal,
    /// Traded quantity
    pub quantity: Decimal,
    /// Side of the taker order
    pub side: OrderSide,
    /// Whether the order was a maker (resting) order
    pub is_maker: bool,
}

/// Ticker update (24h statistics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerUpdate {
    /// Token this ticker belongs to
    pub token_id: Uuid,
    /// Ticker timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Last traded price
    pub last_price: Decimal,
    /// 24-hour high price
    pub high_24h: Decimal,
    /// 24-hour low price
    pub low_24h: Decimal,
    /// 24-hour trading volume
    pub volume_24h: Decimal,
    /// Absolute price change over 24 hours
    pub price_change_24h: Decimal,
    /// Percentage price change over 24 hours
    pub price_change_percent_24h: Decimal,
}

/// OHLCV candlestick update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandlestickUpdate {
    /// Token this candlestick belongs to
    pub token_id: Uuid,
    /// Candlestick time interval
    pub interval: CandlestickInterval,
    /// Candle open timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Opening price
    pub open: Decimal,
    /// Highest price within the interval
    pub high: Decimal,
    /// Lowest price within the interval
    pub low: Decimal,
    /// Closing price (or current price if not yet closed)
    pub close: Decimal,
    /// Trading volume within the interval
    pub volume: Decimal,
    /// Whether this candle is finalised (interval has closed)
    pub is_closed: bool,
}

/// Candlestick interval
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CandlestickInterval {
    /// 1-minute candlestick
    #[serde(rename = "1m")]
    OneMinute,
    /// 5-minute candlestick
    #[serde(rename = "5m")]
    FiveMinutes,
    /// 15-minute candlestick
    #[serde(rename = "15m")]
    FifteenMinutes,
    /// 1-hour candlestick
    #[serde(rename = "1h")]
    OneHour,
    /// 4-hour candlestick
    #[serde(rename = "4h")]
    FourHours,
    /// 1-day candlestick
    #[serde(rename = "1d")]
    OneDay,
}

impl CandlestickInterval {
    /// Return the wall-clock duration corresponding to this interval
    pub fn duration(&self) -> Duration {
        match self {
            Self::OneMinute => Duration::from_secs(60),
            Self::FiveMinutes => Duration::from_secs(300),
            Self::FifteenMinutes => Duration::from_secs(900),
            Self::OneHour => Duration::from_secs(3600),
            Self::FourHours => Duration::from_secs(14400),
            Self::OneDay => Duration::from_secs(86400),
        }
    }
}

/// User position update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    /// User who holds this position
    pub user_id: Uuid,
    /// Token of the position
    pub token_id: Uuid,
    /// Update timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Current position size
    pub quantity: Decimal,
    /// Volume-weighted average entry price
    pub average_price: Decimal,
    /// Unrealised profit/loss at current mark price
    pub unrealized_pnl: Decimal,
    /// Realised profit/loss from closed trades
    pub realized_pnl: Decimal,
}

/// Order status update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdate {
    /// Unique identifier of the order
    pub order_id: Uuid,
    /// User who placed the order
    pub user_id: Uuid,
    /// Token being traded
    pub token_id: Uuid,
    /// Update timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Current order status
    pub status: OrderStatus,
    /// Quantity already filled
    pub filled_quantity: Decimal,
    /// Quantity still open
    pub remaining_quantity: Decimal,
}

/// Order lifecycle status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OrderStatus {
    /// Order submitted, not yet matched
    Pending,
    /// Order partially filled
    PartiallyFilled,
    /// Order fully filled
    Filled,
    /// Order cancelled by user
    Cancelled,
    /// Order expired without being filled
    Expired,
    /// Order rejected by the engine
    Rejected,
}

/// Balance update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceUpdate {
    /// User whose balance changed
    pub user_id: Uuid,
    /// Update timestamp in milliseconds since UNIX epoch
    pub timestamp: u64,
    /// Currency or asset symbol
    pub currency: String,
    /// Available (spendable) balance
    pub available: Decimal,
    /// Reserved balance locked in open orders
    pub reserved: Decimal,
    /// Total balance (available + reserved)
    pub total: Decimal,
}

/// Streaming message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamMessage {
    /// Order book snapshot
    OrderBookSnapshot(OrderBookSnapshot),
    /// Order book delta update
    OrderBookDelta(OrderBookDelta),
    /// Trade event
    Trade(TradeEvent),
    /// Ticker update
    Ticker(TickerUpdate),
    /// Candlestick update
    Candlestick(CandlestickUpdate),
    /// Position update
    Position(PositionUpdate),
    /// Order update
    Order(OrderUpdate),
    /// Balance update
    Balance(BalanceUpdate),
    /// Heartbeat (keep-alive)
    Heartbeat {
        /// Heartbeat timestamp in milliseconds since UNIX epoch
        timestamp: u64,
    },
    /// Error message
    Error {
        /// Machine-readable error code
        code: String,
        /// Human-readable error message
        message: String,
    },
}

/// Subscription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    /// Channel to subscribe to
    pub channel: StreamChannel,
    /// Optional filters to apply to the stream
    pub filters: HashMap<String, String>,
}

/// Stream channel types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "channel", rename_all = "snake_case")]
pub enum StreamChannel {
    /// Order book updates for a specific token
    OrderBook {
        /// Token to stream order book updates for
        token_id: Uuid,
    },
    /// Trade feed for a specific token
    Trades {
        /// Token to stream trades for
        token_id: Uuid,
    },
    /// Ticker updates for a specific token
    Ticker {
        /// Token to stream ticker data for
        token_id: Uuid,
    },
    /// Candlestick updates for a specific token and interval
    Candlesticks {
        /// Token to stream candlesticks for
        token_id: Uuid,
        /// Candlestick time interval
        interval: CandlestickInterval,
    },
    /// User-specific position updates
    Positions {
        /// User whose positions to stream
        user_id: Uuid,
    },
    /// User-specific order updates
    Orders {
        /// User whose orders to stream
        user_id: Uuid,
    },
    /// User-specific balance updates
    Balances {
        /// User whose balances to stream
        user_id: Uuid,
    },
}

/// WebSocket connection state
#[derive(Debug)]
pub struct ConnectionState {
    /// Unique identifier for this connection
    pub connection_id: Uuid,
    /// Authenticated user, if any
    pub user_id: Option<Uuid>,
    /// Active subscriptions mapping channel to last received sequence number
    pub subscriptions: HashMap<StreamChannel, u64>,
    /// When the connection was established
    pub connected_at: SystemTime,
    /// Timestamp of the most recent message activity
    pub last_activity: SystemTime,
}

impl ConnectionState {
    /// Create a new connection state for the given user
    pub fn new(user_id: Option<Uuid>) -> Self {
        let now = SystemTime::now();
        Self {
            connection_id: Uuid::new_v4(),
            user_id,
            subscriptions: HashMap::new(),
            connected_at: now,
            last_activity: now,
        }
    }

    /// Subscribe this connection to a channel, tracking the given sequence number
    pub fn subscribe(&mut self, channel: StreamChannel, sequence_number: u64) {
        self.subscriptions.insert(channel, sequence_number);
        self.last_activity = SystemTime::now();
    }

    /// Unsubscribe this connection from a channel
    pub fn unsubscribe(&mut self, channel: &StreamChannel) {
        self.subscriptions.remove(channel);
        self.last_activity = SystemTime::now();
    }

    /// Record that the connection was recently active
    pub fn update_activity(&mut self) {
        self.last_activity = SystemTime::now();
    }

    /// Return whether this connection is subscribed to the given channel
    pub fn is_subscribed(&self, channel: &StreamChannel) -> bool {
        self.subscriptions.contains_key(channel)
    }
}

/// Order book stream manager
pub struct OrderBookStreamManager {
    /// Token ID -> (sender, sequence number)
    streams: Arc<RwLock<HashMap<Uuid, StreamSender>>>,
    /// Snapshot buffer for recovery
    snapshots: Arc<RwLock<HashMap<Uuid, OrderBookSnapshot>>>,
    /// Delta buffer for replay
    delta_buffer: Arc<RwLock<HashMap<Uuid, VecDeque<OrderBookDelta>>>>,
}

impl OrderBookStreamManager {
    /// Create a new order book stream manager
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            delta_buffer: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a stream for a token
    pub async fn get_or_create_stream(&self, token_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.streams.write().await;
        let (sender, _) = streams
            .entry(token_id)
            .or_insert_with(|| (broadcast::channel(MAX_BUFFER_SIZE).0, 0));
        sender.subscribe()
    }

    /// Publish an order book snapshot
    pub async fn publish_snapshot(&self, snapshot: OrderBookSnapshot) {
        let token_id = snapshot.token_id;
        let sequence_number = snapshot.sequence_number;

        // Store snapshot for recovery
        self.snapshots
            .write()
            .await
            .insert(token_id, snapshot.clone());

        // Send to subscribers
        if let Some((sender, seq)) = self.streams.write().await.get_mut(&token_id) {
            *seq = sequence_number;
            let _ = sender.send(StreamMessage::OrderBookSnapshot(snapshot));
        }
    }

    /// Publish an order book delta
    pub async fn publish_delta(&self, delta: OrderBookDelta) {
        let token_id = delta.token_id;
        let sequence_number = delta.sequence_number;

        // Buffer delta for replay
        let mut buffer = self.delta_buffer.write().await;
        let deltas = buffer.entry(token_id).or_insert_with(VecDeque::new);
        deltas.push_back(delta.clone());

        // Keep only recent deltas (last 100)
        while deltas.len() > 100 {
            deltas.pop_front();
        }

        // Send to subscribers
        if let Some((sender, seq)) = self.streams.write().await.get_mut(&token_id) {
            *seq = sequence_number;
            let _ = sender.send(StreamMessage::OrderBookDelta(delta));
        }
    }

    /// Get the latest snapshot for a token
    pub async fn get_snapshot(&self, token_id: Uuid) -> Option<OrderBookSnapshot> {
        self.snapshots.read().await.get(&token_id).cloned()
    }

    /// Get deltas since a sequence number
    pub async fn get_deltas_since(
        &self,
        token_id: Uuid,
        sequence_number: u64,
    ) -> Vec<OrderBookDelta> {
        if let Some(deltas) = self.delta_buffer.read().await.get(&token_id) {
            deltas
                .iter()
                .filter(|d| d.sequence_number > sequence_number)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for OrderBookStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Market data stream manager
pub struct MarketDataStreamManager {
    /// Trade streams per token
    trade_streams: Arc<RwLock<HashMap<Uuid, broadcast::Sender<StreamMessage>>>>,
    /// Ticker streams per token
    ticker_streams: Arc<RwLock<HashMap<Uuid, broadcast::Sender<StreamMessage>>>>,
    /// Candlestick streams per (token, interval)
    candlestick_streams: Arc<RwLock<HashMap<CandlestickKey, broadcast::Sender<StreamMessage>>>>,
}

impl MarketDataStreamManager {
    /// Create a new market data stream manager
    pub fn new() -> Self {
        Self {
            trade_streams: Arc::new(RwLock::new(HashMap::new())),
            ticker_streams: Arc::new(RwLock::new(HashMap::new())),
            candlestick_streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to trades for a token
    pub async fn subscribe_trades(&self, token_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.trade_streams.write().await;
        let sender = streams
            .entry(token_id)
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish a trade event
    pub async fn publish_trade(&self, trade: TradeEvent) {
        let token_id = trade.token_id;
        if let Some(sender) = self.trade_streams.read().await.get(&token_id) {
            let _ = sender.send(StreamMessage::Trade(trade));
        }
    }

    /// Subscribe to ticker updates for a token
    pub async fn subscribe_ticker(&self, token_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.ticker_streams.write().await;
        let sender = streams
            .entry(token_id)
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish a ticker update
    pub async fn publish_ticker(&self, ticker: TickerUpdate) {
        let token_id = ticker.token_id;
        if let Some(sender) = self.ticker_streams.read().await.get(&token_id) {
            let _ = sender.send(StreamMessage::Ticker(ticker));
        }
    }

    /// Subscribe to candlestick updates
    pub async fn subscribe_candlesticks(
        &self,
        token_id: Uuid,
        interval: CandlestickInterval,
    ) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.candlestick_streams.write().await;
        let sender = streams
            .entry((token_id, interval))
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish a candlestick update
    pub async fn publish_candlestick(&self, candle: CandlestickUpdate) {
        let key = (candle.token_id, candle.interval);
        if let Some(sender) = self.candlestick_streams.read().await.get(&key) {
            let _ = sender.send(StreamMessage::Candlestick(candle));
        }
    }
}

impl Default for MarketDataStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// User stream manager for user-specific updates
pub struct UserStreamManager {
    /// Position streams per user
    position_streams: Arc<RwLock<HashMap<Uuid, broadcast::Sender<StreamMessage>>>>,
    /// Order streams per user
    order_streams: Arc<RwLock<HashMap<Uuid, broadcast::Sender<StreamMessage>>>>,
    /// Balance streams per user
    balance_streams: Arc<RwLock<HashMap<Uuid, broadcast::Sender<StreamMessage>>>>,
}

impl UserStreamManager {
    /// Create a new user stream manager
    pub fn new() -> Self {
        Self {
            position_streams: Arc::new(RwLock::new(HashMap::new())),
            order_streams: Arc::new(RwLock::new(HashMap::new())),
            balance_streams: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Subscribe to position updates for a user
    pub async fn subscribe_positions(&self, user_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.position_streams.write().await;
        let sender = streams
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish a position update
    pub async fn publish_position(&self, position: PositionUpdate) {
        let user_id = position.user_id;
        if let Some(sender) = self.position_streams.read().await.get(&user_id) {
            let _ = sender.send(StreamMessage::Position(position));
        }
    }

    /// Subscribe to order updates for a user
    pub async fn subscribe_orders(&self, user_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.order_streams.write().await;
        let sender = streams
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish an order update
    pub async fn publish_order(&self, order: OrderUpdate) {
        let user_id = order.user_id;
        if let Some(sender) = self.order_streams.read().await.get(&user_id) {
            let _ = sender.send(StreamMessage::Order(order));
        }
    }

    /// Subscribe to balance updates for a user
    pub async fn subscribe_balances(&self, user_id: Uuid) -> broadcast::Receiver<StreamMessage> {
        let mut streams = self.balance_streams.write().await;
        let sender = streams
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(MAX_BUFFER_SIZE).0);
        sender.subscribe()
    }

    /// Publish a balance update
    pub async fn publish_balance(&self, balance: BalanceUpdate) {
        let user_id = balance.user_id;
        if let Some(sender) = self.balance_streams.read().await.get(&user_id) {
            let _ = sender.send(StreamMessage::Balance(balance));
        }
    }
}

impl Default for UserStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to get current timestamp in milliseconds
pub fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("SystemTime should be after UNIX_EPOCH")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn test_order_book_stream_manager() {
        let manager = OrderBookStreamManager::new();
        let token_id = Uuid::new_v4();

        // Create a snapshot
        let snapshot = OrderBookSnapshot {
            token_id,
            timestamp: current_timestamp_ms(),
            sequence_number: 1,
            bids: vec![PriceLevel {
                price: dec!(100),
                quantity: dec!(10),
                order_count: 1,
            }],
            asks: vec![PriceLevel {
                price: dec!(101),
                quantity: dec!(5),
                order_count: 1,
            }],
        };

        // Publish snapshot
        manager.publish_snapshot(snapshot.clone()).await;

        // Get snapshot
        let retrieved = manager.get_snapshot(token_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().sequence_number, 1);
    }

    #[tokio::test]
    async fn test_order_book_delta_buffer() {
        let manager = OrderBookStreamManager::new();
        let token_id = Uuid::new_v4();

        // Publish several deltas
        for i in 1..=5 {
            let delta = OrderBookDelta {
                token_id,
                timestamp: current_timestamp_ms(),
                sequence_number: i,
                changes: vec![],
            };
            manager.publish_delta(delta).await;
        }

        // Get deltas since sequence 2
        let deltas = manager.get_deltas_since(token_id, 2).await;
        assert_eq!(deltas.len(), 3); // sequences 3, 4, 5
        assert_eq!(deltas[0].sequence_number, 3);
    }

    #[tokio::test]
    async fn test_market_data_stream() {
        let manager = MarketDataStreamManager::new();
        let token_id = Uuid::new_v4();

        // Subscribe to trades
        let mut receiver = manager.subscribe_trades(token_id).await;

        // Publish a trade
        let trade = TradeEvent {
            trade_id: Uuid::new_v4(),
            token_id,
            timestamp: current_timestamp_ms(),
            price: dec!(100),
            quantity: dec!(10),
            side: OrderSide::Buy,
            is_maker: false,
        };

        manager.publish_trade(trade.clone()).await;

        // Receive the trade
        let msg = receiver.recv().await.unwrap();
        if let StreamMessage::Trade(received_trade) = msg {
            assert_eq!(received_trade.token_id, token_id);
            assert_eq!(received_trade.price, dec!(100));
        } else {
            panic!("Expected Trade message");
        }
    }

    #[tokio::test]
    async fn test_user_stream_manager() {
        let manager = UserStreamManager::new();
        let user_id = Uuid::new_v4();

        // Subscribe to orders
        let mut receiver = manager.subscribe_orders(user_id).await;

        // Publish an order update
        let order = OrderUpdate {
            order_id: Uuid::new_v4(),
            user_id,
            token_id: Uuid::new_v4(),
            timestamp: current_timestamp_ms(),
            status: OrderStatus::Filled,
            filled_quantity: dec!(10),
            remaining_quantity: dec!(0),
        };

        manager.publish_order(order.clone()).await;

        // Receive the order update
        let msg = receiver.recv().await.unwrap();
        if let StreamMessage::Order(received_order) = msg {
            assert_eq!(received_order.user_id, user_id);
            assert_eq!(received_order.status, OrderStatus::Filled);
        } else {
            panic!("Expected Order message");
        }
    }

    #[test]
    fn test_connection_state() {
        let mut state = ConnectionState::new(Some(Uuid::new_v4()));
        let channel = StreamChannel::OrderBook {
            token_id: Uuid::new_v4(),
        };

        // Subscribe
        state.subscribe(channel.clone(), 0);
        assert!(state.is_subscribed(&channel));

        // Unsubscribe
        state.unsubscribe(&channel);
        assert!(!state.is_subscribed(&channel));
    }

    #[test]
    fn test_candlestick_interval_duration() {
        assert_eq!(
            CandlestickInterval::OneMinute.duration(),
            Duration::from_secs(60)
        );
        assert_eq!(
            CandlestickInterval::OneHour.duration(),
            Duration::from_secs(3600)
        );
        assert_eq!(
            CandlestickInterval::OneDay.duration(),
            Duration::from_secs(86400)
        );
    }
}
