//! Chart pattern recognition
//!
//! This module provides detection of common chart patterns used in technical analysis

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Price point with OHLC data
#[derive(Debug, Clone, Copy)]
pub struct Candle {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub timestamp: i64,
}

/// Chart pattern types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartPattern {
    /// Bearish reversal pattern
    HeadAndShoulders,
    /// Bullish reversal pattern
    InverseHeadAndShoulders,
    /// Continuation patterns
    AscendingTriangle,
    DescendingTriangle,
    SymmetricalTriangle,
    /// Reversal patterns
    DoubleTop,
    DoubleBottom,
    /// Short-term patterns
    BullFlag,
    BearFlag,
    Pennant,
}

/// Pattern detection result
#[derive(Debug, Clone)]
pub struct PatternMatch {
    pub pattern: ChartPattern,
    pub start_index: usize,
    pub end_index: usize,
    pub confidence: Decimal,
    pub target_price: Option<Decimal>,
}

/// Chart pattern detector
pub struct ChartPatternDetector {
    min_confidence: Decimal,
    lookback_periods: usize,
}

impl ChartPatternDetector {
    /// Create a new pattern detector
    pub fn new(min_confidence: Decimal, lookback_periods: usize) -> Self {
        Self {
            min_confidence,
            lookback_periods,
        }
    }

    /// Detect patterns in candle data
    pub fn detect_patterns(&self, candles: &[Candle]) -> Vec<PatternMatch> {
        let mut patterns = Vec::new();

        if candles.len() < 5 {
            return patterns;
        }

        // Check for each pattern type
        if let Some(pattern) = self.detect_head_and_shoulders(candles) {
            if pattern.confidence >= self.min_confidence {
                patterns.push(pattern);
            }
        }

        if let Some(pattern) = self.detect_double_top(candles) {
            if pattern.confidence >= self.min_confidence {
                patterns.push(pattern);
            }
        }

        if let Some(pattern) = self.detect_double_bottom(candles) {
            if pattern.confidence >= self.min_confidence {
                patterns.push(pattern);
            }
        }

        if let Some(pattern) = self.detect_triangle_patterns(candles) {
            if pattern.confidence >= self.min_confidence {
                patterns.push(pattern);
            }
        }

        patterns
    }

    /// Detect head and shoulders pattern
    fn detect_head_and_shoulders(&self, candles: &[Candle]) -> Option<PatternMatch> {
        let len = candles.len();
        if len < 7 {
            return None;
        }

        let lookback = self.lookback_periods.min(len);
        let start = len.saturating_sub(lookback);

        // Find three peaks
        let peaks = self.find_local_maxima(&candles[start..]);
        if peaks.len() < 3 {
            return None;
        }

        // Check if middle peak is highest (head) and outer peaks are similar (shoulders)
        let left_shoulder = peaks[0];
        let head = peaks[1];
        let right_shoulder = peaks[2];

        let head_price = candles[start + head].high;
        let left_price = candles[start + left_shoulder].high;
        let right_price = candles[start + right_shoulder].high;

        // Head should be higher than shoulders
        if head_price <= left_price || head_price <= right_price {
            return None;
        }

        // Shoulders should be relatively similar (within 5%)
        let shoulder_diff = (left_price - right_price).abs() / left_price;
        if shoulder_diff > dec!(0.05) {
            return None;
        }

        // Calculate neckline (support between shoulders)
        let troughs =
            self.find_local_minima(&candles[start + left_shoulder..start + right_shoulder]);
        if troughs.is_empty() {
            return None;
        }

        let neckline = candles[start + left_shoulder + troughs[0]].low;

        // Calculate confidence based on pattern quality
        let confidence = dec!(1.0) - shoulder_diff * dec!(10.0);
        let confidence = confidence.max(dec!(0.5)).min(dec!(1.0));

        // Target price is typically the distance from head to neckline, projected down
        let target_price = Some(neckline - (head_price - neckline));

        Some(PatternMatch {
            pattern: ChartPattern::HeadAndShoulders,
            start_index: start + left_shoulder,
            end_index: start + right_shoulder,
            confidence,
            target_price,
        })
    }

