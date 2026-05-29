//! Volume Pattern Analysis Module
//!
//! This module provides comprehensive volume pattern analysis including:
//! - Volume profile analysis
//! - Accumulation/distribution detection
//! - Volume divergence analysis
//! - Wyckoff method implementation

use crate::CoreError;
use crate::ml::features::PricePoint;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Volume profile representing volume distribution at different price levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeProfile {
    /// Price levels and their corresponding volumes
    pub levels: HashMap<String, Decimal>, // String key for Decimal price
    /// Point of control (price level with highest volume)
    pub poc: Decimal,
    /// Value area high (top of 70% volume area)
    pub vah: Decimal,
    /// Value area low (bottom of 70% volume area)
    pub val: Decimal,
    /// Total volume in the profile
    pub total_volume: Decimal,
}

/// Volume profile analyzer
#[derive(Debug, Clone)]
pub struct VolumeProfileAnalyzer {
    /// Number of price bins for the profile
    num_bins: usize,
}

impl VolumeProfileAnalyzer {
    /// Create a new volume profile analyzer
    pub fn new(num_bins: usize) -> Self {
        Self { num_bins }
    }

    /// Analyze volume profile from price data
    pub fn analyze(&self, data: &[PricePoint]) -> anyhow::Result<VolumeProfile> {
        if data.is_empty() {
            return Err(CoreError::Validation("Empty data".to_string()).into());
        }

        // Find price range
        let min_price = data
            .iter()
            .map(|p| p.low.min(p.high).min(p.close))
            .min()
            .unwrap_or(dec!(0));
        let max_price = data
            .iter()
            .map(|p| p.high.max(p.low).max(p.close))
            .max()
            .unwrap_or(dec!(0));

        if max_price <= min_price {
            return Err(CoreError::Validation("Invalid price range".to_string()).into());
        }

        let price_step = (max_price - min_price) / Decimal::from(self.num_bins);

        // Build volume profile
        let mut levels: HashMap<String, Decimal> = HashMap::new();
        let mut total_volume = dec!(0);

        for point in data {
            // Distribute volume across price range (high to low)
            let num_steps = ((point.high - point.low) / price_step)
                .ceil()
                .to_usize()
                .unwrap_or(1)
                .max(1);
            let volume_per_step = point.volume / Decimal::from(num_steps);

            let mut current_price = point.low;
            while current_price <= point.high {
                let bin_price = (((current_price - min_price) / price_step)
                    .floor()
                    .to_u64()
                    .unwrap_or(0)
                    .min(self.num_bins.saturating_sub(1) as u64)
                    * price_step.to_u64().unwrap_or(1))
                    + min_price.to_u64().unwrap_or(0);
                let bin_price_decimal = Decimal::from(bin_price);
                let key = bin_price_decimal.to_string();

                *levels.entry(key).or_insert(dec!(0)) += volume_per_step;
                total_volume += volume_per_step;
                current_price += price_step;
            }
        }

        // Find POC (Point of Control - highest volume level)
        let poc = levels
            .iter()
            .max_by_key(|(_, vol)| *vol)
            .map(|(price_str, _)| price_str.parse::<Decimal>().unwrap_or(dec!(0)))
            .unwrap_or(dec!(0));

        // Calculate value area (70% of volume)
        let value_area_volume = total_volume * dec!(0.70);
        let (vah, val) = self.calculate_value_area(&levels, poc, value_area_volume);

        Ok(VolumeProfile {
            levels,
            poc,
            vah,
            val,
            total_volume,
        })
    }

    /// Calculate value area high and low
    fn calculate_value_area(
        &self,
        levels: &HashMap<String, Decimal>,
        poc: Decimal,
        target_volume: Decimal,
    ) -> (Decimal, Decimal) {
        let mut sorted_levels: Vec<_> = levels.iter().collect();
        sorted_levels.sort_by_key(|(price_str, _)| price_str.parse::<Decimal>().unwrap_or(dec!(0)));

        let poc_idx = sorted_levels
            .iter()
            .position(|(price_str, _)| price_str.parse::<Decimal>().unwrap_or(dec!(0)) == poc)
            .unwrap_or(0);

        let mut accumulated_volume = *sorted_levels[poc_idx].1;
        let mut low_idx = poc_idx;
        let mut high_idx = poc_idx;

        while accumulated_volume < target_volume {
            let add_below = low_idx > 0
                && (high_idx >= sorted_levels.len() - 1
                    || sorted_levels[low_idx - 1].1 >= sorted_levels[high_idx + 1].1);

            if add_below {
                low_idx -= 1;
                accumulated_volume += sorted_levels[low_idx].1;
            } else if high_idx < sorted_levels.len() - 1 {
                high_idx += 1;
                accumulated_volume += sorted_levels[high_idx].1;
            } else {
                break;
            }
        }

        let val = sorted_levels[low_idx]
            .0
            .parse::<Decimal>()
            .unwrap_or(dec!(0));
        let vah = sorted_levels[high_idx]
            .0
            .parse::<Decimal>()
            .unwrap_or(dec!(0));

        (vah, val)
    }
}

