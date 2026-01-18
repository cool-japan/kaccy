//! Market data analytics (OHLCV, VWAP, TWAP, etc.)

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;
use uuid::Uuid;

/// OHLCV candlestick data
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Candle {
    pub candle_id: Uuid,
    pub token_id: Uuid,
    /// Candle interval (1m, 5m, 15m, 1h, 4h, 1d, etc.)
    pub interval: CandleInterval,
    /// Opening price
    pub open: Decimal,
    /// Highest price
    pub high: Decimal,
    /// Lowest price
    pub low: Decimal,
    /// Closing price
    pub close: Decimal,
    /// Volume traded
    pub volume: Decimal,
    /// Number of trades
    pub trade_count: i32,
    /// Candle start time
    pub start_time: DateTime<Utc>,
    /// Candle end time
    pub end_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum CandleInterval {
    /// 1 minute
    M1,
    /// 5 minutes
    #[default]
    M5,
    /// 15 minutes
    M15,
    /// 30 minutes
    M30,
    /// 1 hour
    H1,
    /// 4 hours
    H4,
    /// 1 day
    D1,
    /// 1 week
    W1,
}

impl fmt::Display for CandleInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandleInterval::M1 => write!(f, "1m"),
            CandleInterval::M5 => write!(f, "5m"),
            CandleInterval::M15 => write!(f, "15m"),
            CandleInterval::M30 => write!(f, "30m"),
            CandleInterval::H1 => write!(f, "1h"),
            CandleInterval::H4 => write!(f, "4h"),
            CandleInterval::D1 => write!(f, "1d"),
            CandleInterval::W1 => write!(f, "1w"),
        }
    }
}

impl CandleInterval {
    /// Get duration in seconds
    pub fn duration_seconds(&self) -> i64 {
        match self {
            CandleInterval::M1 => 60,
            CandleInterval::M5 => 300,
            CandleInterval::M15 => 900,
            CandleInterval::M30 => 1800,
            CandleInterval::H1 => 3600,
            CandleInterval::H4 => 14400,
            CandleInterval::D1 => 86400,
            CandleInterval::W1 => 604800,
        }
    }
}

impl Candle {
    /// Create a new candle
    pub fn new(
        token_id: Uuid,
        interval: CandleInterval,
        start_time: DateTime<Utc>,
        open_price: Decimal,
    ) -> Self {
        let duration = Duration::seconds(interval.duration_seconds());
        let end_time = start_time + duration;

        Self {
            candle_id: Uuid::new_v4(),
            token_id,
            interval,
            open: open_price,
            high: open_price,
            low: open_price,
            close: open_price,
            volume: dec!(0),
            trade_count: 0,
            start_time,
            end_time,
            created_at: Utc::now(),
        }
    }

    /// Update candle with a new trade
    pub fn update_with_trade(&mut self, price: Decimal, volume: Decimal) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += volume;
        self.trade_count += 1;
    }

    /// Check if candle is complete (past end time)
    pub fn is_complete(&self) -> bool {
        Utc::now() >= self.end_time
    }

    /// Calculate price change
    pub fn price_change(&self) -> Decimal {
        self.close - self.open
    }

    /// Calculate price change percentage
    pub fn price_change_percentage(&self) -> Decimal {
        if self.open == dec!(0) {
            return dec!(0);
        }
        ((self.close - self.open) / self.open) * dec!(100)
    }

    /// Calculate typical price (H+L+C)/3
    pub fn typical_price(&self) -> Decimal {
        (self.high + self.low + self.close) / dec!(3)
    }

    /// Calculate weighted price (OHLC/4)
    pub fn weighted_price(&self) -> Decimal {
        (self.open + self.high + self.low + self.close) / dec!(4)
    }
}

impl fmt::Display for Candle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Candle({}, O:{} H:{} L:{} C:{} V:{})",
            self.interval, self.open, self.high, self.low, self.close, self.volume
        )
    }
}

/// Volume-Weighted Average Price (VWAP) calculation
#[derive(Debug, Clone)]
pub struct VwapCalculator {
    _token_id: Uuid,
    cumulative_pv: Decimal, // Price * Volume
    cumulative_volume: Decimal,
    _start_time: DateTime<Utc>,
}

impl VwapCalculator {
    pub fn new(token_id: Uuid) -> Self {
        Self {
            _token_id: token_id,
            cumulative_pv: dec!(0),
            cumulative_volume: dec!(0),
            _start_time: Utc::now(),
        }
    }

    /// Add a trade to VWAP calculation
    pub fn add_trade(&mut self, price: Decimal, volume: Decimal) {
        self.cumulative_pv += price * volume;
        self.cumulative_volume += volume;
    }

