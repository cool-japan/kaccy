//! Enhanced Volume-Synchronized Probability of Informed Trading (VPIN)
//!
//! This module provides advanced order flow toxicity measurement using VPIN,
//! including bulk volume classification and toxicity-based fee adjustments.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

use crate::error::CoreError;
use crate::trading::OrderSide;

/// Classification of trade volume
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeClass {
    /// Small retail trade
    Retail,
    /// Medium-sized trade
    MediumBulk,
    /// Large institutional trade
    LargeBulk,
}

/// Trade with volume classification for VPIN calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedTrade {
    /// Unique trade identifier.
    pub trade_id: Uuid,
    /// Token involved in this trade.
    pub token_id: Uuid,
    /// Buyer- or seller-initiated direction.
    pub side: OrderSide,
    /// Execution price.
    pub price: Decimal,
    /// Trade size.
    pub amount: Decimal,
    /// Time the trade occurred.
    pub timestamp: DateTime<Utc>,
    /// Volume bucket classification for VPIN buckets.
    pub volume_class: VolumeClass,
}

/// Enhanced VPIN metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedVPIN {
    /// Standard VPIN metric
    pub vpin: Decimal,
    /// VPIN for bulk trades only
    pub bulk_vpin: Decimal,
    /// VPIN for retail trades only
    pub retail_vpin: Decimal,
    /// Number of buckets used
    pub bucket_count: usize,
    /// Total volume analyzed
    pub total_volume: Decimal,
    /// Bulk trade percentage
    pub bulk_percentage: Decimal,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl EnhancedVPIN {
    /// Check if flow is toxic (high probability of informed trading)
    pub fn is_toxic(&self) -> bool {
        self.vpin > dec!(0.7) || self.bulk_vpin > dec!(0.75)
    }

    /// Get toxicity level (0-10)
    pub fn toxicity_level(&self) -> u8 {
        // Weight bulk VPIN more heavily as institutional traders are more informed
        let weighted_vpin = (self.vpin * dec!(0.4)) + (self.bulk_vpin * dec!(0.6));
        let score = (weighted_vpin * dec!(10)).round();
        score.to_string().parse::<u8>().unwrap_or(0).min(10)
    }

    /// Calculate suggested fee multiplier based on toxicity (1.0 = normal fees)
    pub fn fee_multiplier(&self) -> Decimal {
        let base = dec!(1.0);
        let toxicity_score = Decimal::from(self.toxicity_level()) / dec!(10);

        // Increase fees up to 3x for toxic flow
        // Formula: 1.0 + (toxicity^2 * 2.0)
        base + (toxicity_score * toxicity_score * dec!(2.0))
    }

    /// Check if bulk flow is significantly more toxic than retail
    pub fn has_informed_bulk_flow(&self) -> bool {
        self.bulk_vpin > self.retail_vpin + dec!(0.2)
    }
}

/// Toxicity-based fee adjustment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToxicityFeeAdjustment {
    /// Base fee rate
    pub base_fee: Decimal,
    /// Toxicity multiplier
    pub toxicity_multiplier: Decimal,
    /// Final adjusted fee
    pub adjusted_fee: Decimal,
    /// VPIN score that triggered adjustment
    pub vpin_score: Decimal,
    /// Reason for adjustment
    pub reason: String,
}

impl ToxicityFeeAdjustment {
    /// Create fee adjustment from VPIN metrics
    pub fn from_vpin(vpin: &EnhancedVPIN, base_fee: Decimal) -> Self {
        let multiplier = vpin.fee_multiplier();
        let adjusted = base_fee * multiplier;

        let reason = if vpin.is_toxic() {
            if vpin.has_informed_bulk_flow() {
                "High informed institutional trading detected".to_string()
            } else {
                "High order flow toxicity detected".to_string()
            }
        } else if multiplier > dec!(1.5) {
            "Elevated toxicity level".to_string()
        } else {
            "Normal market conditions".to_string()
        };

        Self {
            base_fee,
            toxicity_multiplier: multiplier,
            adjusted_fee: adjusted,
            vpin_score: vpin.vpin,
            reason,
        }
    }
}

/// Enhanced VPIN analyzer with bulk volume classification
#[derive(Debug, Clone)]
pub struct VPINAnalyzer {
    token_id: Uuid,
    trades: VecDeque<ClassifiedTrade>,
    max_history: usize,
    /// Threshold for medium bulk trades (as percentage of average)
    medium_bulk_threshold: Decimal,
    /// Threshold for large bulk trades (as percentage of average)
    large_bulk_threshold: Decimal,
}