/// Accumulation/Distribution indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccumulationDistribution {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// A/D value
    pub value: f64,
    /// Signal: Accumulation, Distribution, or Neutral
    pub signal: ADSignal,
}

/// Accumulation/Distribution signal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ADSignal {
    /// Strong accumulation
    Accumulation,
    /// Strong distribution
    Distribution,
    /// Neutral
    Neutral,
}

/// Accumulation/Distribution analyzer
#[derive(Debug, Clone)]
pub struct AccumulationDistributionAnalyzer {
    /// Threshold for strong signals
    threshold: f64,
}

impl Default for AccumulationDistributionAnalyzer {
    fn default() -> Self {
        Self { threshold: 0.01 }
    }
}

impl AccumulationDistributionAnalyzer {
    /// Create a new A/D analyzer
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }

    /// Calculate A/D line
    pub fn analyze(&self, data: &[PricePoint]) -> anyhow::Result<Vec<AccumulationDistribution>> {
        if data.len() < 2 {
            return Err(CoreError::Validation("Need at least 2 data points".to_string()).into());
        }

        let mut results: Vec<AccumulationDistribution> = Vec::new();
        let mut ad_line = 0.0;

        for point in data {
            // Money Flow Multiplier = [(Close - Low) - (High - Close)] / (High - Low)
            let high_f = point.high.to_f64().unwrap_or(0.0);
            let low_f = point.low.to_f64().unwrap_or(0.0);
            let close_f = point.close.to_f64().unwrap_or(0.0);
            let volume_f = point.volume.to_f64().unwrap_or(0.0);

            let range = high_f - low_f;
            let mfm = if range > 0.0 {
                ((close_f - low_f) - (high_f - close_f)) / range
            } else {
                0.0
            };

            // Money Flow Volume = MFM * Volume
            let mfv = mfm * volume_f;

            // AD Line = Previous AD + MFV
            ad_line += mfv;

            // Determine signal
            let signal = if !results.is_empty() {
                let prev_ad = results
                    .last()
                    .expect("results non-empty per !results.is_empty() check above")
                    .value;
                let change_rate = (ad_line - prev_ad) / prev_ad.abs().max(1.0);

                if change_rate > self.threshold {
                    ADSignal::Accumulation
                } else if change_rate < -self.threshold {
                    ADSignal::Distribution
                } else {
                    ADSignal::Neutral
                }
            } else {
                ADSignal::Neutral
            };

            results.push(AccumulationDistribution {
                timestamp: point.timestamp,
                value: ad_line,
                signal,
            });
        }

        Ok(results)
    }
}

/// Volume divergence detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeDivergence {
    /// Start timestamp
    pub start: DateTime<Utc>,
    /// End timestamp
    pub end: DateTime<Utc>,
    /// Divergence type
    pub divergence_type: DivergenceType,
    /// Strength (0.0 to 1.0)
    pub strength: f64,
}

/// Type of divergence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DivergenceType {
    /// Price rising, volume falling (bearish)
    BearishDivergence,
    /// Price falling, volume rising (bullish)
    BullishDivergence,
}

/// Volume divergence detector
#[derive(Debug, Clone)]
pub struct VolumeDivergenceDetector {
    /// Window size for divergence detection
    window_size: usize,
    /// Minimum strength threshold
    min_strength: f64,
}

impl Default for VolumeDivergenceDetector {
    fn default() -> Self {
        Self {
            window_size: 14,
            min_strength: 0.5,
        }
    }
}

impl VolumeDivergenceDetector {
    /// Create a new divergence detector
    pub fn new(window_size: usize, min_strength: f64) -> Self {
        Self {
            window_size,
            min_strength,
        }
    }

