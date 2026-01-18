//! Price manipulation circuit breaker
//!
//! Detects and prevents price manipulation through:
//! - Maximum price change per hour
//! - Maximum volume per user
//! - Cooldown periods after triggers

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Configuration for the price circuit breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCircuitBreakerConfig {
    /// Maximum price change allowed per hour (as percentage, e.g., 0.5 = 50%)
    pub max_price_change_per_hour: Decimal,
    /// Maximum volume a single user can trade per hour
    pub max_user_volume_per_hour: Decimal,
    /// Maximum volume a single user can trade per day
    pub max_user_volume_per_day: Decimal,
    /// Cooldown period in seconds after circuit breaker triggers
    pub cooldown_seconds: u64,
    /// Minimum time between trades for same user (anti-bot)
    pub min_trade_interval_ms: u64,
    /// Threshold for suspicious volume spike (multiplier of average)
    pub volume_spike_threshold: Decimal,
    /// Number of recent trades to consider for pattern detection
    pub pattern_detection_window: usize,
}

impl Default for PriceCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_price_change_per_hour: dec!(0.5), // 50% max change per hour
            max_user_volume_per_hour: dec!(1000), // Max 1000 tokens per user per hour
            max_user_volume_per_day: dec!(5000),  // Max 5000 tokens per user per day
            cooldown_seconds: 300,                // 5 minute cooldown
            min_trade_interval_ms: 100,           // 100ms between trades
            volume_spike_threshold: dec!(3),      // 3x average volume is suspicious
            pattern_detection_window: 100,        // Last 100 trades
        }
    }
}

impl PriceCircuitBreakerConfig {
    /// Strict configuration for high-risk tokens
    pub fn strict() -> Self {
        Self {
            max_price_change_per_hour: dec!(0.25),
            max_user_volume_per_hour: dec!(500),
            max_user_volume_per_day: dec!(2000),
            cooldown_seconds: 600,
            min_trade_interval_ms: 500,
            volume_spike_threshold: dec!(2),
            pattern_detection_window: 200,
        }
    }

    /// Lenient configuration for established tokens
    pub fn lenient() -> Self {
        Self {
            max_price_change_per_hour: dec!(1.0), // 100%
            max_user_volume_per_hour: dec!(5000),
            max_user_volume_per_day: dec!(20000),
            cooldown_seconds: 120,
            min_trade_interval_ms: 50,
            volume_spike_threshold: dec!(5),
            pattern_detection_window: 50,
        }
    }
}

/// State of the circuit breaker for a token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerState {
    pub token_id: Uuid,
    pub is_triggered: bool,
    pub trigger_reason: Option<CircuitBreakerReason>,
    pub triggered_at: Option<i64>,
    pub cooldown_ends_at: Option<i64>,
    pub price_history: Vec<PricePoint>,
    pub user_volumes: HashMap<Uuid, UserVolumeStats>,
    pub recent_trades: Vec<TradeRecord>,
}

impl CircuitBreakerState {
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            is_triggered: false,
            trigger_reason: None,
            triggered_at: None,
            cooldown_ends_at: None,
            price_history: Vec::with_capacity(60), // 1 hour of minute data
            user_volumes: HashMap::new(),
            recent_trades: Vec::with_capacity(100),
        }
    }

    /// Check if cooldown has expired
    pub fn is_in_cooldown(&self, now: i64) -> bool {
        if let Some(cooldown_end) = self.cooldown_ends_at {
            now < cooldown_end
        } else {
            false
        }
    }
}

/// Reason for circuit breaker trigger
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CircuitBreakerReason {
    ExcessivePriceChange {
        change_percent: String,
    },
    UserVolumeLimitExceeded {
        user_id: Uuid,
        volume: String,
    },
    VolumeSpikeDetected {
        multiplier: String,
    },
    RapidTrading {
        user_id: Uuid,
        trades_per_second: String,
    },
    WashTradingDetected {
        users: Vec<Uuid>,
    },
    ManualTrigger {
        admin_id: Uuid,
    },
}

/// Price point for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    pub price: Decimal,
    pub timestamp: i64,
}

/// User volume statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserVolumeStats {
    pub hourly_volume: Decimal,
    pub daily_volume: Decimal,
    pub last_trade_at: i64,
    pub trades_last_minute: u32,
    pub hour_start: i64,
    pub day_start: i64,
}

/// Record of a trade for pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub trade_id: Uuid,
    pub buyer_id: Uuid,
    pub seller_id: Option<Uuid>,
    pub amount: Decimal,
    pub price: Decimal,
    pub timestamp: i64,
}

/// Result of a circuit breaker check
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    pub allowed: bool,
    pub reason: Option<CircuitBreakerReason>,
    pub warnings: Vec<String>,
}

