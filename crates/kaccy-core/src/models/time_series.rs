//! Time-series analytics for historical data tracking and technical indicators
//!
//! This module provides functionality for tracking historical price data,
//! generating OHLCV candlesticks, and calculating technical indicators.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Timeframe for candlestick data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    /// 1 minute candles
    OneMinute,
    /// 5 minute candles
    FiveMinutes,
    /// 15 minute candles
    FifteenMinutes,
    /// 30 minute candles
    ThirtyMinutes,
    /// 1 hour candles
    OneHour,
    /// 4 hour candles
    FourHours,
    /// 1 day candles
    OneDay,
    /// 1 week candles
    OneWeek,
}

impl Timeframe {
    /// Get the duration for this timeframe
    pub fn duration(&self) -> Duration {
        match self {
            Self::OneMinute => Duration::minutes(1),
            Self::FiveMinutes => Duration::minutes(5),
            Self::FifteenMinutes => Duration::minutes(15),
            Self::ThirtyMinutes => Duration::minutes(30),
            Self::OneHour => Duration::hours(1),
            Self::FourHours => Duration::hours(4),
            Self::OneDay => Duration::days(1),
            Self::OneWeek => Duration::weeks(1),
        }
    }

    /// Get a human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::FifteenMinutes => "15m",
            Self::ThirtyMinutes => "30m",
            Self::OneHour => "1h",
            Self::FourHours => "4h",
            Self::OneDay => "1d",
            Self::OneWeek => "1w",
        }
    }
}

/// OHLCV candlestick data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    /// Token ID
    pub token_id: Uuid,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Opening timestamp
    pub open_time: DateTime<Utc>,
    /// Closing timestamp
    pub close_time: DateTime<Utc>,
    /// Opening price
    pub open: Decimal,
    /// Highest price
    pub high: Decimal,
    /// Lowest price
    pub low: Decimal,
    /// Closing price
    pub close: Decimal,
    /// Total volume
    pub volume: Decimal,
    /// Number of trades
    pub trade_count: u32,
}

impl Candle {
    /// Create a new candle
    pub fn new(
        token_id: Uuid,
        timeframe: Timeframe,
        open_time: DateTime<Utc>,
        price: Decimal,
    ) -> Self {
        let close_time = open_time + timeframe.duration();

        Self {
            token_id,
            timeframe,
            open_time,
            close_time,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: Decimal::ZERO,
            trade_count: 0,
        }
    }

    /// Update candle with a new trade
    pub fn add_trade(&mut self, price: Decimal, volume: Decimal) {
        self.close = price;
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.volume += volume;
        self.trade_count += 1;
    }

    /// Check if this candle is complete
    pub fn is_complete(&self) -> bool {
        Utc::now() >= self.close_time
    }

    /// Calculate typical price (HLC/3)
    pub fn typical_price(&self) -> Decimal {
        (self.high + self.low + self.close) / Decimal::from(3)
    }

    /// Calculate body percentage (relative to range)
    pub fn body_percentage(&self) -> Decimal {
        let range = self.high - self.low;
        if range.is_zero() {
            return Decimal::ZERO;
        }
        ((self.close - self.open).abs() / range) * Decimal::from(100)
    }
}

/// Volume profile for a specific price level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    /// Price level
    pub price_level: Decimal,
    /// Total volume at this level
    pub volume: Decimal,
    /// Number of trades at this level
    pub trade_count: u32,
    /// Buy volume
    pub buy_volume: Decimal,
    /// Sell volume
    pub sell_volume: Decimal,
}

/// Candlestick manager for building and storing time-series data
pub struct CandleManager {
    /// Active candles by token and timeframe
    active_candles: std::collections::HashMap<(Uuid, Timeframe), Candle>,
    /// Historical candles (limited size per timeframe)
    history: std::collections::HashMap<(Uuid, Timeframe), VecDeque<Candle>>,
    /// Maximum number of candles to keep in history
    max_history: usize,
}

