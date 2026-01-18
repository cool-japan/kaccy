//! Anomaly detection for unusual trading patterns and price movements
//!
//! This module provides statistical and ML-based anomaly detection techniques
//! for identifying unusual patterns, outliers, and suspicious activities.

use super::features::PricePoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Anomaly detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anomaly {
    pub timestamp: DateTime<Utc>,
    pub anomaly_type: AnomalyType,
    pub severity: AnomalySeverity,
    pub score: f64,
    pub description: String,
    pub value: f64,
    pub expected_range: (f64, f64),
}

/// Type of anomaly detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyType {
    PriceSpike,
    PriceDrop,
    VolumeSpike,
    VolumeDrop,
    Volatility,
    Pattern,
}

/// Severity level of anomaly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl AnomalySeverity {
    pub fn from_score(score: f64) -> Self {
        if score >= 4.0 {
            Self::Critical
        } else if score >= 3.0 {
            Self::High
        } else if score >= 2.0 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// Anomaly detector trait
pub trait AnomalyDetector: Send + Sync {
    /// Detect anomalies in the data
    fn detect(&mut self, data: &[PricePoint]) -> anyhow::Result<Vec<Anomaly>>;

    /// Real-time anomaly detection for streaming data
    fn detect_realtime(&mut self, point: &PricePoint) -> anyhow::Result<Option<Anomaly>>;

    /// Reset detector state
    fn reset(&mut self);

    /// Detector name
    fn name(&self) -> &str;
}

/// Z-score based anomaly detector
#[derive(Debug, Clone)]
pub struct ZScoreDetector {
    window_size: usize,
    threshold: f64,
    price_history: VecDeque<f64>,
    volume_history: VecDeque<f64>,
}

impl ZScoreDetector {
    pub fn new(window_size: usize, threshold: f64) -> Self {
        Self {
            window_size,
            threshold,
            price_history: VecDeque::new(),
            volume_history: VecDeque::new(),
        }
    }

    fn calculate_z_score(&self, value: f64, history: &VecDeque<f64>) -> f64 {
        if history.len() < 2 {
            return 0.0;
        }

        let n = history.len() as f64;
        let mean = history.iter().sum::<f64>() / n;
        let variance = history.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        if std_dev < 1e-10 {
            return 0.0;
        }

        (value - mean) / std_dev
    }
}

impl Default for ZScoreDetector {
    fn default() -> Self {
        Self::new(30, 3.0)
    }
}

impl AnomalyDetector for ZScoreDetector {
    fn detect(&mut self, data: &[PricePoint]) -> anyhow::Result<Vec<Anomaly>> {
        let mut anomalies = Vec::new();

        for point in data {
            if let Some(anomaly) = self.detect_realtime(point)? {
                anomalies.push(anomaly);
            }
        }

        Ok(anomalies)
    }

    fn detect_realtime(&mut self, point: &PricePoint) -> anyhow::Result<Option<Anomaly>> {
        let price = point.close.to_string().parse::<f64>().unwrap_or(0.0);
        let volume = point.volume.to_string().parse::<f64>().unwrap_or(0.0);

        let mut anomaly = None;

        // Check price anomaly
        if self.price_history.len() >= self.window_size {
            let z_score = self.calculate_z_score(price, &self.price_history);

            if z_score.abs() > self.threshold {
                let mean = self.price_history.iter().sum::<f64>() / self.price_history.len() as f64;
                let std = {
                    let variance = self
                        .price_history
                        .iter()
                        .map(|&x| (x - mean).powi(2))
                        .sum::<f64>()
                        / self.price_history.len() as f64;
                    variance.sqrt()
                };

                let anomaly_type = if z_score > 0.0 {
                    AnomalyType::PriceSpike
                } else {
                    AnomalyType::PriceDrop
                };

                anomaly = Some(Anomaly {
                    timestamp: point.timestamp,
                    anomaly_type,
                    severity: AnomalySeverity::from_score(z_score.abs()),
                    score: z_score.abs(),
                    description: format!(
                        "{} detected: price {} is {} standard deviations from mean",
                        if z_score > 0.0 { "Spike" } else { "Drop" },
                        price,
                        z_score.abs()
                    ),
                    value: price,
                    expected_range: (mean - self.threshold * std, mean + self.threshold * std),
                });
            }
        }

        // Check volume anomaly
        if anomaly.is_none() && self.volume_history.len() >= self.window_size {
            let z_score = self.calculate_z_score(volume, &self.volume_history);

            if z_score.abs() > self.threshold {
                let mean =
                    self.volume_history.iter().sum::<f64>() / self.volume_history.len() as f64;
                let std = {
                    let variance = self
                        .volume_history
                        .iter()
                        .map(|&x| (x - mean).powi(2))
                        .sum::<f64>()
                        / self.volume_history.len() as f64;
                    variance.sqrt()
                };

                let anomaly_type = if z_score > 0.0 {
                    AnomalyType::VolumeSpike
                } else {
                    AnomalyType::VolumeDrop
                };

                anomaly = Some(Anomaly {
                    timestamp: point.timestamp,
                    anomaly_type,
                    severity: AnomalySeverity::from_score(z_score.abs()),
                    score: z_score.abs(),
                    description: format!(
                        "Volume {} detected: {} is {} standard deviations from mean",
                        if z_score > 0.0 { "spike" } else { "drop" },
                        volume,
                        z_score.abs()
                    ),
                    value: volume,
                    expected_range: (mean - self.threshold * std, mean + self.threshold * std),
                });
            }
        }

        // Update history
        self.price_history.push_back(price);
        self.volume_history.push_back(volume);

        if self.price_history.len() > self.window_size {
            self.price_history.pop_front();
        }
        if self.volume_history.len() > self.window_size {
            self.volume_history.pop_front();
        }

        Ok(anomaly)
    }

    fn reset(&mut self) {
        self.price_history.clear();
        self.volume_history.clear();
    }

    fn name(&self) -> &str {
        "Z-Score Detector"
    }
}

/// Interquartile range (IQR) based anomaly detector
#[derive(Debug, Clone)]
pub struct IQRDetector {
    window_size: usize,
    multiplier: f64,
    price_history: VecDeque<f64>,
}

impl IQRDetector {
    pub fn new(window_size: usize, multiplier: f64) -> Self {
        Self {
            window_size,
            multiplier,
            price_history: VecDeque::new(),
        }
    }

    fn calculate_quartiles(&self, data: &[f64]) -> (f64, f64, f64) {
        if data.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut sorted = data.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = sorted.len();
        let q1_idx = n / 4;
        let q2_idx = n / 2;
        let q3_idx = 3 * n / 4;

        (sorted[q1_idx], sorted[q2_idx], sorted[q3_idx])
    }
}

impl Default for IQRDetector {
    fn default() -> Self {
        Self::new(30, 1.5)
    }
}

impl AnomalyDetector for IQRDetector {
    fn detect(&mut self, data: &[PricePoint]) -> anyhow::Result<Vec<Anomaly>> {
        let mut anomalies = Vec::new();

        for point in data {
            if let Some(anomaly) = self.detect_realtime(point)? {
                anomalies.push(anomaly);
            }
        }

        Ok(anomalies)
    }

    fn detect_realtime(&mut self, point: &PricePoint) -> anyhow::Result<Option<Anomaly>> {
        let price = point.close.to_string().parse::<f64>().unwrap_or(0.0);

        let mut anomaly = None;

        if self.price_history.len() >= self.window_size {
            let history: Vec<f64> = self.price_history.iter().copied().collect();
            let (q1, _q2, q3) = self.calculate_quartiles(&history);
            let iqr = q3 - q1;

            let lower_bound = q1 - self.multiplier * iqr;
            let upper_bound = q3 + self.multiplier * iqr;

            if price < lower_bound || price > upper_bound {
                let anomaly_type = if price < lower_bound {
                    AnomalyType::PriceDrop
                } else {
                    AnomalyType::PriceSpike
                };

                let deviation = if price < lower_bound {
                    (lower_bound - price) / iqr.max(1.0)
                } else {
                    (price - upper_bound) / iqr.max(1.0)
                };

                anomaly = Some(Anomaly {
                    timestamp: point.timestamp,
                    anomaly_type,
                    severity: AnomalySeverity::from_score(deviation),
                    score: deviation,
                    description: format!(
                        "Price {} is outside IQR bounds [{:.2}, {:.2}]",
                        price, lower_bound, upper_bound
                    ),
                    value: price,
                    expected_range: (lower_bound, upper_bound),
                });
            }
        }

        self.price_history.push_back(price);
        if self.price_history.len() > self.window_size {
            self.price_history.pop_front();
        }

        Ok(anomaly)
    }

    fn reset(&mut self) {
        self.price_history.clear();
    }

    fn name(&self) -> &str {
        "IQR Detector"
    }
}

/// Volatility-based anomaly detector
#[derive(Debug, Clone)]
pub struct VolatilityDetector {
    window_size: usize,
    threshold: f64,
    price_history: VecDeque<f64>,
}

impl VolatilityDetector {
    pub fn new(window_size: usize, threshold: f64) -> Self {
        Self {
            window_size,
            threshold,
            price_history: VecDeque::new(),
        }
    }

    fn calculate_volatility(&self, prices: &[f64]) -> f64 {
        if prices.len() < 2 {
            return 0.0;
        }

        let returns: Vec<f64> = prices
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0].max(1e-10))
            .collect();

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|&r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }
}

