//! Feature engineering framework for machine learning models
//!
//! This module provides comprehensive feature extraction and engineering capabilities
//! for time series and trading data, enabling ML-based price prediction and analysis.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature vector containing extracted features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub timestamp: DateTime<Utc>,
    pub features: HashMap<String, f64>,
}

impl FeatureVector {
    pub fn new(timestamp: DateTime<Utc>) -> Self {
        Self {
            timestamp,
            features: HashMap::new(),
        }
    }

    pub fn add_feature(&mut self, name: String, value: f64) {
        self.features.insert(name, value);
    }

    pub fn get_feature(&self, name: &str) -> Option<f64> {
        self.features.get(name).copied()
    }

    pub fn feature_count(&self) -> usize {
        self.features.len()
    }
}

/// Price data point for feature extraction
#[derive(Debug, Clone, Copy)]
pub struct PricePoint {
    pub timestamp: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
}

/// Feature extractor trait for implementing custom feature extraction logic
pub trait FeatureExtractor: Send + Sync {
    fn extract(&self, data: &[PricePoint]) -> anyhow::Result<Vec<FeatureVector>>;
    fn feature_names(&self) -> Vec<String>;
}

/// Technical indicator-based feature extractor
#[derive(Debug, Clone)]
pub struct TechnicalFeatureExtractor {
    sma_periods: Vec<usize>,
    ema_periods: Vec<usize>,
    rsi_period: usize,
    bollinger_period: usize,
    bollinger_std: f64,
}

impl Default for TechnicalFeatureExtractor {
    fn default() -> Self {
        Self {
            sma_periods: vec![5, 10, 20, 50, 200],
            ema_periods: vec![12, 26],
            rsi_period: 14,
            bollinger_period: 20,
            bollinger_std: 2.0,
        }
    }
}

impl TechnicalFeatureExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_sma_periods(mut self, periods: Vec<usize>) -> Self {
        self.sma_periods = periods;
        self
    }

    pub fn with_ema_periods(mut self, periods: Vec<usize>) -> Self {
        self.ema_periods = periods;
        self
    }

    fn calculate_sma(&self, prices: &[f64], period: usize) -> Vec<Option<f64>> {
        let mut result = Vec::with_capacity(prices.len());
        for i in 0..prices.len() {
            if i + 1 < period {
                result.push(None);
            } else {
                let sum: f64 = prices[i + 1 - period..=i].iter().sum();
                result.push(Some(sum / period as f64));
            }
        }
        result
    }

    fn calculate_ema(&self, prices: &[f64], period: usize) -> Vec<Option<f64>> {
        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut result = Vec::with_capacity(prices.len());

        let mut ema = None;
        for (i, &price) in prices.iter().enumerate() {
            if i + 1 < period {
                result.push(None);
            } else if ema.is_none() {
                // Initialize with SMA
                let sum: f64 = prices[i + 1 - period..=i].iter().sum();
                ema = Some(sum / period as f64);
                result.push(ema);
            } else {
                let new_ema = (price - ema.unwrap()) * multiplier + ema.unwrap();
                ema = Some(new_ema);
                result.push(Some(new_ema));
            }
        }
        result
    }

    fn calculate_rsi(&self, prices: &[f64], period: usize) -> Vec<Option<f64>> {
        let mut result = Vec::with_capacity(prices.len());

        for i in 0..prices.len() {
            if i < period {
                result.push(None);
                continue;
            }

            let mut gains = 0.0;
            let mut losses = 0.0;

            for j in i - period + 1..=i {
                let change = prices[j] - prices[j - 1];
                if change > 0.0 {
                    gains += change;
                } else {
                    losses += -change;
                }
            }

            let avg_gain = gains / period as f64;
            let avg_loss = losses / period as f64;

            let rsi = if avg_loss == 0.0 {
                100.0
            } else {
                100.0 - (100.0 / (1.0 + avg_gain / avg_loss))
            };

            result.push(Some(rsi));
        }
        result
    }

    fn calculate_bollinger_bands(
        &self,
        prices: &[f64],
        period: usize,
        num_std: f64,
    ) -> Vec<Option<(f64, f64, f64)>> {
        let mut result = Vec::with_capacity(prices.len());

        for i in 0..prices.len() {
            if i + 1 < period {
                result.push(None);
                continue;
            }

            let window = &prices[i + 1 - period..=i];
            let mean: f64 = window.iter().sum::<f64>() / period as f64;
            let variance: f64 =
                window.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / period as f64;
            let std_dev = variance.sqrt();

            let upper = mean + num_std * std_dev;
            let lower = mean - num_std * std_dev;

            result.push(Some((lower, mean, upper)));
        }
        result
    }
}