    /// Get current VWAP
    pub fn vwap(&self) -> Decimal {
        if self.cumulative_volume == dec!(0) {
            return dec!(0);
        }
        self.cumulative_pv / self.cumulative_volume
    }

    /// Reset VWAP calculation
    pub fn reset(&mut self) {
        self.cumulative_pv = dec!(0);
        self.cumulative_volume = dec!(0);
        self._start_time = Utc::now();
    }
}

/// Time-Weighted Average Price (TWAP) calculation
#[derive(Debug, Clone)]
pub struct TwapCalculator {
    _token_id: Uuid,
    samples: Vec<(DateTime<Utc>, Decimal)>,
    max_samples: usize,
}

impl TwapCalculator {
    pub fn new(token_id: Uuid, max_samples: usize) -> Self {
        Self {
            _token_id: token_id,
            samples: Vec::new(),
            max_samples,
        }
    }

    /// Add a price sample
    pub fn add_sample(&mut self, price: Decimal) {
        self.samples.push((Utc::now(), price));

        // Keep only recent samples
        if self.samples.len() > self.max_samples {
            self.samples.remove(0);
        }
    }

    /// Calculate TWAP
    pub fn twap(&self) -> Decimal {
        if self.samples.is_empty() {
            return dec!(0);
        }

        let total: Decimal = self.samples.iter().map(|(_, price)| price).sum();
        total / Decimal::from(self.samples.len())
    }

    /// Calculate TWAP over specific time window
    pub fn twap_window(&self, window_seconds: i64) -> Decimal {
        let cutoff = Utc::now() - Duration::seconds(window_seconds);

        let recent_samples: Vec<&Decimal> = self
            .samples
            .iter()
            .filter(|(time, _)| *time > cutoff)
            .map(|(_, price)| price)
            .collect();

        if recent_samples.is_empty() {
            return dec!(0);
        }

        let total: Decimal = recent_samples.iter().copied().sum();
        total / Decimal::from(recent_samples.len())
    }
}

/// Market depth snapshot
#[derive(Debug, Clone, Serialize)]
pub struct MarketDepth {
    pub token_id: Uuid,
    /// Buy orders (price, quantity)
    pub bids: Vec<(Decimal, Decimal)>,
    /// Sell orders (price, quantity)
    pub asks: Vec<(Decimal, Decimal)>,
    /// Best bid price
    pub best_bid: Option<Decimal>,
    /// Best ask price
    pub best_ask: Option<Decimal>,
    /// Spread (ask - bid)
    pub spread: Option<Decimal>,
    /// Spread percentage
    pub spread_percentage: Option<Decimal>,
    pub snapshot_time: DateTime<Utc>,
}

impl MarketDepth {
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            bids: Vec::new(),
            asks: Vec::new(),
            best_bid: None,
            best_ask: None,
            spread: None,
            spread_percentage: None,
            snapshot_time: Utc::now(),
        }
    }

    /// Calculate mid price (average of best bid and ask)
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid, self.best_ask) {
            (Some(bid), Some(ask)) => Some((bid + ask) / dec!(2)),
            _ => None,
        }
    }

    /// Calculate total bid volume
    pub fn total_bid_volume(&self) -> Decimal {
        self.bids.iter().map(|(_, qty)| qty).sum()
    }

    /// Calculate total ask volume
    pub fn total_ask_volume(&self) -> Decimal {
        self.asks.iter().map(|(_, qty)| qty).sum()
    }

    /// Calculate imbalance ratio (bid volume / total volume)
    pub fn imbalance_ratio(&self) -> Decimal {
        let bid_vol = self.total_bid_volume();
        let ask_vol = self.total_ask_volume();
        let total_vol = bid_vol + ask_vol;

        if total_vol == dec!(0) {
            return dec!(0.5); // Neutral
        }

        bid_vol / total_vol
    }
}

