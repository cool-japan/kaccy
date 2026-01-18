//! Advanced mathematical utilities for trading and analysis
//!
//! This module provides sophisticated mathematical functions for:
//! - Statistical analysis (correlation, covariance, percentiles)
//! - Time series decomposition
//! - Numerical optimization
//! - Signal processing
//! - Portfolio mathematics

use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::collections::VecDeque;

/// Statistical moments calculator
#[derive(Debug, Clone)]
pub struct MomentsCalculator {
    values: VecDeque<Decimal>,
    max_size: usize,
}

impl MomentsCalculator {
    /// Create a new moments calculator with a maximum window size
    pub fn new(max_size: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Add a new value to the window
    pub fn add(&mut self, value: Decimal) {
        if self.values.len() >= self.max_size {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Calculate the mean (first moment)
    pub fn mean(&self) -> Option<Decimal> {
        if self.values.is_empty() {
            return None;
        }
        let sum: Decimal = self.values.iter().sum();
        Some(sum / Decimal::from(self.values.len()))
    }

    /// Calculate variance (second central moment)
    pub fn variance(&self) -> Option<Decimal> {
        let mean = self.mean()?;
        if self.values.len() < 2 {
            return None;
        }

        let sum_squared_diff: Decimal = self.values.iter().map(|&x| (x - mean) * (x - mean)).sum();

        Some(sum_squared_diff / Decimal::from(self.values.len() - 1))
    }

    /// Calculate standard deviation
    pub fn std_dev(&self) -> Option<Decimal> {
        let variance = self.variance()?;
        // Simple square root approximation for Decimal
        variance.sqrt()
    }

    /// Calculate skewness (third standardized moment)
    pub fn skewness(&self) -> Option<Decimal> {
        let mean = self.mean()?;
        let std_dev = self.std_dev()?;

        if std_dev == Decimal::ZERO || self.values.len() < 3 {
            return None;
        }

        let n = Decimal::from(self.values.len());
        let sum_cubed: Decimal = self
            .values
            .iter()
            .map(|&x| {
                let z = (x - mean) / std_dev;
                z * z * z
            })
            .sum();

        Some(sum_cubed / n)
    }

    /// Calculate kurtosis (fourth standardized moment)
    pub fn kurtosis(&self) -> Option<Decimal> {
        let mean = self.mean()?;
        let std_dev = self.std_dev()?;

        if std_dev == Decimal::ZERO || self.values.len() < 4 {
            return None;
        }

        let n = Decimal::from(self.values.len());
        let sum_fourth: Decimal = self
            .values
            .iter()
            .map(|&x| {
                let z = (x - mean) / std_dev;
                z * z * z * z
            })
            .sum();

        // Excess kurtosis (subtract 3 for normal distribution baseline)
        Some((sum_fourth / n) - Decimal::from(3))
    }
}

/// Calculate Pearson correlation coefficient between two series
pub fn correlation(x: &[Decimal], y: &[Decimal]) -> Option<Decimal> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let n = Decimal::from(x.len());
    let mean_x: Decimal = x.iter().sum::<Decimal>() / n;
    let mean_y: Decimal = y.iter().sum::<Decimal>() / n;

    let mut cov = Decimal::ZERO;
    let mut var_x = Decimal::ZERO;
    let mut var_y = Decimal::ZERO;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x == Decimal::ZERO || var_y == Decimal::ZERO {
        return None;
    }

    let denom = (var_x * var_y).sqrt()?;
    Some(cov / denom)
}

/// Calculate covariance between two series
pub fn covariance(x: &[Decimal], y: &[Decimal]) -> Option<Decimal> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }

    let n = Decimal::from(x.len());
    let mean_x: Decimal = x.iter().sum::<Decimal>() / n;
    let mean_y: Decimal = y.iter().sum::<Decimal>() / n;

    let cov: Decimal = x
        .iter()
        .zip(y.iter())
        .map(|(&xi, &yi)| (xi - mean_x) * (yi - mean_y))
        .sum();

    Some(cov / (n - Decimal::ONE))
}

/// Calculate percentile (0-100) using linear interpolation
pub fn percentile(values: &[Decimal], p: u8) -> Option<Decimal> {
    if values.is_empty() || p > 100 {
        return None;
    }

    let mut sorted = values.to_vec();
    sorted.sort();

    if p == 0 {
        return Some(sorted[0]);
    }
    if p == 100 {
        return Some(sorted[sorted.len() - 1]);
    }

    let rank = Decimal::from(p) / Decimal::from(100) * Decimal::from(sorted.len() - 1);
    let lower_idx = rank.floor().to_usize()?;
    let upper_idx = rank.ceil().to_usize()?;

    if lower_idx == upper_idx {
        return Some(sorted[lower_idx]);
    }

    let lower_val = sorted[lower_idx];
    let upper_val = sorted[upper_idx];
    let fraction = rank - Decimal::from(lower_idx);

    Some(lower_val + (upper_val - lower_val) * fraction)
}

/// Exponential moving average calculator
#[derive(Debug, Clone)]
pub struct EMACalculator {
    alpha: Decimal,
    current_value: Option<Decimal>,
}