impl CheckResult {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
            warnings: Vec::new(),
        }
    }

    pub fn blocked(reason: CircuitBreakerReason) -> Self {
        Self {
            allowed: false,
            reason: Some(reason),
            warnings: Vec::new(),
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Price circuit breaker service
pub struct PriceCircuitBreaker {
    config: PriceCircuitBreakerConfig,
    states: HashMap<Uuid, CircuitBreakerState>,
}

impl PriceCircuitBreaker {
    pub fn new(config: PriceCircuitBreakerConfig) -> Self {
        Self {
            config,
            states: HashMap::new(),
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(PriceCircuitBreakerConfig::default())
    }

    /// Get or create state for a token
    pub fn get_state(&mut self, token_id: Uuid) -> &mut CircuitBreakerState {
        self.states
            .entry(token_id)
            .or_insert_with(|| CircuitBreakerState::new(token_id))
    }

    /// Check if a trade is allowed
    pub fn check_trade(
        &mut self,
        token_id: Uuid,
        user_id: Uuid,
        amount: Decimal,
        price: Decimal,
    ) -> CheckResult {
        let now = chrono::Utc::now().timestamp();
        let now_ms = chrono::Utc::now().timestamp_millis();

        // Copy config values to avoid borrow issues
        let config = self.config.clone();

        let state = self.get_state(token_id);

        // Check if in cooldown
        if state.is_triggered && state.is_in_cooldown(now) {
            return CheckResult::blocked(state.trigger_reason.clone().unwrap_or(
                CircuitBreakerReason::ManualTrigger {
                    admin_id: Uuid::nil(),
                },
            ));
        }

        // Reset trigger if cooldown expired
        if state.is_triggered && !state.is_in_cooldown(now) {
            state.is_triggered = false;
            state.trigger_reason = None;
            state.triggered_at = None;
            state.cooldown_ends_at = None;
        }

        let mut result = CheckResult::allowed();

        // Check price change
        if let Some(reason) = Self::check_price_change_static(state, price, now, &config) {
            return Self::trigger_static(state, reason, now, &config);
        }

        // Check user volume
        if let Some(reason) = Self::check_user_volume_static(state, user_id, amount, &config) {
            return Self::trigger_static(state, reason, now, &config);
        }

        // Check trade frequency
        if let Some(reason) = Self::check_trade_frequency_static(state, user_id, now_ms, &config) {
            return Self::trigger_static(state, reason, now, &config);
        }

        // Check volume spike
        if let Some(reason) = Self::check_volume_spike_static(state, amount, &config) {
            result = result.with_warning(format!("Volume spike detected: {:?}", reason));
        }

        result
    }

    /// Record a completed trade
    pub fn record_trade(&mut self, token_id: Uuid, trade: TradeRecord) {
        let now = chrono::Utc::now().timestamp();
        let pattern_window = self.config.pattern_detection_window;

        let state = self.get_state(token_id);

        // Update price history
        state.price_history.push(PricePoint {
            price: trade.price,
            timestamp: trade.timestamp,
        });

        // Keep only last hour of prices
        let hour_ago = now - 3600;
        state.price_history.retain(|p| p.timestamp > hour_ago);

        // Update user volumes
        let user_stats = state.user_volumes.entry(trade.buyer_id).or_default();
        Self::update_user_stats_static(user_stats, trade.amount, trade.timestamp);

        // Add to recent trades
        state.recent_trades.push(trade);
        if state.recent_trades.len() > pattern_window {
            state.recent_trades.remove(0);
        }
    }

    fn check_price_change_static(
        state: &CircuitBreakerState,
        current_price: Decimal,
        now: i64,
        config: &PriceCircuitBreakerConfig,
    ) -> Option<CircuitBreakerReason> {
        let hour_ago = now - 3600;

        // Get price from an hour ago
        let old_price = state
            .price_history
            .iter()
            .rfind(|p| p.timestamp <= hour_ago)
            .or_else(|| state.price_history.first());

        if let Some(old_point) = old_price {
            if old_point.price > Decimal::ZERO {
                let change = (current_price - old_point.price).abs() / old_point.price;
                if change > config.max_price_change_per_hour {
                    return Some(CircuitBreakerReason::ExcessivePriceChange {
                        change_percent: format!("{:.2}%", change * dec!(100)),
                    });
                }
            }
        }

        None
    }

    fn check_user_volume_static(
        state: &CircuitBreakerState,
        user_id: Uuid,
        amount: Decimal,
        config: &PriceCircuitBreakerConfig,
    ) -> Option<CircuitBreakerReason> {
        if let Some(stats) = state.user_volumes.get(&user_id) {
            // Check hourly limit
            if stats.hourly_volume + amount > config.max_user_volume_per_hour {
                return Some(CircuitBreakerReason::UserVolumeLimitExceeded {
                    user_id,
                    volume: format!("{} (hourly)", stats.hourly_volume + amount),
                });
            }

            // Check daily limit
            if stats.daily_volume + amount > config.max_user_volume_per_day {
                return Some(CircuitBreakerReason::UserVolumeLimitExceeded {
                    user_id,
                    volume: format!("{} (daily)", stats.daily_volume + amount),
                });
            }
        }

        None
    }

    fn check_trade_frequency_static(
        state: &CircuitBreakerState,
        user_id: Uuid,
        now_ms: i64,
        config: &PriceCircuitBreakerConfig,
    ) -> Option<CircuitBreakerReason> {
        if let Some(stats) = state.user_volumes.get(&user_id) {
            let time_since_last = now_ms - stats.last_trade_at;
            if time_since_last < config.min_trade_interval_ms as i64 && stats.last_trade_at > 0 {
                let trades_per_second = 1000.0 / time_since_last.max(1) as f64;
                return Some(CircuitBreakerReason::RapidTrading {
                    user_id,
                    trades_per_second: format!("{:.2}", trades_per_second),
                });
            }
        }

        None
    }

    fn check_volume_spike_static(
        state: &CircuitBreakerState,
        amount: Decimal,
        config: &PriceCircuitBreakerConfig,
    ) -> Option<CircuitBreakerReason> {
        if state.recent_trades.len() < 10 {
            return None; // Not enough data
        }

        let total_volume: Decimal = state.recent_trades.iter().map(|t| t.amount).sum();
        let avg_volume = total_volume / Decimal::from(state.recent_trades.len());

        if avg_volume > Decimal::ZERO && amount > avg_volume * config.volume_spike_threshold {
            return Some(CircuitBreakerReason::VolumeSpikeDetected {
                multiplier: format!("{:.2}x", amount / avg_volume),
            });
        }

        None
    }

    fn update_user_stats_static(stats: &mut UserVolumeStats, amount: Decimal, timestamp: i64) {
        let hour_start = timestamp - (timestamp % 3600);
        let day_start = timestamp - (timestamp % 86400);

        // Reset hourly if new hour
        if hour_start != stats.hour_start {
            stats.hourly_volume = Decimal::ZERO;
            stats.hour_start = hour_start;
        }

        // Reset daily if new day
        if day_start != stats.day_start {
            stats.daily_volume = Decimal::ZERO;
            stats.day_start = day_start;
        }

        stats.hourly_volume += amount;
        stats.daily_volume += amount;
        stats.last_trade_at = timestamp;
    }

    fn trigger_static(
        state: &mut CircuitBreakerState,
        reason: CircuitBreakerReason,
        now: i64,
        config: &PriceCircuitBreakerConfig,
    ) -> CheckResult {
        state.is_triggered = true;
        state.trigger_reason = Some(reason.clone());
        state.triggered_at = Some(now);
        state.cooldown_ends_at = Some(now + config.cooldown_seconds as i64);

        tracing::warn!(
            token_id = %state.token_id,
            reason = ?reason,
            "Price circuit breaker triggered"
        );

        CheckResult::blocked(reason)
    }

    /// Manually trigger the circuit breaker
    pub fn manual_trigger(&mut self, token_id: Uuid, admin_id: Uuid) -> CheckResult {
        let now = chrono::Utc::now().timestamp();
        let cooldown_seconds = self.config.cooldown_seconds;
        let reason = CircuitBreakerReason::ManualTrigger { admin_id };

        let state = self.get_state(token_id);
        state.is_triggered = true;
        state.trigger_reason = Some(reason.clone());
        state.triggered_at = Some(now);
        state.cooldown_ends_at = Some(now + cooldown_seconds as i64);

        tracing::warn!(
            token_id = %state.token_id,
            reason = ?reason,
            "Price circuit breaker triggered"
        );

        CheckResult::blocked(reason)
    }

    /// Manually reset the circuit breaker
    pub fn reset(&mut self, token_id: Uuid) {
        if let Some(state) = self.states.get_mut(&token_id) {
            state.is_triggered = false;
            state.trigger_reason = None;
            state.triggered_at = None;
            state.cooldown_ends_at = None;
        }
    }

    /// Get current status for a token
    pub fn get_status(&self, token_id: Uuid) -> Option<&CircuitBreakerState> {
        self.states.get(&token_id)
    }
}

impl Default for PriceCircuitBreaker {
    fn default() -> Self {
        Self::with_default_config()
    }
}