    /// Detect divergences in price/volume data
    pub fn detect(&self, data: &[PricePoint]) -> anyhow::Result<Vec<VolumeDivergence>> {
        if data.len() < self.window_size {
            return Ok(Vec::new());
        }

        let mut divergences = Vec::new();

        for i in self.window_size..data.len() {
            let window = &data[i - self.window_size..i];

            // Calculate price trend
            let price_start = window
                .first()
                .expect("window length >= window_size > 0")
                .close
                .to_f64()
                .unwrap_or(0.0);
            let price_end = window
                .last()
                .expect("window length >= window_size > 0")
                .close
                .to_f64()
                .unwrap_or(0.0);
            let price_change = (price_end - price_start) / price_start.max(0.0001);

            // Calculate volume trend
            let vol_first_half: f64 = window[..self.window_size / 2]
                .iter()
                .map(|p| p.volume.to_f64().unwrap_or(0.0))
                .sum();
            let vol_second_half: f64 = window[self.window_size / 2..]
                .iter()
                .map(|p| p.volume.to_f64().unwrap_or(0.0))
                .sum();

            let avg_vol_first = vol_first_half / (self.window_size / 2) as f64;
            let avg_vol_second = vol_second_half / (self.window_size / 2) as f64;
            let vol_change = (avg_vol_second - avg_vol_first) / avg_vol_first.max(0.0001);

            // Detect divergence
            let (divergence_type, strength) = if price_change > 0.02 && vol_change < -0.1 {
                // Price up, volume down - bearish divergence
                (
                    Some(DivergenceType::BearishDivergence),
                    (price_change - vol_change).abs().min(1.0),
                )
            } else if price_change < -0.02 && vol_change > 0.1 {
                // Price down, volume up - bullish divergence
                (
                    Some(DivergenceType::BullishDivergence),
                    (price_change.abs() + vol_change).min(1.0),
                )
            } else {
                (None, 0.0)
            };

            if let Some(div_type) = divergence_type {
                if strength >= self.min_strength {
                    divergences.push(VolumeDivergence {
                        start: window
                            .first()
                            .expect("window length >= window_size > 0")
                            .timestamp,
                        end: window
                            .last()
                            .expect("window length >= window_size > 0")
                            .timestamp,
                        divergence_type: div_type,
                        strength,
                    });
                }
            }
        }

        Ok(divergences)
    }
}

/// Wyckoff market phase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WyckoffPhase {
    /// Accumulation phase
    Accumulation,
    /// Markup phase
    Markup,
    /// Distribution phase
    Distribution,
    /// Markdown phase
    Markdown,
    /// Unknown/transitioning
    Unknown,
}

/// Wyckoff analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WyckoffAnalysis {
    /// Current phase
    pub phase: WyckoffPhase,
    /// Confidence (0.0 to 1.0)
    pub confidence: f64,
    /// Supporting evidence
    pub evidence: Vec<String>,
}

/// Wyckoff method analyzer
#[derive(Debug, Clone)]
pub struct WyckoffAnalyzer {
    /// Analysis window size
    window_size: usize,
}

impl Default for WyckoffAnalyzer {
    fn default() -> Self {
        Self { window_size: 30 }
    }
}