impl FeatureExtractor for TechnicalFeatureExtractor {
    fn extract(&self, data: &[PricePoint]) -> anyhow::Result<Vec<FeatureVector>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let close_prices: Vec<f64> = data
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let volumes: Vec<f64> = data
            .iter()
            .map(|p| p.volume.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        // Calculate all indicators
        let mut sma_results = HashMap::new();
        for &period in &self.sma_periods {
            sma_results.insert(period, self.calculate_sma(&close_prices, period));
        }

        let mut ema_results = HashMap::new();
        for &period in &self.ema_periods {
            ema_results.insert(period, self.calculate_ema(&close_prices, period));
        }

        let rsi = self.calculate_rsi(&close_prices, self.rsi_period);
        let bollinger = self.calculate_bollinger_bands(
            &close_prices,
            self.bollinger_period,
            self.bollinger_std,
        );

        // Build feature vectors
        let mut features = Vec::new();
        for (i, point) in data.iter().enumerate() {
            let mut fv = FeatureVector::new(point.timestamp);

            // Price features
            fv.add_feature("close".to_string(), close_prices[i]);
            fv.add_feature("volume".to_string(), volumes[i]);

            // Returns
            if i > 0 {
                let ret = (close_prices[i] - close_prices[i - 1]) / close_prices[i - 1];
                fv.add_feature("return_1d".to_string(), ret);
            }

            if i >= 5 {
                let ret = (close_prices[i] - close_prices[i - 5]) / close_prices[i - 5];
                fv.add_feature("return_5d".to_string(), ret);
            }

            // SMA features
            for (&period, values) in &sma_results {
                if let Some(Some(sma)) = values.get(i) {
                    fv.add_feature(format!("sma_{}", period), *sma);
                    fv.add_feature(format!("sma_{}_ratio", period), close_prices[i] / sma);
                }
            }

            // EMA features
            for (&period, values) in &ema_results {
                if let Some(Some(ema)) = values.get(i) {
                    fv.add_feature(format!("ema_{}", period), *ema);
                }
            }

            // RSI
            if let Some(Some(rsi_val)) = rsi.get(i) {
                fv.add_feature("rsi".to_string(), *rsi_val);
            }

            // Bollinger Bands
            if let Some(Some((lower, middle, upper))) = bollinger.get(i) {
                fv.add_feature("bb_lower".to_string(), *lower);
                fv.add_feature("bb_middle".to_string(), *middle);
                fv.add_feature("bb_upper".to_string(), *upper);
                fv.add_feature("bb_width".to_string(), upper - lower);
                fv.add_feature(
                    "bb_position".to_string(),
                    (close_prices[i] - lower) / (upper - lower),
                );
            }

            // Volatility
            if i >= 20 {
                let window = &close_prices[i - 19..=i];
                let mean: f64 = window.iter().sum::<f64>() / 20.0;
                let variance: f64 = window.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / 20.0;
                fv.add_feature("volatility_20d".to_string(), variance.sqrt());
            }

            features.push(fv);
        }

        Ok(features)
    }

    fn feature_names(&self) -> Vec<String> {
        let mut names = vec![
            "close".to_string(),
            "volume".to_string(),
            "return_1d".to_string(),
            "return_5d".to_string(),
            "rsi".to_string(),
            "bb_lower".to_string(),
            "bb_middle".to_string(),
            "bb_upper".to_string(),
            "bb_width".to_string(),
            "bb_position".to_string(),
            "volatility_20d".to_string(),
        ];

        for period in &self.sma_periods {
            names.push(format!("sma_{}", period));
            names.push(format!("sma_{}_ratio", period));
        }

        for period in &self.ema_periods {
            names.push(format!("ema_{}", period));
        }

        names
    }
}

/// Volume-based feature extractor
#[derive(Debug, Clone)]
pub struct VolumeFeatureExtractor {
    lookback_periods: Vec<usize>,
}

impl Default for VolumeFeatureExtractor {
    fn default() -> Self {
        Self {
            lookback_periods: vec![5, 10, 20],
        }
    }
}

impl VolumeFeatureExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_periods(mut self, periods: Vec<usize>) -> Self {
        self.lookback_periods = periods;
        self
    }
}