impl EMACalculator {
    /// Create a new EMA calculator with specified smoothing factor
    /// alpha should be between 0 and 1 (typically 2/(n+1) for n-period EMA)
    pub fn new(alpha: Decimal) -> Self {
        Self {
            alpha: alpha.max(Decimal::ZERO).min(Decimal::ONE),
            current_value: None,
        }
    }

    /// Create EMA calculator from period (uses alpha = 2/(period+1))
    pub fn from_period(period: usize) -> Self {
        let alpha = Decimal::from(2) / Decimal::from(period + 1);
        Self::new(alpha)
    }

    /// Update with new value and return current EMA
    pub fn update(&mut self, value: Decimal) -> Decimal {
        match self.current_value {
            None => {
                self.current_value = Some(value);
                value
            }
            Some(prev) => {
                let new_value = self.alpha * value + (Decimal::ONE - self.alpha) * prev;
                self.current_value = Some(new_value);
                new_value
            }
        }
    }

    /// Get current EMA value without updating
    pub fn current(&self) -> Option<Decimal> {
        self.current_value
    }

    /// Reset the calculator
    pub fn reset(&mut self) {
        self.current_value = None;
    }
}

/// Simple linear regression calculator
#[derive(Debug, Clone)]
pub struct LinearRegression {
    pub slope: Decimal,
    pub intercept: Decimal,
    pub r_squared: Decimal,
}

impl LinearRegression {
    /// Fit a linear regression model to the data
    /// Returns None if insufficient data or calculation fails
    pub fn fit(x: &[Decimal], y: &[Decimal]) -> Option<Self> {
        if x.len() != y.len() || x.len() < 2 {
            return None;
        }

        let n = Decimal::from(x.len());
        let sum_x: Decimal = x.iter().sum();
        let sum_y: Decimal = y.iter().sum();
        let sum_xy: Decimal = x.iter().zip(y.iter()).map(|(&xi, &yi)| xi * yi).sum();
        let sum_x2: Decimal = x.iter().map(|&xi| xi * xi).sum();

        let mean_x = sum_x / n;
        let mean_y = sum_y / n;

        let numerator = sum_xy - n * mean_x * mean_y;
        let denominator = sum_x2 - n * mean_x * mean_x;

        if denominator == Decimal::ZERO {
            return None;
        }

        let slope = numerator / denominator;
        let intercept = mean_y - slope * mean_x;

        // Calculate R-squared
        let ss_tot: Decimal = y.iter().map(|&yi| (yi - mean_y) * (yi - mean_y)).sum();
        let ss_res: Decimal = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| {
                let predicted = slope * xi + intercept;
                (yi - predicted) * (yi - predicted)
            })
            .sum();

        let r_squared = if ss_tot != Decimal::ZERO {
            Decimal::ONE - (ss_res / ss_tot)
        } else {
            Decimal::ZERO
        };

        Some(Self {
            slope,
            intercept,
            r_squared,
        })
    }

    /// Predict y value for given x
    pub fn predict(&self, x: Decimal) -> Decimal {
        self.slope * x + self.intercept
    }
}

/// Calculate autocorrelation at a given lag
pub fn autocorrelation(values: &[Decimal], lag: usize) -> Option<Decimal> {
    if lag >= values.len() || values.len() < 2 {
        return None;
    }

    let n = values.len();
    let mean: Decimal = values.iter().sum::<Decimal>() / Decimal::from(n);

    let mut numerator = Decimal::ZERO;
    let mut denominator = Decimal::ZERO;

    for i in 0..(n - lag) {
        numerator += (values[i] - mean) * (values[i + lag] - mean);
    }

    for &value in values {
        denominator += (value - mean) * (value - mean);
    }

    if denominator == Decimal::ZERO {
        return None;
    }

    Some(numerator / denominator)
}

/// Z-score normalization
pub fn z_score(value: Decimal, mean: Decimal, std_dev: Decimal) -> Option<Decimal> {
    if std_dev == Decimal::ZERO {
        return None;
    }
    Some((value - mean) / std_dev)
}

/// Min-max normalization to [0, 1] range
pub fn min_max_normalize(values: &[Decimal]) -> Option<Vec<Decimal>> {
    if values.is_empty() {
        return None;
    }

    let min_val = *values.iter().min()?;
    let max_val = *values.iter().max()?;
    let range = max_val - min_val;

    if range == Decimal::ZERO {
        return Some(vec![Decimal::ZERO; values.len()]);
    }

    Some(values.iter().map(|&v| (v - min_val) / range).collect())
}

/// Weighted average calculation
pub fn weighted_average(values: &[Decimal], weights: &[Decimal]) -> Option<Decimal> {
    if values.len() != weights.len() || values.is_empty() {
        return None;
    }

    let weighted_sum: Decimal = values
        .iter()
        .zip(weights.iter())
        .map(|(&v, &w)| v * w)
        .sum();
    let weight_sum: Decimal = weights.iter().sum();

    if weight_sum == Decimal::ZERO {
        return None;
    }

    Some(weighted_sum / weight_sum)
}