impl VPINAnalyzer {
    /// Create a new VPIN analyzer
    pub fn new(token_id: Uuid) -> Self {
        Self {
            token_id,
            trades: VecDeque::new(),
            max_history: 5000,
            medium_bulk_threshold: dec!(2.0), // 2x average
            large_bulk_threshold: dec!(5.0),  // 5x average
        }
    }

    /// Create with custom thresholds
    pub fn with_thresholds(
        token_id: Uuid,
        medium_bulk_threshold: Decimal,
        large_bulk_threshold: Decimal,
    ) -> Self {
        Self {
            token_id,
            trades: VecDeque::new(),
            max_history: 5000,
            medium_bulk_threshold,
            large_bulk_threshold,
        }
    }

    /// Add a trade and classify its volume
    pub fn add_trade(
        &mut self,
        trade_id: Uuid,
        token_id: Uuid,
        side: OrderSide,
        price: Decimal,
        amount: Decimal,
        timestamp: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        if token_id != self.token_id {
            return Err(CoreError::Validation(
                "Trade token ID does not match analyzer".to_string(),
            ));
        }

        let volume_class = self.classify_volume(amount);

        let trade = ClassifiedTrade {
            trade_id,
            token_id,
            side,
            price,
            amount,
            timestamp,
            volume_class,
        };

        self.trades.push_back(trade);

        // Trim old trades
        while self.trades.len() > self.max_history {
            self.trades.pop_front();
        }

        Ok(())
    }

    /// Classify trade volume based on recent average
    fn classify_volume(&self, amount: Decimal) -> VolumeClass {
        if self.trades.is_empty() {
            return VolumeClass::Retail;
        }

        // Calculate average trade size from recent trades (last 100 or all if less)
        let sample_size = self.trades.len().min(100);
        let total: Decimal = self
            .trades
            .iter()
            .rev()
            .take(sample_size)
            .map(|t| t.amount)
            .sum();
        let avg = total / Decimal::from(sample_size);

        if amount >= avg * self.large_bulk_threshold {
            VolumeClass::LargeBulk
        } else if amount >= avg * self.medium_bulk_threshold {
            VolumeClass::MediumBulk
        } else {
            VolumeClass::Retail
        }
    }

    /// Calculate enhanced VPIN with bulk classification
    pub fn calculate_vpin(
        &self,
        window_seconds: i64,
        bucket_count: usize,
    ) -> Result<EnhancedVPIN, CoreError> {
        if bucket_count == 0 {
            return Err(CoreError::Validation(
                "Bucket count must be greater than 0".to_string(),
            ));
        }

        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        // Calculate total volume
        let mut total_volume = dec!(0);
        let mut bulk_volume = dec!(0);
        let mut retail_volume = dec!(0);

        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }
            total_volume += trade.amount;