impl FeatureExtractor for VolumeFeatureExtractor {
    fn extract(&self, data: &[PricePoint]) -> anyhow::Result<Vec<FeatureVector>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let volumes: Vec<f64> = data
            .iter()
            .map(|p| p.volume.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let close_prices: Vec<f64> = data
            .iter()
            .map(|p| p.close.to_string().parse::<f64>().unwrap_or(0.0))
            .collect();

        let mut features = Vec::new();
        for (i, point) in data.iter().enumerate() {
            let mut fv = FeatureVector::new(point.timestamp);

            fv.add_feature("volume".to_string(), volumes[i]);

            // Volume ratios
            for &period in &self.lookback_periods {
                if i + 1 >= period {
                    let avg_vol: f64 =
                        volumes[i + 1 - period..=i].iter().sum::<f64>() / period as f64;
                    fv.add_feature(
                        format!("volume_ratio_{}", period),
                        volumes[i] / avg_vol.max(1.0),
                    );
                }
            }

            // On-balance volume (OBV)
            if i > 0 {
                let price_change = close_prices[i] - close_prices[i - 1];
                let obv_change = if price_change > 0.0 {
                    volumes[i]
                } else if price_change < 0.0 {
                    -volumes[i]
                } else {
                    0.0
                };
                fv.add_feature("obv_change".to_string(), obv_change);
            }

            features.push(fv);
        }

        Ok(features)
    }

    fn feature_names(&self) -> Vec<String> {
        let mut names = vec!["volume".to_string(), "obv_change".to_string()];
        for period in &self.lookback_periods {
            names.push(format!("volume_ratio_{}", period));
        }
        names
    }
}

/// Composite feature extractor that combines multiple extractors
pub struct CompositeFeatureExtractor {
    extractors: Vec<Box<dyn FeatureExtractor>>,
}

impl CompositeFeatureExtractor {
    pub fn new() -> Self {
        Self {
            extractors: Vec::new(),
        }
    }

    pub fn add_extractor(mut self, extractor: Box<dyn FeatureExtractor>) -> Self {
        self.extractors.push(extractor);
        self
    }

    pub fn with_technical(self) -> Self {
        self.add_extractor(Box::new(TechnicalFeatureExtractor::new()))
    }

    pub fn with_volume(self) -> Self {
        self.add_extractor(Box::new(VolumeFeatureExtractor::new()))
    }
}

impl Default for CompositeFeatureExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureExtractor for CompositeFeatureExtractor {
    fn extract(&self, data: &[PricePoint]) -> anyhow::Result<Vec<FeatureVector>> {
        if self.extractors.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_features = Vec::new();
        for extractor in &self.extractors {
            all_features.push(extractor.extract(data)?);
        }

        // Merge features
        let len = all_features[0].len();
        let mut result = Vec::with_capacity(len);

        for i in 0..len {
            let timestamp = all_features[0][i].timestamp;
            let mut fv = FeatureVector::new(timestamp);

            for feature_list in &all_features {
                for (name, value) in &feature_list[i].features {
                    fv.add_feature(name.clone(), *value);
                }
            }

            result.push(fv);
        }

        Ok(result)
    }

    fn feature_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for extractor in &self.extractors {
            names.extend(extractor.feature_names());
        }
        names.sort();
        names.dedup();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_data() -> Vec<PricePoint> {
        let now = Utc::now();
        (0..100)
            .map(|i| PricePoint {
                timestamp: now - chrono::Duration::days(100 - i),
                open: dec!(100) + Decimal::from(i),
                high: dec!(105) + Decimal::from(i),
                low: dec!(95) + Decimal::from(i),
                close: dec!(100) + Decimal::from(i),
                volume: dec!(1000) + Decimal::from(i * 10),
            })
            .collect()
    }

    #[test]
    fn test_technical_feature_extractor() {
        let data = create_test_data();
        let extractor = TechnicalFeatureExtractor::new();

        let features = extractor.extract(&data).unwrap();
        assert_eq!(features.len(), data.len());

        // Last feature should have most features populated
        let last = &features[features.len() - 1];
        assert!(last.get_feature("close").is_some());
        assert!(last.get_feature("sma_20").is_some());
        assert!(last.get_feature("rsi").is_some());
    }

    #[test]
    fn test_volume_feature_extractor() {
        let data = create_test_data();
        let extractor = VolumeFeatureExtractor::new();

        let features = extractor.extract(&data).unwrap();
        assert_eq!(features.len(), data.len());

        let last = &features[features.len() - 1];
        assert!(last.get_feature("volume").is_some());
    }

    #[test]
    fn test_composite_extractor() {
        let data = create_test_data();
        let extractor = CompositeFeatureExtractor::new()
            .with_technical()
            .with_volume();

        let features = extractor.extract(&data).unwrap();
        assert_eq!(features.len(), data.len());

        // Should have features from both extractors
        let last = &features[features.len() - 1];
        assert!(last.get_feature("close").is_some());
        assert!(last.get_feature("volume").is_some());
        assert!(last.get_feature("rsi").is_some());
    }

    #[test]
    fn test_feature_names() {
        let extractor = TechnicalFeatureExtractor::new();
        let names = extractor.feature_names();
        assert!(!names.is_empty());
        assert!(names.contains(&"close".to_string()));
        assert!(names.contains(&"rsi".to_string()));
    }
}