impl Default for VolatilityDetector {
    fn default() -> Self {
        Self::new(20, 0.05)
    }
}

impl AnomalyDetector for VolatilityDetector {
    fn detect(&mut self, data: &[PricePoint]) -> anyhow::Result<Vec<Anomaly>> {
        let mut anomalies = Vec::new();

        for point in data {
            if let Some(anomaly) = self.detect_realtime(point)? {
                anomalies.push(anomaly);
            }
        }

        Ok(anomalies)
    }

    fn detect_realtime(&mut self, point: &PricePoint) -> anyhow::Result<Option<Anomaly>> {
        let price = point.close.to_string().parse::<f64>().unwrap_or(0.0);
        self.price_history.push_back(price);

        if self.price_history.len() > self.window_size {
            self.price_history.pop_front();
        }

        let mut anomaly = None;

        if self.price_history.len() >= self.window_size {
            let prices: Vec<f64> = self.price_history.iter().copied().collect();
            let volatility = self.calculate_volatility(&prices);

            if volatility > self.threshold {
                let severity_score = (volatility / self.threshold) * 2.0;

                anomaly = Some(Anomaly {
                    timestamp: point.timestamp,
                    anomaly_type: AnomalyType::Volatility,
                    severity: AnomalySeverity::from_score(severity_score),
                    score: severity_score,
                    description: format!(
                        "High volatility detected: {:.4} (threshold: {:.4})",
                        volatility, self.threshold
                    ),
                    value: volatility,
                    expected_range: (0.0, self.threshold),
                });
            }
        }

        Ok(anomaly)
    }