/// Trading statistics for a token
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TradingStats {
    pub token_id: Uuid,
    /// 24h trading volume
    pub volume_24h: Decimal,
    /// 24h price change
    pub price_change_24h: Decimal,
    /// 24h price change percentage
    pub price_change_24h_pct: Decimal,
    /// 24h high price
    pub high_24h: Decimal,
    /// 24h low price
    pub low_24h: Decimal,
    /// 24h number of trades
    pub trades_24h: i32,
    /// All-time high price
    pub ath_price: Option<Decimal>,
    /// All-time high time
    pub ath_time: Option<DateTime<Utc>>,
    /// All-time low price
    pub atl_price: Option<Decimal>,
    /// All-time low time
    pub atl_time: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl TradingStats {
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            volume_24h: dec!(0),
            price_change_24h: dec!(0),
            price_change_24h_pct: dec!(0),
            high_24h: dec!(0),
            low_24h: Decimal::MAX,
            trades_24h: 0,
            ath_price: None,
            ath_time: None,
            atl_price: None,
            atl_time: None,
            updated_at: Utc::now(),
        }
    }

    /// Update stats with new trade
    pub fn update_with_trade(&mut self, price: Decimal, volume: Decimal) {
        self.volume_24h += volume;
        self.high_24h = self.high_24h.max(price);
        self.low_24h = self.low_24h.min(price);
        self.trades_24h += 1;

        // Update ATH
        if self.ath_price.is_none() || price > self.ath_price.unwrap() {
            self.ath_price = Some(price);
            self.ath_time = Some(Utc::now());
        }

        // Update ATL
        if self.atl_price.is_none() || price < self.atl_price.unwrap() {
            self.atl_price = Some(price);
            self.atl_time = Some(Utc::now());
        }

        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_creation_and_update() {
        let token_id = Uuid::new_v4();
        let start_time = Utc::now();
        let mut candle = Candle::new(token_id, CandleInterval::M5, start_time, dec!(100));

        assert_eq!(candle.open, dec!(100));
        assert_eq!(candle.high, dec!(100));
        assert_eq!(candle.low, dec!(100));
        assert_eq!(candle.close, dec!(100));

        // Update with higher price
        candle.update_with_trade(dec!(110), dec!(10));
        assert_eq!(candle.high, dec!(110));
        assert_eq!(candle.close, dec!(110));
        assert_eq!(candle.volume, dec!(10));

        // Update with lower price
        candle.update_with_trade(dec!(95), dec!(5));
        assert_eq!(candle.low, dec!(95));
        assert_eq!(candle.close, dec!(95));
        assert_eq!(candle.volume, dec!(15));
    }

    #[test]
    fn test_candle_price_change() {
        let token_id = Uuid::new_v4();
        let start_time = Utc::now();
        let mut candle = Candle::new(token_id, CandleInterval::H1, start_time, dec!(100));

        candle.update_with_trade(dec!(120), dec!(10));

        assert_eq!(candle.price_change(), dec!(20));
        assert_eq!(candle.price_change_percentage(), dec!(20));
    }

    #[test]
    fn test_vwap_calculation() {
        let token_id = Uuid::new_v4();
        let mut vwap = VwapCalculator::new(token_id);

        // Trade 1: 10 units at $100 = $1000
        vwap.add_trade(dec!(100), dec!(10));

        // Trade 2: 20 units at $110 = $2200
        vwap.add_trade(dec!(110), dec!(20));

        // VWAP = (1000 + 2200) / (10 + 20) = 3200 / 30 = 106.666...
        let result = vwap.vwap();
        assert!(result > dec!(106.66) && result < dec!(106.67));
    }

    #[test]
    fn test_twap_calculation() {
        let token_id = Uuid::new_v4();
        let mut twap = TwapCalculator::new(token_id, 100);

        twap.add_sample(dec!(100));
        twap.add_sample(dec!(110));
        twap.add_sample(dec!(105));

        // TWAP = (100 + 110 + 105) / 3 = 105
        assert_eq!(twap.twap(), dec!(105));
    }

    #[test]
    fn test_market_depth() {
        let token_id = Uuid::new_v4();
        let mut depth = MarketDepth::new(token_id);

        depth.bids = vec![(dec!(99), dec!(100)), (dec!(98), dec!(200))];
        depth.asks = vec![(dec!(101), dec!(150)), (dec!(102), dec!(250))];
        depth.best_bid = Some(dec!(99));
        depth.best_ask = Some(dec!(101));

        assert_eq!(depth.mid_price(), Some(dec!(100)));
        assert_eq!(depth.total_bid_volume(), dec!(300));
        assert_eq!(depth.total_ask_volume(), dec!(400));

        // Imbalance = 300 / 700 = 0.428...
        let imbalance = depth.imbalance_ratio();
        assert!(imbalance > dec!(0.42) && imbalance < dec!(0.43));
    }

    #[test]
    fn test_trading_stats() {
        let token_id = Uuid::new_v4();
        let mut stats = TradingStats::new(token_id);

        stats.update_with_trade(dec!(100), dec!(10));
        stats.update_with_trade(dec!(110), dec!(20));
        stats.update_with_trade(dec!(90), dec!(15));

        assert_eq!(stats.volume_24h, dec!(45));
        assert_eq!(stats.high_24h, dec!(110));
        assert_eq!(stats.low_24h, dec!(90));
        assert_eq!(stats.trades_24h, 3);
        assert_eq!(stats.ath_price, Some(dec!(110)));
        assert_eq!(stats.atl_price, Some(dec!(90)));
    }
}