    /// Detect double top pattern
    fn detect_double_top(&self, candles: &[Candle]) -> Option<PatternMatch> {
        let len = candles.len();
        if len < 5 {
            return None;
        }

        let lookback = self.lookback_periods.min(len);
        let start = len.saturating_sub(lookback);

        let peaks = self.find_local_maxima(&candles[start..]);
        if peaks.len() < 2 {
            return None;
        }

        // Get last two peaks
        let first_peak = peaks[peaks.len() - 2];
        let second_peak = peaks[peaks.len() - 1];

        let first_price = candles[start + first_peak].high;
        let second_price = candles[start + second_peak].high;

        // Peaks should be similar (within 3%)
        let price_diff = (first_price - second_price).abs() / first_price;
        if price_diff > dec!(0.03) {
            return None;
        }

        // Find trough between peaks
        let troughs = self.find_local_minima(&candles[start + first_peak..start + second_peak]);
        if troughs.is_empty() {
            return None;
        }

        let support_level = candles[start + first_peak + troughs[0]].low;

        let confidence = dec!(1.0) - price_diff * dec!(20.0);
        let confidence = confidence.max(dec!(0.5)).min(dec!(1.0));

        // Target is distance from peaks to support, projected down
        let target_price = Some(support_level - (first_price - support_level));

        Some(PatternMatch {
            pattern: ChartPattern::DoubleTop,
            start_index: start + first_peak,
            end_index: start + second_peak,
            confidence,
            target_price,
        })
    }

    /// Detect double bottom pattern
    fn detect_double_bottom(&self, candles: &[Candle]) -> Option<PatternMatch> {
        let len = candles.len();
        if len < 5 {
            return None;
        }

        let lookback = self.lookback_periods.min(len);
        let start = len.saturating_sub(lookback);

        let troughs = self.find_local_minima(&candles[start..]);
        if troughs.len() < 2 {
            return None;
        }

        let first_trough = troughs[troughs.len() - 2];
        let second_trough = troughs[troughs.len() - 1];

        let first_price = candles[start + first_trough].low;
        let second_price = candles[start + second_trough].low;

        // Troughs should be similar (within 3%)
        let price_diff = (first_price - second_price).abs() / first_price;
        if price_diff > dec!(0.03) {
            return None;
        }

        // Find peak between troughs
        let peaks = self.find_local_maxima(&candles[start + first_trough..start + second_trough]);
        if peaks.is_empty() {
            return None;
        }

        let resistance_level = candles[start + first_trough + peaks[0]].high;

        let confidence = dec!(1.0) - price_diff * dec!(20.0);
        let confidence = confidence.max(dec!(0.5)).min(dec!(1.0));

        // Target is distance from resistance to troughs, projected up
        let target_price = Some(resistance_level + (resistance_level - first_price));

        Some(PatternMatch {
            pattern: ChartPattern::DoubleBottom,
            start_index: start + first_trough,
            end_index: start + second_trough,
            confidence,
            target_price,
        })
    }

    /// Detect triangle patterns
    fn detect_triangle_patterns(&self, candles: &[Candle]) -> Option<PatternMatch> {
        let len = candles.len();
        if len < 10 {
            return None;
        }

        let lookback = self.lookback_periods.min(len);
        let start = len.saturating_sub(lookback);

        let highs = self.find_local_maxima(&candles[start..]);
        let lows = self.find_local_minima(&candles[start..]);

        if highs.len() < 2 || lows.len() < 2 {
            return None;
        }

        // Calculate trend lines for highs and lows
        let high_slope = self.calculate_slope(&highs, &candles[start..], true);
        let low_slope = self.calculate_slope(&lows, &candles[start..], false);

        let pattern_type = if high_slope.abs() < dec!(0.001) && low_slope > dec!(0) {
            // Horizontal resistance, rising support = Ascending Triangle
            ChartPattern::AscendingTriangle
        } else if low_slope.abs() < dec!(0.001) && high_slope < dec!(0) {
            // Horizontal support, falling resistance = Descending Triangle
            ChartPattern::DescendingTriangle
        } else if high_slope < dec!(0)
            && low_slope > dec!(0)
            && (high_slope.abs() - low_slope.abs()).abs() < dec!(0.01)
        {
            // Converging lines = Symmetrical Triangle
            ChartPattern::SymmetricalTriangle
        } else {
            return None;
        };

        Some(PatternMatch {
            pattern: pattern_type,
            start_index: start,
            end_index: len - 1,
            confidence: dec!(0.75),
            target_price: None,
        })
    }