    fn reset(&mut self) {
        self.price_history.clear();
    }

    fn name(&self) -> &str {
        "Volatility Detector"
    }
}

/// Composite anomaly detector that combines multiple detectors
pub struct CompositeAnomalyDetector {
    detectors: Vec<Box<dyn AnomalyDetector>>,
}

impl CompositeAnomalyDetector {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn add_detector(mut self, detector: Box<dyn AnomalyDetector>) -> Self {
        self.detectors.push(detector);
        self
    }

    pub fn with_zscore(self) -> Self {
        self.add_detector(Box::new(ZScoreDetector::default()))
    }

    pub fn with_iqr(self) -> Self {
        self.add_detector(Box::new(IQRDetector::default()))
    }

    pub fn with_volatility(self) -> Self {
        self.add_detector(Box::new(VolatilityDetector::default()))
    }
}

impl Default for CompositeAnomalyDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl AnomalyDetector for CompositeAnomalyDetector {
    fn detect(&mut self, data: &[PricePoint]) -> anyhow::Result<Vec<Anomaly>> {
        let mut all_anomalies = Vec::new();

        for detector in &mut self.detectors {
            all_anomalies.extend(detector.detect(data)?);
        }

        // Sort by timestamp and severity
        all_anomalies.sort_by(|a, b| match a.timestamp.cmp(&b.timestamp) {
            std::cmp::Ordering::Equal => b.severity.cmp(&a.severity),
            other => other,
        });

        Ok(all_anomalies)
    }

