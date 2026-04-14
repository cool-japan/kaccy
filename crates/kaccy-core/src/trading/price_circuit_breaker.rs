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
    /// Token this circuit breaker state tracks
    pub token_id: Uuid,
    /// Whether the circuit breaker is currently triggered
    pub is_triggered: bool,
    /// Reason the circuit breaker was last triggered
    pub trigger_reason: Option<CircuitBreakerReason>,
    /// Unix timestamp when the breaker was triggered
    pub triggered_at: Option<i64>,
    /// Unix timestamp when the cooldown period ends
    pub cooldown_ends_at: Option<i64>,
    /// Recent price history for detecting excessive moves
    pub price_history: Vec<PricePoint>,
    /// Per-user volume statistics for limit enforcement
    pub user_volumes: HashMap<Uuid, UserVolumeStats>,
    /// Recent trades for pattern analysis
    pub recent_trades: Vec<TradeRecord>,
}

impl CircuitBreakerState {
    /// Create a new circuit breaker state for the given token
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
    /// Price moved more than the configured maximum in the measurement window
    ExcessivePriceChange {
        /// Formatted percentage of the price change that triggered the breaker
        change_percent: String,
    },
    /// A single user exceeded their hourly or daily volume limit
    UserVolumeLimitExceeded {
        /// User who exceeded the limit
        user_id: Uuid,
        /// Total volume that caused the violation
        volume: String,
    },
    /// Sudden volume spike exceeded the configured multiplier threshold
    VolumeSpikeDetected {
        /// How many times the average volume the spike represents
        multiplier: String,
    },
    /// A user is submitting orders too quickly (potential bot activity)
    RapidTrading {
        /// User submitting orders at an excessive rate
        user_id: Uuid,
        /// Observed trades-per-second rate
        trades_per_second: String,
    },
    /// Wash trading pattern detected between accounts
    WashTradingDetected {
        /// Users involved in the suspected wash trade
        users: Vec<Uuid>,
    },
    /// Circuit breaker was triggered manually by an administrator
    ManualTrigger {
        /// Administrator who triggered the breaker
        admin_id: Uuid,
    },
}

/// Price point for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    /// Token price at this point in time
    pub price: Decimal,
    /// Unix timestamp of the observation
    pub timestamp: i64,
}

/// User volume statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserVolumeStats {
    /// Total tokens traded in the current hour window
    pub hourly_volume: Decimal,
    /// Total tokens traded in the current day window
    pub daily_volume: Decimal,
    /// Millisecond timestamp of the most recent trade by this user
    pub last_trade_at: i64,
    /// Number of trades submitted in the last minute
    pub trades_last_minute: u32,
    /// Unix timestamp marking the start of the current hour bucket
    pub hour_start: i64,
    /// Unix timestamp marking the start of the current day bucket
    pub day_start: i64,
}

/// Record of a trade for pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    /// Unique identifier for this trade
    pub trade_id: Uuid,
    /// User that placed the buy side of this trade
    pub buyer_id: Uuid,
    /// User that placed the sell side, if known
    pub seller_id: Option<Uuid>,
    /// Token quantity traded
    pub amount: Decimal,
    /// Execution price of the trade
    pub price: Decimal,
    /// Unix timestamp when the trade occurred
    pub timestamp: i64,
}

/// Result of a circuit breaker check
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    /// Whether the trade is permitted
    pub allowed: bool,
    /// Reason the trade was blocked, if applicable
    pub reason: Option<CircuitBreakerReason>,
    /// Non-fatal warning messages generated during the check
    pub warnings: Vec<String>,
}

impl CheckResult {
    /// Create an allowed result with no warnings
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
            warnings: Vec::new(),
        }
    }

    /// Create a blocked result with the given reason
    pub fn blocked(reason: CircuitBreakerReason) -> Self {
        Self {
            allowed: false,
            reason: Some(reason),
            warnings: Vec::new(),
        }
    }

    /// Append a warning message to this result
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Price circuit breaker service
pub struct PriceCircuitBreaker {
    /// Configuration governing when to trigger the circuit breaker
    config: PriceCircuitBreakerConfig,
    /// Per-token circuit breaker states
    states: HashMap<Uuid, CircuitBreakerState>,
}