impl WyckoffAnalyzer {
    /// Create a new Wyckoff analyzer
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }

    /// Analyze market phase using Wyckoff method
    pub fn analyze(&self, data: &[PricePoint]) -> anyhow::Result<WyckoffAnalysis> {
        if data.len() < self.window_size {
            return Ok(WyckoffAnalysis {
                phase: WyckoffPhase::Unknown,
                confidence: 0.0,
                evidence: vec!["Insufficient data".to_string()],
            });
        }

        let window = &data[data.len() - self.window_size..];

        // Calculate price trend
        let prices: Vec<f64> = window
            .iter()
            .map(|p| p.close.to_f64().unwrap_or(0.0))
            .collect();
        let price_start = prices[0];
        let price_end = *prices
            .last()
            .expect("prices has window_size elements, which is > 0");
        let price_trend = (price_end - price_start) / price_start.max(0.0001);

        // Calculate volume trend
        let volumes: Vec<f64> = window
            .iter()
            .map(|p| p.volume.to_f64().unwrap_or(0.0))
            .collect();
        let vol_avg_first =
            volumes[..self.window_size / 2].iter().sum::<f64>() / (self.window_size / 2) as f64;
        let vol_avg_second =
            volumes[self.window_size / 2..].iter().sum::<f64>() / (self.window_size / 2) as f64;
        let vol_trend = (vol_avg_second - vol_avg_first) / vol_avg_first.max(0.0001);

        // Calculate price volatility
        let price_range = prices
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .expect("prices has window_size elements, which is > 0")
            - prices
                .iter()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .expect("prices has window_size elements, which is > 0");
        let volatility = price_range / price_start.max(0.0001);

        // Determine phase based on Wyckoff principles
        let mut evidence = Vec::new();
        let (phase, confidence) = if price_trend.abs() < 0.05 && volatility < 0.1 {
            // Low price movement, low volatility
            if vol_trend > 0.2 {
                evidence.push("High volume in range".to_string());
                evidence.push("Price consolidation".to_string());
                (WyckoffPhase::Accumulation, 0.7)
            } else {
                evidence.push("Low volume in range".to_string());
                evidence.push("Price consolidation".to_string());
                (WyckoffPhase::Distribution, 0.6)
            }
        } else if price_trend > 0.1 {
            // Strong upward price movement
            if vol_trend > 0.0 {
                evidence.push("Rising price with volume".to_string());
                evidence.push("Strong upward momentum".to_string());
                (WyckoffPhase::Markup, 0.8)
            } else {
                evidence.push("Rising price, declining volume".to_string());
                evidence.push("Potential exhaustion".to_string());
                (WyckoffPhase::Distribution, 0.7)
            }
        } else if price_trend < -0.1 {
            // Strong downward price movement
            if vol_trend > 0.0 {
                evidence.push("Falling price with volume".to_string());
                evidence.push("Strong downward momentum".to_string());
                (WyckoffPhase::Markdown, 0.8)
            } else {
                evidence.push("Falling price, declining volume".to_string());
                evidence.push("Potential bottom forming".to_string());
                (WyckoffPhase::Accumulation, 0.6)
            }
        } else {
            evidence.push("Mixed signals".to_string());
            (WyckoffPhase::Unknown, 0.3)
        };

        Ok(WyckoffAnalysis {
            phase,
            confidence,
            evidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_data() -> Vec<PricePoint> {
        vec![
            PricePoint {
                timestamp: Utc::now(),
                open: dec!(100),
                high: dec!(110),
                low: dec!(95),
                close: dec!(105),
                volume: dec!(1000),
            },
            PricePoint {
                timestamp: Utc::now(),
                open: dec!(105),
                high: dec!(115),
                low: dec!(100),
                close: dec!(110),
                volume: dec!(1200),
            },
            PricePoint {
                timestamp: Utc::now(),
                open: dec!(110),
                high: dec!(120),
                low: dec!(105),
                close: dec!(115),
                volume: dec!(1500),
            },
        ]
    }

    #[test]
    fn test_volume_profile_analyzer() {
        let data = create_test_data();
        let analyzer = VolumeProfileAnalyzer::new(10);
        let profile = analyzer.analyze(&data).unwrap();

        assert!(profile.total_volume > dec!(0));
        assert!(profile.poc >= dec!(95));
        assert!(profile.poc <= dec!(120));
        assert!(profile.vah >= profile.val);
    }

    #[test]
    fn test_accumulation_distribution() {
        let data = create_test_data();
        let analyzer = AccumulationDistributionAnalyzer::default();
        let results = analyzer.analyze(&data).unwrap();

        assert_eq!(results.len(), data.len());
        assert!(results.iter().all(|r| r.value.is_finite()));
    }

    #[test]
    fn test_volume_divergence_detector() {
        let mut data = Vec::new();
        for i in 0..20 {
            let base_price = 100.0 + i as f64 * 2.0; // Rising price
            let volume = 1000.0 - i as f64 * 30.0; // Falling volume

            data.push(PricePoint {
                timestamp: Utc::now(),
                open: Decimal::from_f64_retain(base_price).unwrap(),
                high: Decimal::from_f64_retain(base_price + 2.0).unwrap(),
                low: Decimal::from_f64_retain(base_price - 2.0).unwrap(),
                close: Decimal::from_f64_retain(base_price + 1.0).unwrap(),
                volume: Decimal::from_f64_retain(volume.max(100.0)).unwrap(),
            });
        }

        let detector = VolumeDivergenceDetector::default();
        let divergences = detector.detect(&data).unwrap();

        assert!(!divergences.is_empty());
        assert!(
            divergences
                .iter()
                .any(|d| d.divergence_type == DivergenceType::BearishDivergence)
        );
    }

    #[test]
    fn test_wyckoff_analyzer() {
        let mut data = Vec::new();
        for i in 0..40 {
            let base_price = 100.0;
            let volume = if i < 20 { 1000.0 } else { 1500.0 }; // Increasing volume

            data.push(PricePoint {
                timestamp: Utc::now(),
                open: Decimal::from_f64_retain(base_price).unwrap(),
                high: Decimal::from_f64_retain(base_price + 2.0).unwrap(),
                low: Decimal::from_f64_retain(base_price - 2.0).unwrap(),
                close: Decimal::from_f64_retain(base_price + 1.0).unwrap(),
                volume: Decimal::from_f64_retain(volume).unwrap(),
            });
        }

        let analyzer = WyckoffAnalyzer::default();
        let analysis = analyzer.analyze(&data).unwrap();

        assert!(analysis.confidence >= 0.0 && analysis.confidence <= 1.0);
        assert!(!analysis.evidence.is_empty());
    }
}