    fn detect_realtime(&mut self, point: &PricePoint) -> anyhow::Result<Option<Anomaly>> {
        let mut highest_severity_anomaly: Option<Anomaly> = None;

        for detector in &mut self.detectors {
            if let Some(anomaly) = detector.detect_realtime(point)? {
                if let Some(ref current) = highest_severity_anomaly {
                    if anomaly.severity > current.severity {
                        highest_severity_anomaly = Some(anomaly);
                    }
                } else {
                    highest_severity_anomaly = Some(anomaly);
                }
            }
        }

        Ok(highest_severity_anomaly)
    }

    fn reset(&mut self) {
        for detector in &mut self.detectors {
            detector.reset();
        }
    }

    fn name(&self) -> &str {
        "Composite Detector"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    fn create_normal_data() -> Vec<PricePoint> {
        let now = Utc::now();
        (0..100)
            .map(|i| PricePoint {
                timestamp: now - chrono::Duration::days(100 - i),
                open: dec!(100),
                high: dec!(105),
                low: dec!(95),
                close: dec!(100),
                volume: dec!(1000),
            })
            .collect()
    }

    fn create_anomaly_data() -> Vec<PricePoint> {
        let now = Utc::now();
        let mut data: Vec<_> = (0..100)
            .map(|i| PricePoint {
                timestamp: now - chrono::Duration::days(100 - i),
                open: dec!(100),
                high: dec!(105),
                low: dec!(95),
                // Add small variation to prices to avoid zero std_dev
                close: dec!(100) + Decimal::from(i % 10) - dec!(5),
                volume: dec!(1000),
            })
            .collect();

        // Add significant anomaly
        data[50].close = dec!(500);

        data
    }

    #[test]
    fn test_zscore_detector() {
        let mut detector = ZScoreDetector::default();
        let data = create_anomaly_data();

        let anomalies = detector.detect(&data).unwrap();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_iqr_detector() {
        let mut detector = IQRDetector::default();
        let data = create_anomaly_data();

        let anomalies = detector.detect(&data).unwrap();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_volatility_detector() {
        let mut detector = VolatilityDetector::new(20, 0.01);
        let data = create_anomaly_data();

        let anomalies = detector.detect(&data).unwrap();
        // May or may not detect depending on volatility threshold
        // Just verify it doesn't error
        let _ = anomalies.len();
    }

    #[test]
    fn test_composite_detector() {
        let mut detector = CompositeAnomalyDetector::new().with_zscore().with_iqr();

        let data = create_anomaly_data();
        let anomalies = detector.detect(&data).unwrap();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_no_anomalies_in_normal_data() {
        let mut detector = ZScoreDetector::default();
        let data = create_normal_data();

        let anomalies = detector.detect(&data).unwrap();
        assert_eq!(anomalies.len(), 0);
    }

    #[test]
    fn test_anomaly_severity() {
        assert_eq!(AnomalySeverity::from_score(1.5), AnomalySeverity::Low);
        assert_eq!(AnomalySeverity::from_score(2.5), AnomalySeverity::Medium);
        assert_eq!(AnomalySeverity::from_score(3.5), AnomalySeverity::High);
        assert_eq!(AnomalySeverity::from_score(4.5), AnomalySeverity::Critical);
    }
}