impl CandleManager {
    /// Create a new candle manager
    pub fn new(max_history: usize) -> Self {
        Self {
            active_candles: std::collections::HashMap::new(),
            history: std::collections::HashMap::new(),
            max_history,
        }
    }

    /// Record a trade and update relevant candles
    pub fn record_trade(
        &mut self,
        token_id: Uuid,
        price: Decimal,
        volume: Decimal,
        timestamp: DateTime<Utc>,
    ) {
        for timeframe in &[
            Timeframe::OneMinute,
            Timeframe::FiveMinutes,
            Timeframe::FifteenMinutes,
            Timeframe::ThirtyMinutes,
            Timeframe::OneHour,
            Timeframe::FourHours,
            Timeframe::OneDay,
            Timeframe::OneWeek,
        ] {
            let key = (token_id, *timeframe);

            // Check if we need to complete the current candle
            let needs_new_candle = self
                .active_candles
                .get(&key)
                .map(|c| c.is_complete())
                .unwrap_or(true);

            if needs_new_candle {
                // Move old candle to history if it exists
                if let Some(old_candle) = self.active_candles.remove(&key) {
                    self.add_to_history(token_id, *timeframe, old_candle);
                }
                // Create new candle
                self.active_candles
                    .insert(key, Candle::new(token_id, *timeframe, timestamp, price));
            }

            // Update candle with trade data
            if let Some(candle) = self.active_candles.get_mut(&key) {
                candle.add_trade(price, volume);
            }
        }
    }

    /// Add a completed candle to history
    fn add_to_history(&mut self, token_id: Uuid, timeframe: Timeframe, candle: Candle) {
        let key = (token_id, timeframe);
        let history = self.history.entry(key).or_default();

        history.push_back(candle);

        // Trim history if needed
        while history.len() > self.max_history {
            history.pop_front();
        }
    }

