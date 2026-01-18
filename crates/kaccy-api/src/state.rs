//! Application state

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;

use jsonwebtoken::{DecodingKey, Validation, decode};
use kaccy_bitcoin::BitcoinClient;
use kaccy_db::{CacheConfig, RedisCache};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::events::EventBus;
use crate::jobs::JobProcessor;
use crate::middleware::rate_limit::{DynamicRateLimiter, LoginAttemptTracker};
use crate::routes::ws::{OrderUpdate, PriceUpdate, TradeNotification, WsBroadcaster};
use tokio::sync::Mutex;

/// JWT claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,
}

/// Application state shared across request handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub pool: PgPool,
    /// JWT secret for token signing/verification
    pub jwt_secret: String,
    /// Application start time for uptime tracking
    pub start_time: Instant,
    /// Bitcoin client (optional, for payment processing)
    pub bitcoin_client: Option<Arc<BitcoinClient>>,
    /// Redis cache (optional, for caching)
    pub redis_cache: Option<Arc<RedisCache>>,
    /// Login attempt tracker for rate limiting and lockout
    pub login_tracker: LoginAttemptTracker,
    /// WebSocket broadcaster for real-time updates
    pub ws_broadcaster: WsBroadcaster,
    /// Dynamic rate limiter for adaptive rate limiting
    pub rate_limiter: DynamicRateLimiter,
    /// Event bus for event-driven architecture (optional)
    pub event_bus: Option<Arc<EventBus>>,
    /// Job processor for background job processing (optional, uses Mutex for shutdown)
    pub job_processor: Option<Arc<Mutex<JobProcessor>>>,
}

impl AppState {
    /// Create new app state with database pool and JWT secret
    pub fn new(pool: PgPool, jwt_secret: String) -> Self {
        Self {
            pool,
            jwt_secret,
            start_time: Instant::now(),
            bitcoin_client: None,
            redis_cache: None,
            login_tracker: LoginAttemptTracker::default(),
            ws_broadcaster: WsBroadcaster::new(),
            rate_limiter: DynamicRateLimiter::default(),
            event_bus: None,
            job_processor: None,
        }
    }

    /// Create new app state with all dependencies
    pub fn with_bitcoin(pool: PgPool, jwt_secret: String, bitcoin_client: BitcoinClient) -> Self {
        Self {
            pool,
            jwt_secret,
            start_time: Instant::now(),
            bitcoin_client: Some(Arc::new(bitcoin_client)),
            redis_cache: None,
            login_tracker: LoginAttemptTracker::default(),
            ws_broadcaster: WsBroadcaster::new(),
            rate_limiter: DynamicRateLimiter::default(),
            event_bus: None,
            job_processor: None,
        }
    }

    /// Add Redis cache to app state
    pub fn with_redis(mut self, cache: RedisCache) -> Self {
        self.redis_cache = Some(Arc::new(cache));
        self
    }

    /// Add Bitcoin client to app state
    pub fn add_bitcoin_client(mut self, client: BitcoinClient) -> Self {
        self.bitcoin_client = Some(Arc::new(client));
        self
    }

    /// Create Redis cache from config
    pub async fn connect_redis(&mut self, config: CacheConfig) -> Result<(), kaccy_db::DbError> {
        let cache = RedisCache::new(config).await?;
        self.redis_cache = Some(Arc::new(cache));
        Ok(())
    }

    /// Check if Bitcoin client is configured
    pub fn has_bitcoin(&self) -> bool {
        self.bitcoin_client.is_some()
    }

    /// Get Bitcoin client reference (returns error if not configured)
    pub fn bitcoin(&self) -> Result<&BitcoinClient, BitcoinNotConfigured> {
        self.bitcoin_client
            .as_ref()
            .map(|c| c.as_ref())
            .ok_or(BitcoinNotConfigured)
    }

    /// Check if an IP is locked out from login attempts
    pub fn check_login_lockout(&self, ip: IpAddr) -> Option<std::time::Duration> {
        self.login_tracker.is_locked_out(ip)
    }

    /// Record a failed login attempt
    pub fn record_failed_login(&self, ip: IpAddr) {
        self.login_tracker.record_failure(ip);
    }

    /// Clear login attempts (on successful login)
    pub fn clear_login_attempts(&self, ip: IpAddr) {
        self.login_tracker.clear(ip);
    }

    /// Verify a JWT token and return the user ID
    pub fn verify_token(&self, token: &str) -> Option<Uuid> {
        let validation = Validation::default();
        let key = DecodingKey::from_secret(self.jwt_secret.as_bytes());

        decode::<Claims>(token, &key, &validation)
            .ok()
            .and_then(|data| Uuid::parse_str(&data.claims.sub).ok())
    }

    /// Subscribe to price updates
    pub fn subscribe_prices(&self) -> broadcast::Receiver<PriceUpdate> {
        self.ws_broadcaster.subscribe_prices()
    }

    /// Subscribe to trade updates for a specific token
    pub fn subscribe_trades(&self, token_id: Uuid) -> broadcast::Receiver<TradeNotification> {
        self.ws_broadcaster.subscribe_trades(token_id)
    }