/// Calculate geometric mean
pub fn geometric_mean(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() || values.iter().any(|&v| v <= Decimal::ZERO) {
        return None;
    }

    // For Decimal, we approximate: GM = exp(mean(log(x)))
    // Since we don't have direct log for Decimal, we use a simplified approach
    // for typical financial returns which are small percentages
    let n = Decimal::from(values.len());
    let product: Decimal = values.iter().product();

    // Approximate nth root using Newton's method
    let mut result = product / n; // Initial guess
    for _ in 0..10 {
        // Newton's method iterations
        let pow_n_minus_1 = result.powi((values.len() - 1) as i64);
        if pow_n_minus_1 == Decimal::ZERO {
            break;
        }
        result = ((n - Decimal::ONE) * result + product / pow_n_minus_1) / n;
    }

    Some(result)
}

/// Calculate harmonic mean
pub fn harmonic_mean(values: &[Decimal]) -> Option<Decimal> {
    if values.is_empty() || values.contains(&Decimal::ZERO) {
        return None;
    }

    let n = Decimal::from(values.len());
    let sum_reciprocals: Decimal = values.iter().map(|&v| Decimal::ONE / v).sum();

    Some(n / sum_reciprocals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_moments_calculator() {
        let mut calc = MomentsCalculator::new(5);
        calc.add(dec!(1.0));
        calc.add(dec!(2.0));
        calc.add(dec!(3.0));
        calc.add(dec!(4.0));
        calc.add(dec!(5.0));

        assert_eq!(calc.mean(), Some(dec!(3.0)));
        assert!(calc.variance().is_some());
        assert!(calc.std_dev().is_some());
    }

    #[test]
    fn test_correlation() {
        let x = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];
        let y = vec![dec!(2.0), dec!(4.0), dec!(6.0), dec!(8.0), dec!(10.0)];

        let corr = correlation(&x, &y).unwrap();
        assert!(corr > dec!(0.99)); // Perfect positive correlation
    }

    #[test]
    fn test_covariance() {
        let x = vec![dec!(1.0), dec!(2.0), dec!(3.0)];
        let y = vec![dec!(2.0), dec!(4.0), dec!(6.0)];

        let cov = covariance(&x, &y);
        assert!(cov.is_some());
        assert!(cov.unwrap() > Decimal::ZERO);
    }

    #[test]
    fn test_percentile() {
        let values = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];

        assert_eq!(percentile(&values, 0), Some(dec!(1.0)));
        assert_eq!(percentile(&values, 50), Some(dec!(3.0)));
        assert_eq!(percentile(&values, 100), Some(dec!(5.0)));
    }

    #[test]
    fn test_ema_calculator() {
        let mut ema = EMACalculator::from_period(3);

        let val1 = ema.update(dec!(10.0));
        assert_eq!(val1, dec!(10.0)); // First value

        let val2 = ema.update(dec!(12.0));
        assert!(val2 > dec!(10.0) && val2 < dec!(12.0));
    }

    #[test]
    fn test_linear_regression() {
        let x = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];
        let y = vec![dec!(2.0), dec!(4.0), dec!(6.0), dec!(8.0), dec!(10.0)];

        let lr = LinearRegression::fit(&x, &y).unwrap();
        assert_eq!(lr.slope, dec!(2.0));
        assert_eq!(lr.intercept, dec!(0.0));
        assert!(lr.r_squared > dec!(0.99));
    }

    #[test]
    fn test_autocorrelation() {
        let values = vec![dec!(1.0), dec!(2.0), dec!(3.0), dec!(4.0), dec!(5.0)];
        let ac = autocorrelation(&values, 1);
        assert!(ac.is_some());
    }

    #[test]
    fn test_z_score() {
        let score = z_score(dec!(15.0), dec!(10.0), dec!(5.0)).unwrap();
        assert_eq!(score, dec!(1.0));
    }

    #[test]
    fn test_min_max_normalize() {
        let values = vec![dec!(0.0), dec!(5.0), dec!(10.0)];
        let normalized = min_max_normalize(&values).unwrap();

        assert_eq!(normalized[0], dec!(0.0));
        assert_eq!(normalized[1], dec!(0.5));
        assert_eq!(normalized[2], dec!(1.0));
    }

    #[test]
    fn test_weighted_average() {
        let values = vec![dec!(10.0), dec!(20.0), dec!(30.0)];
        let weights = vec![dec!(1.0), dec!(2.0), dec!(3.0)];

        let avg = weighted_average(&values, &weights).unwrap();
        // (10*1 + 20*2 + 30*3) / (1+2+3) = 140/6 = 23.333...
        assert!(avg > dec!(23.33) && avg < dec!(23.34));
    }

    #[test]
    fn test_harmonic_mean() {
        let values = vec![dec!(1.0), dec!(2.0), dec!(4.0)];
        let hm = harmonic_mean(&values).unwrap();
        // HM = 3 / (1/1 + 1/2 + 1/4) = 3 / 1.75 = 1.714...
        assert!(hm > dec!(1.7) && hm < dec!(1.8));
    }
}