    /// Get historical candles
    pub fn get_history(
        &self,
        token_id: Uuid,
        timeframe: Timeframe,
        limit: Option<usize>,
    ) -> Vec<Candle> {
        let key = (token_id, timeframe);
        if let Some(history) = self.history.get(&key) {
            let limit = limit.unwrap_or(history.len());
            history.iter().rev().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get the current active candle
    pub fn get_active_candle(&self, token_id: Uuid, timeframe: Timeframe) -> Option<&Candle> {
        self.active_candles.get(&(token_id, timeframe))
    }
}

impl Default for CandleManager {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// Technical indicator calculator
pub struct IndicatorCalculator {
    /// Historical closes for calculation
    closes: VecDeque<Decimal>,
    /// Maximum history to keep
    max_history: usize,
}

impl IndicatorCalculator {
    /// Create a new indicator calculator
    pub fn new(max_history: usize) -> Self {
        Self {
            closes: VecDeque::new(),
            max_history,
        }
    }

    /// Add a new closing price
    pub fn add_close(&mut self, close: Decimal) {
        self.closes.push_back(close);
        if self.closes.len() > self.max_history {
            self.closes.pop_front();
        }
    }

    /// Calculate Simple Moving Average (SMA)
    pub fn sma(&self, period: usize) -> Option<Decimal> {
        if self.closes.len() < period {
            return None;
        }

        let sum: Decimal = self.closes.iter().rev().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    /// Calculate Exponential Moving Average (EMA)
    pub fn ema(&self, period: usize) -> Option<Decimal> {
        if self.closes.len() < period {
            return None;
        }

        let multiplier = Decimal::from(2) / (Decimal::from(period) + Decimal::ONE);
        let mut ema = *self.closes.iter().rev().nth(period - 1)?;

        for close in self.closes.iter().rev().take(period - 1) {
            ema = (close - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// Calculate Relative Strength Index (RSI)
    pub fn rsi(&self, period: usize) -> Option<Decimal> {
        if self.closes.len() < period + 1 {
            return None;
        }

        let mut gains = Decimal::ZERO;
        let mut losses = Decimal::ZERO;

        let prices: Vec<Decimal> = self.closes.iter().rev().take(period + 1).copied().collect();

        for i in 1..=period {
            let change = prices[i - 1] - prices[i];
            if change > Decimal::ZERO {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / Decimal::from(period);
        let avg_loss = losses / Decimal::from(period);

        if avg_loss.is_zero() {
            return Some(Decimal::from(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs));

        Some(rsi)
    }

    /// Calculate Moving Average Convergence Divergence (MACD)
    pub fn macd(&self) -> Option<(Decimal, Decimal, Decimal)> {
        let ema12 = self.ema(12)?;
        let ema26 = self.ema(26)?;
        let macd_line = ema12 - ema26;

        // For signal line, we would need to track MACD values over time
        // Simplified version: use the current MACD value as signal
        let signal = macd_line;
        let histogram = macd_line - signal;

        Some((macd_line, signal, histogram))
    }

    /// Calculate Bollinger Bands
    pub fn bollinger_bands(
        &self,
        period: usize,
        std_dev: Decimal,
    ) -> Option<(Decimal, Decimal, Decimal)> {
        let sma = self.sma(period)?;

        if self.closes.len() < period {
            return None;
        }

        let recent: Vec<Decimal> = self.closes.iter().rev().take(period).copied().collect();

        // Calculate standard deviation
        let variance: Decimal = recent
            .iter()
            .map(|&x| {
                let diff = x - sma;
                diff * diff
            })
            .sum::<Decimal>()
            / Decimal::from(period);

        let std = variance.sqrt().unwrap_or(Decimal::ZERO);

        let upper = sma + (std * std_dev);
        let lower = sma - (std * std_dev);

        Some((upper, sma, lower))
    }
}

impl Default for IndicatorCalculator {
    fn default() -> Self {
        Self::new(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_candle_creation() {
        let token_id = Uuid::new_v4();
        let now = Utc::now();
        let candle = Candle::new(token_id, Timeframe::OneMinute, now, dec!(100));

        assert_eq!(candle.open, dec!(100));
        assert_eq!(candle.high, dec!(100));
        assert_eq!(candle.low, dec!(100));
        assert_eq!(candle.close, dec!(100));
        assert_eq!(candle.volume, Decimal::ZERO);
        assert_eq!(candle.trade_count, 0);
    }

    #[test]
    fn test_candle_add_trade() {
        let token_id = Uuid::new_v4();
        let now = Utc::now();
        let mut candle = Candle::new(token_id, Timeframe::OneHour, now, dec!(100));

        candle.add_trade(dec!(105), dec!(10));
        assert_eq!(candle.close, dec!(105));
        assert_eq!(candle.high, dec!(105));
        assert_eq!(candle.low, dec!(100));
        assert_eq!(candle.volume, dec!(10));
        assert_eq!(candle.trade_count, 1);

        candle.add_trade(dec!(95), dec!(5));
        assert_eq!(candle.close, dec!(95));
        assert_eq!(candle.high, dec!(105));
        assert_eq!(candle.low, dec!(95));
        assert_eq!(candle.volume, dec!(15));
        assert_eq!(candle.trade_count, 2);
    }

    #[test]
    fn test_candle_typical_price() {
        let token_id = Uuid::new_v4();
        let now = Utc::now();
        let mut candle = Candle::new(token_id, Timeframe::OneDay, now, dec!(100));

        candle.high = dec!(110);
        candle.low = dec!(90);
        candle.close = dec!(105);

        let typical = candle.typical_price();
        assert_eq!(typical, (dec!(110) + dec!(90) + dec!(105)) / dec!(3));
    }

    #[test]
    fn test_candle_manager_record_trade() {
        let mut manager = CandleManager::new(100);
        let token_id = Uuid::new_v4();
        let now = Utc::now();

        manager.record_trade(token_id, dec!(100), dec!(10), now);

        let candle = manager
            .get_active_candle(token_id, Timeframe::OneMinute)
            .unwrap();
        assert_eq!(candle.open, dec!(100));
        assert_eq!(candle.volume, dec!(10));
        assert_eq!(candle.trade_count, 1);
    }

    #[test]
    fn test_indicator_sma() {
        let mut calc = IndicatorCalculator::new(100);

        calc.add_close(dec!(10));
        calc.add_close(dec!(20));
        calc.add_close(dec!(30));
        calc.add_close(dec!(40));
        calc.add_close(dec!(50));

        let sma3 = calc.sma(3).unwrap();
        assert_eq!(sma3, (dec!(30) + dec!(40) + dec!(50)) / dec!(3));

        let sma5 = calc.sma(5).unwrap();
        assert_eq!(
            sma5,
            (dec!(10) + dec!(20) + dec!(30) + dec!(40) + dec!(50)) / dec!(5)
        );
    }

    #[test]
    fn test_indicator_ema() {
        let mut calc = IndicatorCalculator::new(100);

        for i in 1..=20 {
            calc.add_close(Decimal::from(i));
        }

        let ema = calc.ema(10);
        assert!(ema.is_some());
        assert!(ema.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_indicator_rsi() {
        let mut calc = IndicatorCalculator::new(100);

        // Upward trend
        for i in 1..=20 {
            calc.add_close(Decimal::from(i * 10));
        }

        let rsi = calc.rsi(14);
        assert!(rsi.is_some());
        let rsi_value = rsi.unwrap();
        assert!(rsi_value > dec!(50)); // Should be high in uptrend
    }

    #[test]
    fn test_indicator_bollinger_bands() {
        let mut calc = IndicatorCalculator::new(100);

        for i in 1..=30 {
            calc.add_close(dec!(100) + Decimal::from(i % 10));
        }

        let bands = calc.bollinger_bands(20, dec!(2));
        assert!(bands.is_some());

        let (upper, middle, lower) = bands.unwrap();
        assert!(upper > middle);
        assert!(middle > lower);
    }

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::OneMinute.duration(), Duration::minutes(1));
        assert_eq!(Timeframe::OneHour.duration(), Duration::hours(1));
        assert_eq!(Timeframe::OneDay.duration(), Duration::days(1));
    }

    #[test]
    #[allow(dead_code)]
    fn test_candle_body_percentage() {
        let token_id = Uuid::new_v4();
        let now = Utc::now();
        let mut candle = Candle::new(token_id, Timeframe::FifteenMinutes, now, dec!(100));

        candle.high = dec!(110);
        candle.low = dec!(90);
        candle.close = dec!(105);

        let body_pct = candle.body_percentage();
        // Body is |105-100| = 5, range is 110-90 = 20, so 5/20 * 100 = 25%
        assert_eq!(body_pct, dec!(25));
    }

    #[test]
    fn test_candle_manager_history() {
        let mut manager = CandleManager::new(5);
        let token_id = Uuid::new_v4();
        let now = Utc::now();

        // Add multiple completed candles
        for i in 0..10 {
            let past_time = now - Duration::minutes(10 - i);
            let candle = Candle::new(token_id, Timeframe::OneMinute, past_time, dec!(100));
            manager.add_to_history(token_id, Timeframe::OneMinute, candle);
        }

        let history = manager.get_history(token_id, Timeframe::OneMinute, None);
        assert_eq!(history.len(), 5); // Should only keep last 5
    }

    #[test]
    fn test_indicator_insufficient_data() {
        let calc = IndicatorCalculator::new(100);

        // Should return None when there's not enough data
        assert!(calc.sma(10).is_none());
        assert!(calc.ema(10).is_none());
        assert!(calc.rsi(14).is_none());
    }
}