            match trade.volume_class {
                VolumeClass::Retail => retail_volume += trade.amount,
                VolumeClass::MediumBulk | VolumeClass::LargeBulk => bulk_volume += trade.amount,
            }
        }

        if total_volume == dec!(0) {
            return Ok(EnhancedVPIN {
                vpin: dec!(0),
                bulk_vpin: dec!(0),
                retail_vpin: dec!(0),
                bucket_count,
                total_volume: dec!(0),
                bulk_percentage: dec!(0),
                timestamp: now,
            });
        }

        // Calculate standard VPIN (all trades)
        let vpin = self.calculate_vpin_for_filter(
            window_seconds,
            bucket_count,
            Some(|_: &ClassifiedTrade| true),
        )?;

        // Calculate bulk VPIN (only bulk trades)
        let bulk_vpin = self.calculate_vpin_for_filter(
            window_seconds,
            bucket_count,
            Some(|t: &ClassifiedTrade| {
                matches!(
                    t.volume_class,
                    VolumeClass::MediumBulk | VolumeClass::LargeBulk
                )
            }),
        )?;

        // Calculate retail VPIN (only retail trades)
        let retail_vpin = self.calculate_vpin_for_filter(
            window_seconds,
            bucket_count,
            Some(|t: &ClassifiedTrade| matches!(t.volume_class, VolumeClass::Retail)),
        )?;

        let bulk_percentage = if total_volume > dec!(0) {
            bulk_volume / total_volume
        } else {
            dec!(0)
        };

        Ok(EnhancedVPIN {
            vpin,
            bulk_vpin,
            retail_vpin,
            bucket_count,
            total_volume,
            bulk_percentage,
            timestamp: now,
        })
    }

    /// Calculate VPIN with optional trade filter
    fn calculate_vpin_for_filter<F>(
        &self,
        window_seconds: i64,
        bucket_count: usize,
        filter: Option<F>,
    ) -> Result<Decimal, CoreError>
    where
        F: Fn(&ClassifiedTrade) -> bool,
    {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        // Split trades into volume buckets
        let mut buckets: Vec<(Decimal, Decimal)> = vec![(dec!(0), dec!(0)); bucket_count];
        let mut total_volume = dec!(0);

        // Calculate total volume for filtered trades
        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }
            if let Some(ref f) = filter {
                if !f(trade) {
                    continue;
                }
            }
            total_volume += trade.amount;
        }

        if total_volume == dec!(0) {
            return Ok(dec!(0));
        }

        let volume_per_bucket = total_volume / Decimal::from(bucket_count);
        let mut current_bucket = 0;
        let mut bucket_volume = dec!(0);

        // Distribute trades into volume buckets
        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }
            if let Some(ref f) = filter {
                if !f(trade) {
                    continue;
                }
            }

            if current_bucket >= bucket_count {
                break;
            }

            let remaining_in_bucket = volume_per_bucket - bucket_volume;
            let trade_volume = trade.amount.min(remaining_in_bucket);

            match trade.side {
                OrderSide::Buy => buckets[current_bucket].0 += trade_volume,
                OrderSide::Sell => buckets[current_bucket].1 += trade_volume,
            }

            bucket_volume += trade_volume;

            if bucket_volume >= volume_per_bucket {
                current_bucket += 1;
                bucket_volume = dec!(0);
            }
        }

        // Calculate VPIN (average absolute order imbalance across buckets)
        let mut vpin_sum = dec!(0);
        let mut valid_buckets = 0;

        for (buy_vol, sell_vol) in &buckets {
            let bucket_total = buy_vol + sell_vol;
            if bucket_total > dec!(0) {
                let imbalance = (buy_vol - sell_vol).abs() / bucket_total;
                vpin_sum += imbalance;
                valid_buckets += 1;
            }
        }

        let vpin = if valid_buckets > 0 {
            vpin_sum / Decimal::from(valid_buckets)
        } else {
            dec!(0)
        };

        Ok(vpin)
    }

    /// Get volume class distribution
    pub fn get_volume_distribution(&self, window_seconds: i64) -> (Decimal, Decimal, Decimal) {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(window_seconds);

        let mut retail = dec!(0);
        let mut medium = dec!(0);
        let mut large = dec!(0);

        for trade in self.trades.iter().rev() {
            if trade.timestamp < cutoff {
                break;
            }
            match trade.volume_class {
                VolumeClass::Retail => retail += trade.amount,
                VolumeClass::MediumBulk => medium += trade.amount,
                VolumeClass::LargeBulk => large += trade.amount,
            }
        }

        (retail, medium, large)
    }

    /// Get the number of stored trades
    pub fn trade_count(&self) -> usize {
        self.trades.len()
    }

    /// Clear all stored trades
    pub fn clear(&mut self) {
        self.trades.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_classification() {
        let token_id = Uuid::new_v4();
        let mut analyzer = VPINAnalyzer::new(token_id);

        // Add some baseline trades
        for i in 0..10 {
            analyzer
                .add_trade(
                    Uuid::new_v4(),
                    token_id,
                    OrderSide::Buy,
                    dec!(100),
                    dec!(10),
                    Utc::now() - chrono::Duration::seconds(i * 10),
                )
                .unwrap();
        }

        // Add a large bulk trade (should be 5x average = 50)
        analyzer
            .add_trade(
                Uuid::new_v4(),
                token_id,
                OrderSide::Buy,
                dec!(100),
                dec!(60),
                Utc::now(),
            )
            .unwrap();

        let last_trade = analyzer.trades.back().unwrap();
        assert_eq!(last_trade.volume_class, VolumeClass::LargeBulk);
    }

    #[test]
    fn test_enhanced_vpin_calculation() {
        let token_id = Uuid::new_v4();
        let mut analyzer = VPINAnalyzer::new(token_id);

        // Add imbalanced trades
        for i in 0..20 {
            let side = if i < 14 {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };
            let amount = if i < 2 { dec!(50) } else { dec!(10) }; // First 2 are bulk

            analyzer
                .add_trade(
                    Uuid::new_v4(),
                    token_id,
                    side,
                    dec!(100),
                    amount,
                    Utc::now() - chrono::Duration::seconds((20 - i) * 2),
                )
                .unwrap();
        }

        let vpin = analyzer.calculate_vpin(60, 5).unwrap();
        assert!(vpin.vpin >= dec!(0));
        assert!(vpin.bulk_vpin >= dec!(0));
        assert!(vpin.retail_vpin >= dec!(0));
        assert!(vpin.total_volume > dec!(0));
    }

    #[test]
    fn test_toxicity_detection() {
        let vpin = EnhancedVPIN {
            vpin: dec!(0.8),
            bulk_vpin: dec!(0.85),
            retail_vpin: dec!(0.4),
            bucket_count: 5,
            total_volume: dec!(1000),
            bulk_percentage: dec!(0.3),
            timestamp: Utc::now(),
        };

        assert!(vpin.is_toxic());
        assert!(vpin.has_informed_bulk_flow());
        assert!(vpin.toxicity_level() >= 7);
    }

    #[test]
    fn test_fee_multiplier_calculation() {
        let low_toxicity = EnhancedVPIN {
            vpin: dec!(0.2),
            bulk_vpin: dec!(0.15),
            retail_vpin: dec!(0.25),
            bucket_count: 5,
            total_volume: dec!(1000),
            bulk_percentage: dec!(0.2),
            timestamp: Utc::now(),
        };

        let multiplier = low_toxicity.fee_multiplier();
        assert!(multiplier >= dec!(1.0));
        assert!(multiplier <= dec!(1.5));

        let high_toxicity = EnhancedVPIN {
            vpin: dec!(0.9),
            bulk_vpin: dec!(0.95),
            retail_vpin: dec!(0.5),
            bucket_count: 5,
            total_volume: dec!(1000),
            bulk_percentage: dec!(0.4),
            timestamp: Utc::now(),
        };

        let high_multiplier = high_toxicity.fee_multiplier();
        assert!(high_multiplier > multiplier);
        assert!(high_multiplier <= dec!(3.0));
    }

    #[test]
    fn test_toxicity_fee_adjustment() {
        let vpin = EnhancedVPIN {
            vpin: dec!(0.75),
            bulk_vpin: dec!(0.8),
            retail_vpin: dec!(0.3),
            bucket_count: 5,
            total_volume: dec!(1000),
            bulk_percentage: dec!(0.35),
            timestamp: Utc::now(),
        };

        let adjustment = ToxicityFeeAdjustment::from_vpin(&vpin, dec!(0.0025));
        assert!(adjustment.adjusted_fee > adjustment.base_fee);
        assert!(adjustment.toxicity_multiplier >= dec!(1.0));
        assert!(!adjustment.reason.is_empty());
    }

    #[test]
    fn test_volume_distribution() {
        let token_id = Uuid::new_v4();
        let mut analyzer = VPINAnalyzer::new(token_id);

        // Add various sized trades
        for _ in 0..5 {
            analyzer
                .add_trade(
                    Uuid::new_v4(),
                    token_id,
                    OrderSide::Buy,
                    dec!(100),
                    dec!(10),
                    Utc::now(),
                )
                .unwrap();
        }

        // Add medium bulk
        analyzer
            .add_trade(
                Uuid::new_v4(),
                token_id,
                OrderSide::Buy,
                dec!(100),
                dec!(25),
                Utc::now(),
            )
            .unwrap();

        // Add large bulk (needs to be 5x the new average: (50 + 25)/6 = 12.5, so 5x = 62.5)
        // Use 100 to be safe
        analyzer
            .add_trade(
                Uuid::new_v4(),
                token_id,
                OrderSide::Buy,
                dec!(100),
                dec!(100),
                Utc::now(),
            )
            .unwrap();

        let (retail, medium, large) = analyzer.get_volume_distribution(60);
        assert!(retail > dec!(0));
        assert!(medium > dec!(0));
        assert!(large > dec!(0));
    }

    #[test]
    fn test_invalid_token_id() {
        let token_id = Uuid::new_v4();
        let mut analyzer = VPINAnalyzer::new(token_id);

        let result = analyzer.add_trade(
            Uuid::new_v4(),
            Uuid::new_v4(), // Different token ID
            OrderSide::Buy,
            dec!(100),
            dec!(10),
            Utc::now(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_empty_vpin_calculation() {
        let token_id = Uuid::new_v4();
        let analyzer = VPINAnalyzer::new(token_id);

        let vpin = analyzer.calculate_vpin(60, 5).unwrap();
        assert_eq!(vpin.vpin, dec!(0));
        assert_eq!(vpin.total_volume, dec!(0));
    }

    #[test]
    fn test_max_history_limit() {
        let token_id = Uuid::new_v4();
        let mut analyzer = VPINAnalyzer::new(token_id);
        analyzer.max_history = 10;

        // Add 20 trades
        for _ in 0..20 {
            analyzer
                .add_trade(
                    Uuid::new_v4(),
                    token_id,
                    OrderSide::Buy,
                    dec!(100),
                    dec!(10),
                    Utc::now(),
                )
                .unwrap();
        }

        assert_eq!(analyzer.trade_count(), 10);
    }
}