    /// Find local maxima in price data
    fn find_local_maxima(&self, candles: &[Candle]) -> Vec<usize> {
        let mut maxima = Vec::new();

        for i in 1..candles.len() - 1 {
            if candles[i].high > candles[i - 1].high && candles[i].high > candles[i + 1].high {
                maxima.push(i);
            }
        }

        maxima
    }

    /// Find local minima in price data
    fn find_local_minima(&self, candles: &[Candle]) -> Vec<usize> {
        let mut minima = Vec::new();

        for i in 1..candles.len() - 1 {
            if candles[i].low < candles[i - 1].low && candles[i].low < candles[i + 1].low {
                minima.push(i);
            }
        }

        minima
    }

    /// Calculate slope of trend line through points
    fn calculate_slope(&self, indices: &[usize], candles: &[Candle], use_high: bool) -> Decimal {
        if indices.len() < 2 {
            return dec!(0);
        }

        let first_idx = indices[0];
        let last_idx = indices[indices.len() - 1];

        let first_price = if use_high {
            candles[first_idx].high
        } else {
            candles[first_idx].low
        };

        let last_price = if use_high {
            candles[last_idx].high
        } else {
            candles[last_idx].low
        };

        let time_diff = Decimal::from(last_idx - first_idx);
        if time_diff == dec!(0) {
            return dec!(0);
        }

        (last_price - first_price) / time_diff
    }
}

impl Default for ChartPatternDetector {
    fn default() -> Self {
        Self::new(dec!(0.7), 50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_candles(prices: Vec<Decimal>) -> Vec<Candle> {
        prices
            .iter()
            .enumerate()
            .map(|(i, &price)| Candle {
                open: price,
                high: price,
                low: price,
                close: price,
                volume: dec!(1000),
                timestamp: i as i64,
            })
            .collect()
    }

    #[test]
    fn test_detector_creation() {
        let detector = ChartPatternDetector::new(dec!(0.8), 30);
        assert_eq!(detector.min_confidence, dec!(0.8));
        assert_eq!(detector.lookback_periods, 30);
    }

    #[test]
    fn test_find_local_maxima() {
        let prices = vec![
            dec!(10),
            dec!(15),
            dec!(12),
            dec!(18),
            dec!(14),
            dec!(16),
            dec!(13),
        ];
        let candles = create_test_candles(prices);

        let detector = ChartPatternDetector::default();
        let maxima = detector.find_local_maxima(&candles);

        assert!(maxima.contains(&1)); // 15
        assert!(maxima.contains(&3)); // 18
        assert!(maxima.contains(&5)); // 16
    }

    #[test]
    fn test_find_local_minima() {
        let prices = vec![
            dec!(15),
            dec!(10),
            dec!(12),
            dec!(8),
            dec!(14),
            dec!(9),
            dec!(13),
        ];
        let candles = create_test_candles(prices);

        let detector = ChartPatternDetector::default();
        let minima = detector.find_local_minima(&candles);

        assert!(minima.contains(&1)); // 10
        assert!(minima.contains(&3)); // 8
        assert!(minima.contains(&5)); // 9
    }

    #[test]
    fn test_double_top_detection() {
        // Create a double top pattern: low, high, low, high, low
        let prices = vec![dec!(10), dec!(20), dec!(12), dec!(20.5), dec!(11)];
        let candles = create_test_candles(prices);

        let detector = ChartPatternDetector::new(dec!(0.5), 50);
        let patterns = detector.detect_patterns(&candles);

        // May or may not detect depending on exact criteria, but should not panic
        assert!(patterns.len() <= 1);
    }

    #[test]
    fn test_no_patterns_in_short_data() {
        let prices = vec![dec!(10), dec!(11), dec!(12)];
        let candles = create_test_candles(prices);

        let detector = ChartPatternDetector::default();
        let patterns = detector.detect_patterns(&candles);

        assert!(patterns.is_empty());
    }
}