    /// Subscribe to order updates for a specific user
    pub fn subscribe_user_orders(&self, user_id: Uuid) -> broadcast::Receiver<OrderUpdate> {
        self.ws_broadcaster.subscribe_user_orders(user_id)
    }

    /// Broadcast a price update
    pub fn broadcast_price(&self, update: PriceUpdate) {
        self.ws_broadcaster.broadcast_price(update);
    }

    /// Broadcast a trade notification
    pub fn broadcast_trade(&self, token_id: Uuid, trade: TradeNotification) {
        self.ws_broadcaster.broadcast_trade(token_id, trade);
    }

    /// Broadcast an order update
    pub fn broadcast_order(&self, user_id: Uuid, update: OrderUpdate) {
        self.ws_broadcaster.broadcast_order(user_id, update);
    }

    /// Check if Redis cache is configured
    pub fn has_redis(&self) -> bool {
        self.redis_cache.is_some()
    }

    /// Get Redis cache reference (returns error if not configured)
    pub fn redis(&self) -> Result<&RedisCache, RedisNotConfigured> {
        self.redis_cache
            .as_ref()
            .map(|c| c.as_ref())
            .ok_or(RedisNotConfigured)
    }

    /// Cache a user session in Redis
    pub async fn cache_session(&self, session_id: &str, user_id: Uuid) -> Option<()> {
        if let Some(cache) = &self.redis_cache {
            cache.set_session(session_id, user_id).await.ok()
        } else {
            None
        }
    }

    /// Get cached session from Redis
    pub async fn get_cached_session(&self, session_id: &str) -> Option<Uuid> {
        if let Some(cache) = &self.redis_cache {
            cache.get_session(session_id).await.ok().flatten()
        } else {
            None
        }
    }

    /// Invalidate a cached session
    pub async fn invalidate_session(&self, session_id: &str) -> Option<bool> {
        if let Some(cache) = &self.redis_cache {
            cache.invalidate_session(session_id).await.ok()
        } else {
            None
        }
    }

    /// Cache token price
    pub async fn cache_token_price(&self, token_id: Uuid, price_btc: f64) -> Option<()> {
        if let Some(cache) = &self.redis_cache {
            cache.set_token_price(token_id, price_btc).await.ok()
        } else {
            None
        }
    }

    /// Get cached token price
    pub async fn get_cached_token_price(&self, token_id: Uuid) -> Option<f64> {
        if let Some(cache) = &self.redis_cache {
            cache.get_token_price(token_id).await.ok().flatten()
        } else {
            None
        }
    }

    /// Check rate limit using Redis (if available)
    pub async fn check_redis_rate_limit(
        &self,
        identifier: &str,
        limit: u64,
        window_secs: u64,
    ) -> Option<(u64, bool)> {
        if let Some(cache) = &self.redis_cache {
            cache
                .check_rate_limit(identifier, limit, window_secs)
                .await
                .ok()
        } else {
            None
        }
    }

    /// Add event bus to app state
    pub fn with_event_bus(mut self, event_bus: EventBus) -> Self {
        self.event_bus = Some(Arc::new(event_bus));
        self
    }

    /// Add job processor to app state
    pub fn with_job_processor(mut self, job_processor: JobProcessor) -> Self {
        self.job_processor = Some(Arc::new(Mutex::new(job_processor)));
        self
    }

    /// Check if event bus is configured
    pub fn has_event_bus(&self) -> bool {
        self.event_bus.is_some()
    }

    /// Get event bus reference (returns error if not configured)
    pub fn event_bus(&self) -> Result<&EventBus, EventBusNotConfigured> {
        self.event_bus
            .as_ref()
            .map(|e| e.as_ref())
            .ok_or(EventBusNotConfigured)
    }

    /// Check if job processor is configured
    pub fn has_job_processor(&self) -> bool {
        self.job_processor.is_some()
    }

    /// Get job processor reference (returns error if not configured)
    pub fn job_processor(&self) -> Result<Arc<Mutex<JobProcessor>>, JobProcessorNotConfigured> {
        self.job_processor
            .as_ref()
            .map(|j| j.clone())
            .ok_or(JobProcessorNotConfigured)
    }

    /// Get application uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

/// Error returned when Bitcoin client is not configured
#[derive(Debug, Clone, Copy)]
pub struct BitcoinNotConfigured;

impl std::fmt::Display for BitcoinNotConfigured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bitcoin client is not configured")
    }
}

impl std::error::Error for BitcoinNotConfigured {}

/// Error returned when Redis cache is not configured
#[derive(Debug, Clone, Copy)]
pub struct RedisNotConfigured;

impl std::fmt::Display for RedisNotConfigured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Redis cache is not configured")
    }
}

impl std::error::Error for RedisNotConfigured {}

/// Error returned when event bus is not configured
#[derive(Debug, Clone, Copy)]
pub struct EventBusNotConfigured;

impl std::fmt::Display for EventBusNotConfigured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Event bus is not configured")
    }
}

impl std::error::Error for EventBusNotConfigured {}

/// Error returned when job processor is not configured
#[derive(Debug, Clone, Copy)]
pub struct JobProcessorNotConfigured;

impl std::fmt::Display for JobProcessorNotConfigured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Job processor is not configured")
    }
}

impl std::error::Error for JobProcessorNotConfigured {}