impl PriceCircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: PriceCircuitBreakerConfig) -> Self {
        Self {
            config,
            states: HashMap::new(),
        }
    }

    /// Create a new circuit breaker with the default configuration
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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_token_id() -> Uuid {
        Uuid::new_v4()
    }

    fn make_user_id() -> Uuid {
        Uuid::new_v4()
    }

    // ----------------------------------------------------------------
    // Config preset tests
    // ----------------------------------------------------------------

    #[test]
    fn test_default_config_values() {
        let config = PriceCircuitBreakerConfig::default();
        assert_eq!(config.max_price_change_per_hour, dec!(0.5));
        assert_eq!(config.max_user_volume_per_hour, dec!(1000));
        assert_eq!(config.max_user_volume_per_day, dec!(5000));
        assert_eq!(config.cooldown_seconds, 300);
        assert_eq!(config.min_trade_interval_ms, 100);
        assert_eq!(config.volume_spike_threshold, dec!(3));
        assert_eq!(config.pattern_detection_window, 100);
    }

    #[test]
    fn test_strict_config_is_tighter_than_default() {
        let strict = PriceCircuitBreakerConfig::strict();
        let default = PriceCircuitBreakerConfig::default();
        assert!(strict.max_price_change_per_hour < default.max_price_change_per_hour);
        assert!(strict.max_user_volume_per_hour < default.max_user_volume_per_hour);
        assert!(strict.cooldown_seconds > default.cooldown_seconds);
        assert!(strict.min_trade_interval_ms > default.min_trade_interval_ms);
    }

    #[test]
    fn test_lenient_config_is_looser_than_default() {
        let lenient = PriceCircuitBreakerConfig::lenient();
        let default = PriceCircuitBreakerConfig::default();
        assert!(lenient.max_price_change_per_hour > default.max_price_change_per_hour);
        assert!(lenient.max_user_volume_per_hour > default.max_user_volume_per_hour);
        assert!(lenient.cooldown_seconds < default.cooldown_seconds);
    }

    // ----------------------------------------------------------------
    // Allowed trade — no issues
    // ----------------------------------------------------------------

    #[test]
    fn test_trade_allowed_when_no_history() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let user_id = make_user_id();

        let result = cb.check_trade(token_id, user_id, dec!(10), dec!(1.0));
        assert!(
            result.allowed,
            "Trade with no prior history must be allowed"
        );
        assert!(result.reason.is_none());
    }

    // ----------------------------------------------------------------
    // Cooldown state helpers
    // ----------------------------------------------------------------

    #[test]
    fn test_is_in_cooldown_while_active() {
        let token_id = make_token_id();
        let mut state = CircuitBreakerState::new(token_id);
        let now = chrono::Utc::now().timestamp();

        // Put the cooldown end well in the future
        state.cooldown_ends_at = Some(now + 3600);
        assert!(
            state.is_in_cooldown(now),
            "State must report in-cooldown when end is in the future"
        );
    }

    #[test]
    fn test_is_in_cooldown_after_expiry() {
        let token_id = make_token_id();
        let mut state = CircuitBreakerState::new(token_id);
        let now = chrono::Utc::now().timestamp();

        // Cooldown ended in the past
        state.cooldown_ends_at = Some(now - 1);
        assert!(
            !state.is_in_cooldown(now),
            "State must not report in-cooldown when end is in the past"
        );
    }

    #[test]
    fn test_is_in_cooldown_when_none() {
        let token_id = make_token_id();
        let state = CircuitBreakerState::new(token_id);
        let now = chrono::Utc::now().timestamp();

        assert!(
            !state.is_in_cooldown(now),
            "Newly created state must not be in cooldown"
        );
    }

    // ----------------------------------------------------------------
    // Manual trigger and reset
    // ----------------------------------------------------------------

    #[test]
    fn test_manual_trigger_blocks_trade() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let admin_id = make_user_id();

        let trigger_result = cb.manual_trigger(token_id, admin_id);
        assert!(!trigger_result.allowed, "Manual trigger must block trades");

        // Subsequent check must also be blocked while in cooldown
        let user_id = make_user_id();
        let check_result = cb.check_trade(token_id, user_id, dec!(1), dec!(1.0));
        assert!(
            !check_result.allowed,
            "Trade after manual trigger must be blocked during cooldown"
        );
    }

    #[test]
    fn test_reset_after_manual_trigger_allows_trade() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let admin_id = make_user_id();

        cb.manual_trigger(token_id, admin_id);
        cb.reset(token_id);

        let user_id = make_user_id();
        let result = cb.check_trade(token_id, user_id, dec!(1), dec!(1.0));
        assert!(
            result.allowed,
            "Trade after reset must be allowed even within original cooldown window"
        );
    }

    #[test]
    fn test_manual_trigger_reason_is_manual_trigger() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let admin_id = make_user_id();

        let result = cb.manual_trigger(token_id, admin_id);
        assert!(
            matches!(
                result.reason,
                Some(CircuitBreakerReason::ManualTrigger { .. })
            ),
            "Manual trigger must produce ManualTrigger reason"
        );
    }

    // ----------------------------------------------------------------
    // Excessive price change detection
    // ----------------------------------------------------------------

    #[test]
    fn test_price_change_triggers_circuit_breaker() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let user_id = make_user_id();

        // Seed historical price from 2 hours ago so rfind finds it
        let now = chrono::Utc::now().timestamp();
        let two_hours_ago = now - 7200;
        let state = cb.get_state(token_id);
        state.price_history.push(PricePoint {
            price: dec!(1.0),
            timestamp: two_hours_ago,
        });

        // Default threshold is 50%; jump from 1.0 to 2.0 is exactly 100% — must trigger
        let result = cb.check_trade(token_id, user_id, dec!(1), dec!(2.0));
        assert!(
            !result.allowed,
            "A 100% price jump must trigger the circuit breaker (threshold 50%)"
        );
        assert!(
            matches!(
                result.reason,
                Some(CircuitBreakerReason::ExcessivePriceChange { .. })
            ),
            "Reason must be ExcessivePriceChange"
        );
    }

    #[test]
    fn test_small_price_change_does_not_trigger() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let user_id = make_user_id();

        let now = chrono::Utc::now().timestamp();
        let two_hours_ago = now - 7200;
        let state = cb.get_state(token_id);
        state.price_history.push(PricePoint {
            price: dec!(1.0),
            timestamp: two_hours_ago,
        });

        // 5% move — well below 50% threshold
        let result = cb.check_trade(token_id, user_id, dec!(1), dec!(1.05));
        assert!(
            result.allowed,
            "A 5% price change must not trigger the circuit breaker"
        );
    }

    // ----------------------------------------------------------------
    // User volume limit detection
    // ----------------------------------------------------------------

    #[test]
    fn test_user_volume_limit_triggers_circuit_breaker() {
        // Use a config with a very low hourly limit so we can exceed it easily
        let config = PriceCircuitBreakerConfig {
            max_user_volume_per_hour: dec!(100),
            max_user_volume_per_day: dec!(200),
            ..PriceCircuitBreakerConfig::default()
        };
        let mut cb = PriceCircuitBreaker::new(config);
        let token_id = make_token_id();
        let user_id = make_user_id();

        // Record a trade that consumes 90 tokens of the 100 hourly limit
        let now = chrono::Utc::now().timestamp();
        cb.record_trade(
            token_id,
            TradeRecord {
                trade_id: Uuid::new_v4(),
                buyer_id: user_id,
                seller_id: None,
                amount: dec!(90),
                price: dec!(1.0),
                timestamp: now,
            },
        );

        // Now attempt a trade for 20 more — should exceed 100 limit
        let result = cb.check_trade(token_id, user_id, dec!(20), dec!(1.0));
        assert!(
            !result.allowed,
            "Exceeding hourly user volume must trigger the circuit breaker"
        );
        assert!(
            matches!(
                result.reason,
                Some(CircuitBreakerReason::UserVolumeLimitExceeded { .. })
            ),
            "Reason must be UserVolumeLimitExceeded"
        );
    }

    #[test]
    fn test_user_volume_within_limit_is_allowed() {
        let config = PriceCircuitBreakerConfig {
            max_user_volume_per_hour: dec!(100),
            max_user_volume_per_day: dec!(200),
            ..PriceCircuitBreakerConfig::default()
        };
        let mut cb = PriceCircuitBreaker::new(config);
        let token_id = make_token_id();
        let user_id = make_user_id();

        let now = chrono::Utc::now().timestamp();
        cb.record_trade(
            token_id,
            TradeRecord {
                trade_id: Uuid::new_v4(),
                buyer_id: user_id,
                seller_id: None,
                amount: dec!(50),
                price: dec!(1.0),
                timestamp: now,
            },
        );

        // 30 more — total 80, still within 100
        let result = cb.check_trade(token_id, user_id, dec!(30), dec!(1.0));
        assert!(
            result.allowed,
            "Total volume of 80 must be under the 100 hourly limit"
        );
    }

    // ----------------------------------------------------------------
    // Volume spike detection (warning only, not block)
    // ----------------------------------------------------------------

    #[test]
    fn test_volume_spike_generates_warning_but_allows_trade() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();

        // Seed at least 10 small trades so the spike checker activates
        let now = chrono::Utc::now().timestamp();
        for _ in 0..10 {
            let uid = make_user_id();
            cb.record_trade(
                token_id,
                TradeRecord {
                    trade_id: Uuid::new_v4(),
                    buyer_id: uid,
                    seller_id: None,
                    amount: dec!(1),
                    price: dec!(1.0),
                    timestamp: now,
                },
            );
        }

        // Average is 1; spike threshold is 3x; sending 100x average.
        // Use a fresh user so the volume-limit check doesn't fire first.
        let new_user = make_user_id();
        let result = cb.check_trade(token_id, new_user, dec!(100), dec!(1.0));
        // Volume spike produces warning but still allows (not a hard block)
        assert!(
            result.allowed,
            "Volume spike must not block the trade — only generate a warning"
        );
        assert!(
            !result.warnings.is_empty(),
            "Volume spike must produce at least one warning"
        );
    }

    // ----------------------------------------------------------------
    // Status / state retrieval
    // ----------------------------------------------------------------

    #[test]
    fn test_get_status_returns_none_for_unknown_token() {
        let cb = PriceCircuitBreaker::with_default_config();
        let unknown_token = make_token_id();
        assert!(
            cb.get_status(unknown_token).is_none(),
            "get_status must return None for a token that was never accessed"
        );
    }

    #[test]
    fn test_get_status_returns_state_after_manual_trigger() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let admin_id = make_user_id();

        cb.manual_trigger(token_id, admin_id);

        let status = cb
            .get_status(token_id)
            .expect("get_status must return Some after manual trigger");
        assert!(status.is_triggered, "State must be triggered");
        assert!(
            status.trigger_reason.is_some(),
            "Trigger reason must be set"
        );
        assert!(status.triggered_at.is_some(), "triggered_at must be set");
        assert!(
            status.cooldown_ends_at.is_some(),
            "cooldown_ends_at must be set"
        );
    }

    #[test]
    fn test_record_trade_populates_price_history() {
        let mut cb = PriceCircuitBreaker::with_default_config();
        let token_id = make_token_id();
        let user_id = make_user_id();

        let now = chrono::Utc::now().timestamp();
        cb.record_trade(
            token_id,
            TradeRecord {
                trade_id: Uuid::new_v4(),
                buyer_id: user_id,
                seller_id: None,
                amount: dec!(10),
                price: dec!(1.5),
                timestamp: now,
            },
        );

        let status = cb
            .get_status(token_id)
            .expect("state must exist after record_trade");
        assert_eq!(
            status.price_history.len(),
            1,
            "Price history must contain exactly the recorded trade's price point"
        );
        assert_eq!(
            status.price_history[0].price,
            dec!(1.5),
            "Recorded price must match"
        );
    }
}
