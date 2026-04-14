//! Performance benchmarking against indices with alpha, beta, information ratio, and tracking error

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::{CoreError, Result};

/// Price point with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePoint {
    /// Time of this price observation.
    pub timestamp: DateTime<Utc>,
    /// Observed price value.
    pub price: Decimal,
}

/// Return series for a portfolio or benchmark
#[derive(Debug, Clone)]
pub struct ReturnSeries {
    /// Timestamps
    pub timestamps: Vec<DateTime<Utc>>,
    /// Returns (as decimals, e.g., 0.05 for 5%)
    pub returns: Vec<Decimal>,
}

impl ReturnSeries {
    /// Create from price points
    pub fn from_prices(prices: Vec<PricePoint>) -> Result<Self> {
        if prices.len() < 2 {
            return Err(CoreError::Validation(
                "Need at least 2 price points".to_string(),
            ));
        }

        let mut timestamps = Vec::new();
        let mut returns = Vec::new();

        for i in 1..prices.len() {
            let prev = &prices[i - 1];
            let curr = &prices[i];

            if prev.price == Decimal::ZERO {
                return Err(CoreError::Validation("Price cannot be zero".to_string()));
            }

            let ret = (curr.price - prev.price) / prev.price;
            timestamps.push(curr.timestamp);
            returns.push(ret);
        }

        Ok(Self {
            timestamps,
            returns,
        })
    }

    /// Calculate mean return
    pub fn mean(&self) -> Decimal {
        if self.returns.is_empty() {
            return Decimal::ZERO;
        }

        let sum: Decimal = self.returns.iter().sum();
        sum / Decimal::from(self.returns.len())
    }

    /// Calculate variance
    pub fn variance(&self) -> Decimal {
        if self.returns.len() < 2 {
            return Decimal::ZERO;
        }

        let mean = self.mean();
        let squared_diffs: Decimal = self.returns.iter().map(|r| (*r - mean) * (*r - mean)).sum();

        squared_diffs / Decimal::from(self.returns.len() - 1)
    }

    /// Calculate standard deviation
    pub fn std_dev(&self) -> Decimal {
        let variance = self.variance();
        // Approximation for sqrt using Newton's method
        sqrt_decimal(variance)
    }

    /// Calculate covariance with another series
    pub fn covariance(&self, other: &ReturnSeries) -> Result<Decimal> {
        if self.returns.len() != other.returns.len() {
            return Err(CoreError::Validation(
                "Return series must have same length".to_string(),
            ));
        }

        if self.returns.len() < 2 {
            return Ok(Decimal::ZERO);
        }

        let mean_self = self.mean();
        let mean_other = other.mean();

        let cov: Decimal = self
            .returns
            .iter()
            .zip(other.returns.iter())
            .map(|(r1, r2)| (*r1 - mean_self) * (*r2 - mean_other))
            .sum();

        Ok(cov / Decimal::from(self.returns.len() - 1))
    }

    /// Calculate correlation with another series
    pub fn correlation(&self, other: &ReturnSeries) -> Result<Decimal> {
        let cov = self.covariance(other)?;
        let std_self = self.std_dev();
        let std_other = other.std_dev();

        if std_self == Decimal::ZERO || std_other == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        Ok(cov / (std_self * std_other))
    }
}

/// Square root approximation using Newton's method
fn sqrt_decimal(value: Decimal) -> Decimal {
    if value == Decimal::ZERO {
        return Decimal::ZERO;
    }

    if value < Decimal::ZERO {
        return Decimal::ZERO; // Return 0 for negative values
    }

    let mut x = value;
    let two = dec!(2.0);

    // Newton's method: x_{n+1} = (x_n + value/x_n) / 2
    for _ in 0..20 {
        let x_next = (x + value / x) / two;
        if (x_next - x).abs() < dec!(0.000001) {
            break;
        }
        x = x_next;
    }

    x
}

/// Benchmark index for comparison
#[derive(Debug, Clone)]
pub struct BenchmarkIndex {
    /// Index name (e.g., "S&P 500", "Bitcoin", "Custom Index")
    pub name: String,
    /// Return series for the index
    pub returns: ReturnSeries,
}

impl BenchmarkIndex {
    /// Create from price points
    pub fn from_prices(name: String, prices: Vec<PricePoint>) -> Result<Self> {
        let returns = ReturnSeries::from_prices(prices)?;
        Ok(Self { name, returns })
    }

    /// Calculate beta relative to this benchmark
    pub fn calculate_beta(&self, portfolio: &ReturnSeries) -> Result<Decimal> {
        let cov = portfolio.covariance(&self.returns)?;
        let variance = self.returns.variance();

        if variance == Decimal::ZERO {
            return Ok(Decimal::ZERO);
        }

        Ok(cov / variance)
    }

    /// Calculate alpha relative to this benchmark
    pub fn calculate_alpha(
        &self,
        portfolio: &ReturnSeries,
        risk_free_rate: Decimal,
    ) -> Result<Decimal> {
        let beta = self.calculate_beta(portfolio)?;
        let portfolio_return = portfolio.mean();
        let benchmark_return = self.returns.mean();

        // Alpha = Portfolio Return - [Risk-Free Rate + Beta * (Benchmark Return - Risk-Free Rate)]
        let alpha =
            portfolio_return - (risk_free_rate + beta * (benchmark_return - risk_free_rate));

        Ok(alpha)
    }
}

/// Performance attribution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAttribution {
    /// Portfolio return
    pub portfolio_return: Decimal,
    /// Benchmark return
    pub benchmark_return: Decimal,
    /// Alpha (excess return)
    pub alpha: Decimal,
    /// Beta (systematic risk)
    pub beta: Decimal,
    /// Correlation with benchmark
    pub correlation: Decimal,
    /// R-squared (proportion of variance explained by benchmark)
    pub r_squared: Decimal,
    /// Active return (portfolio - benchmark)
    pub active_return: Decimal,
}

/// Information ratio calculator
#[derive(Debug, Clone)]
pub struct InformationRatioCalculator;

impl InformationRatioCalculator {
    /// Calculate information ratio
    /// IR = Active Return / Tracking Error
    pub fn calculate(
        portfolio: &ReturnSeries,
        benchmark: &ReturnSeries,
    ) -> Result<InformationRatio> {
        if portfolio.returns.len() != benchmark.returns.len() {
            return Err(CoreError::Validation(
                "Portfolio and benchmark must have same length".to_string(),
            ));
        }

        // Calculate active returns (portfolio - benchmark)
        let active_returns: Vec<Decimal> = portfolio
            .returns
            .iter()
            .zip(benchmark.returns.iter())
            .map(|(p, b)| *p - *b)
            .collect();

        let active_return_series = ReturnSeries {
            timestamps: portfolio.timestamps.clone(),
            returns: active_returns,
        };

        let active_return = active_return_series.mean();
        let tracking_error = active_return_series.std_dev();

        let information_ratio = if tracking_error == Decimal::ZERO {
            Decimal::ZERO
        } else {
            active_return / tracking_error
        };

        Ok(InformationRatio {
            active_return,
            tracking_error,
            information_ratio,
        })
    }
}

/// Information ratio result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InformationRatio {
    /// Active return (portfolio - benchmark)
    pub active_return: Decimal,
    /// Tracking error (std dev of active returns)
    pub tracking_error: Decimal,
    /// Information ratio
    pub information_ratio: Decimal,
}

/// Tracking error calculator
#[derive(Debug, Clone)]
pub struct TrackingErrorCalculator;

impl TrackingErrorCalculator {
    /// Calculate tracking error (standard deviation of active returns)
    pub fn calculate(portfolio: &ReturnSeries, benchmark: &ReturnSeries) -> Result<TrackingError> {
        if portfolio.returns.len() != benchmark.returns.len() {
            return Err(CoreError::Validation(
                "Portfolio and benchmark must have same length".to_string(),
            ));
        }

        // Calculate active returns (portfolio - benchmark)
        let active_returns: Vec<Decimal> = portfolio
            .returns
            .iter()
            .zip(benchmark.returns.iter())
            .map(|(p, b)| *p - *b)
            .collect();

        let active_return_series = ReturnSeries {
            timestamps: portfolio.timestamps.clone(),
            returns: active_returns.clone(),
        };

        let mean_active_return = active_return_series.mean();
        let tracking_error = active_return_series.std_dev();

        // Calculate max deviation
        let max_deviation = active_returns
            .iter()
            .map(|r| (*r - mean_active_return).abs())
            .max()
            .unwrap_or(Decimal::ZERO);

        // Calculate upside and downside tracking error
        let upside_returns: Vec<Decimal> = active_returns
            .iter()
            .filter(|r| **r > Decimal::ZERO)
            .copied()
            .collect();

        let downside_returns: Vec<Decimal> = active_returns
            .iter()
            .filter(|r| **r < Decimal::ZERO)
            .copied()
            .collect();

        let upside_te = if !upside_returns.is_empty() {
            let upside_series = ReturnSeries {
                timestamps: Vec::new(),
                returns: upside_returns,
            };
            upside_series.std_dev()
        } else {
            Decimal::ZERO
        };

        let downside_te = if !downside_returns.is_empty() {
            let downside_series = ReturnSeries {
                timestamps: Vec::new(),
                returns: downside_returns,
            };
            downside_series.std_dev()
        } else {
            Decimal::ZERO
        };

        Ok(TrackingError {
            tracking_error,
            mean_active_return,
            max_deviation,
            upside_tracking_error: upside_te,
            downside_tracking_error: downside_te,
        })
    }

    /// Calculate annualized tracking error
    pub fn annualized(
        portfolio: &ReturnSeries,
        benchmark: &ReturnSeries,
        periods_per_year: usize,
    ) -> Result<Decimal> {
        let te = Self::calculate(portfolio, benchmark)?;

        // Annualize: TE_annual = TE_period * sqrt(periods_per_year)
        let multiplier = sqrt_decimal(Decimal::from(periods_per_year));
        Ok(te.tracking_error * multiplier)
    }
}

/// Tracking error result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingError {
    /// Tracking error (std dev of active returns)
    pub tracking_error: Decimal,
    /// Mean active return
    pub mean_active_return: Decimal,
    /// Maximum deviation from benchmark
    pub max_deviation: Decimal,
    /// Upside tracking error (volatility of positive active returns)
    pub upside_tracking_error: Decimal,
    /// Downside tracking error (volatility of negative active returns)
    pub downside_tracking_error: Decimal,
}

/// Performance benchmark analyzer
pub struct PerformanceBenchmark {
    /// Benchmark indices
    benchmarks: BTreeMap<String, BenchmarkIndex>,
    /// Risk-free rate (annualized)
    risk_free_rate: Decimal,
}

impl PerformanceBenchmark {
    /// Create new performance benchmark analyzer
    pub fn new(risk_free_rate: Decimal) -> Self {
        Self {
            benchmarks: BTreeMap::new(),
            risk_free_rate,
        }
    }

    /// Add a benchmark index
    pub fn add_benchmark(&mut self, benchmark: BenchmarkIndex) {
        self.benchmarks.insert(benchmark.name.clone(), benchmark);
    }

    /// Analyze portfolio against a benchmark
    pub fn analyze(
        &self,
        portfolio: &ReturnSeries,
        benchmark_name: &str,
    ) -> Result<PerformanceAttribution> {
        let benchmark = self.benchmarks.get(benchmark_name).ok_or_else(|| {
            CoreError::NotFound(format!("Benchmark '{}' not found", benchmark_name))
        })?;

        let beta = benchmark.calculate_beta(portfolio)?;
        let alpha = benchmark.calculate_alpha(portfolio, self.risk_free_rate)?;
        let correlation = portfolio.correlation(&benchmark.returns)?;
        let r_squared = correlation * correlation;

        let portfolio_return = portfolio.mean();
        let benchmark_return = benchmark.returns.mean();
        let active_return = portfolio_return - benchmark_return;

        Ok(PerformanceAttribution {
            portfolio_return,
            benchmark_return,
            alpha,
            beta,
            correlation,
            r_squared,
            active_return,
        })
    }

    /// Calculate information ratio against a benchmark
    pub fn information_ratio(
        &self,
        portfolio: &ReturnSeries,
        benchmark_name: &str,
    ) -> Result<InformationRatio> {
        let benchmark = self.benchmarks.get(benchmark_name).ok_or_else(|| {
            CoreError::NotFound(format!("Benchmark '{}' not found", benchmark_name))
        })?;

        InformationRatioCalculator::calculate(portfolio, &benchmark.returns)
    }

    /// Calculate tracking error against a benchmark
    pub fn tracking_error(
        &self,
        portfolio: &ReturnSeries,
        benchmark_name: &str,
    ) -> Result<TrackingError> {
        let benchmark = self.benchmarks.get(benchmark_name).ok_or_else(|| {
            CoreError::NotFound(format!("Benchmark '{}' not found", benchmark_name))
        })?;

        TrackingErrorCalculator::calculate(portfolio, &benchmark.returns)
    }

    /// Comprehensive analysis against all benchmarks
    pub fn comprehensive_analysis(
        &self,
        portfolio: &ReturnSeries,
    ) -> Result<
        Vec<(
            String,
            PerformanceAttribution,
            InformationRatio,
            TrackingError,
        )>,
    > {
        let mut results = Vec::new();

        for name in self.benchmarks.keys() {
            let attribution = self.analyze(portfolio, name)?;
            let ir = self.information_ratio(portfolio, name)?;
            let te = self.tracking_error(portfolio, name)?;

            results.push((name.clone(), attribution, ir, te));
        }

        Ok(results)
    }

    /// Get list of available benchmarks
    pub fn available_benchmarks(&self) -> Vec<String> {
        self.benchmarks.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_test_prices() -> Vec<PricePoint> {
        vec![
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(100.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(105.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(103.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(108.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(107.0),
            },
        ]
    }

    #[test]
    fn test_return_series_from_prices() {
        let prices = create_test_prices();
        let returns = ReturnSeries::from_prices(prices).unwrap();

        assert_eq!(returns.returns.len(), 4);
        // First return: (105 - 100) / 100 = 0.05
        assert_eq!(returns.returns[0], dec!(0.05));
    }

    #[test]
    fn test_return_series_mean() {
        let returns = ReturnSeries {
            timestamps: vec![Utc::now(); 3],
            returns: vec![dec!(0.05), dec!(0.02), dec!(-0.01)],
        };

        let mean = returns.mean();
        assert!((mean - dec!(0.02)).abs() < dec!(0.0001));
    }

    #[test]
    fn test_return_series_std_dev() {
        let returns = ReturnSeries {
            timestamps: vec![Utc::now(); 3],
            returns: vec![dec!(0.05), dec!(0.02), dec!(-0.01)],
        };

        let std_dev = returns.std_dev();
        assert!(std_dev > Decimal::ZERO);
    }

    #[test]
    fn test_return_series_covariance() {
        let returns1 = ReturnSeries {
            timestamps: vec![Utc::now(); 3],
            returns: vec![dec!(0.05), dec!(0.02), dec!(-0.01)],
        };

        let returns2 = ReturnSeries {
            timestamps: vec![Utc::now(); 3],
            returns: vec![dec!(0.04), dec!(0.03), dec!(0.01)],
        };

        let cov = returns1.covariance(&returns2).unwrap();
        assert!(cov != Decimal::ZERO);
    }

    #[test]
    fn test_benchmark_beta() {
        let benchmark_prices = create_test_prices();
        let benchmark =
            BenchmarkIndex::from_prices("Test Index".to_string(), benchmark_prices).unwrap();

        let portfolio_prices = vec![
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(100.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(110.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(106.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(116.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(114.0),
            },
        ];

        let portfolio = ReturnSeries::from_prices(portfolio_prices).unwrap();
        let beta = benchmark.calculate_beta(&portfolio).unwrap();

        assert!(beta > Decimal::ZERO);
    }

    #[test]
    fn test_benchmark_alpha() {
        let benchmark_prices = create_test_prices();
        let benchmark =
            BenchmarkIndex::from_prices("Test Index".to_string(), benchmark_prices).unwrap();

        let portfolio_prices = vec![
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(100.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(110.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(106.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(116.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(114.0),
            },
        ];

        let portfolio = ReturnSeries::from_prices(portfolio_prices).unwrap();
        let _alpha = benchmark.calculate_alpha(&portfolio, dec!(0.02)).unwrap();

        // Alpha can be positive or negative - we just verify the calculation succeeds
    }

    #[test]
    fn test_information_ratio() {
        let portfolio = ReturnSeries {
            timestamps: vec![Utc::now(); 5],
            returns: vec![dec!(0.05), dec!(0.02), dec!(-0.01), dec!(0.03), dec!(0.01)],
        };

        let benchmark = ReturnSeries {
            timestamps: vec![Utc::now(); 5],
            returns: vec![dec!(0.04), dec!(0.03), dec!(0.01), dec!(0.02), dec!(0.02)],
        };

        let ir = InformationRatioCalculator::calculate(&portfolio, &benchmark).unwrap();

        assert!(ir.tracking_error >= Decimal::ZERO);
        assert!(ir.information_ratio != Decimal::ZERO || ir.tracking_error == Decimal::ZERO);
    }

    #[test]
    fn test_tracking_error() {
        let portfolio = ReturnSeries {
            timestamps: vec![Utc::now(); 5],
            returns: vec![dec!(0.05), dec!(0.02), dec!(-0.01), dec!(0.03), dec!(0.01)],
        };

        let benchmark = ReturnSeries {
            timestamps: vec![Utc::now(); 5],
            returns: vec![dec!(0.04), dec!(0.03), dec!(0.01), dec!(0.02), dec!(0.02)],
        };

        let te = TrackingErrorCalculator::calculate(&portfolio, &benchmark).unwrap();

        assert!(te.tracking_error >= Decimal::ZERO);
        assert!(te.max_deviation >= Decimal::ZERO);
    }

    #[test]
    fn test_performance_benchmark() {
        let mut benchmark_analyzer = PerformanceBenchmark::new(dec!(0.02));

        let benchmark_prices = create_test_prices();
        let benchmark =
            BenchmarkIndex::from_prices("S&P 500".to_string(), benchmark_prices).unwrap();
        benchmark_analyzer.add_benchmark(benchmark);

        let portfolio_prices = vec![
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(100.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(110.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(106.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(116.0),
            },
            PricePoint {
                timestamp: Utc::now(),
                price: dec!(114.0),
            },
        ];

        let portfolio = ReturnSeries::from_prices(portfolio_prices).unwrap();
        let attribution = benchmark_analyzer.analyze(&portfolio, "S&P 500").unwrap();

        assert!(attribution.beta != Decimal::ZERO);
        assert!(attribution.r_squared >= Decimal::ZERO);
        assert!(attribution.r_squared <= dec!(1.0));
    }

    #[test]
    fn test_comprehensive_analysis() {
        let mut benchmark_analyzer = PerformanceBenchmark::new(dec!(0.02));

        let benchmark1 =
            BenchmarkIndex::from_prices("Index 1".to_string(), create_test_prices()).unwrap();
        let benchmark2 =
            BenchmarkIndex::from_prices("Index 2".to_string(), create_test_prices()).unwrap();

        benchmark_analyzer.add_benchmark(benchmark1);
        benchmark_analyzer.add_benchmark(benchmark2);

        let portfolio = ReturnSeries::from_prices(create_test_prices()).unwrap();
        let results = benchmark_analyzer
            .comprehensive_analysis(&portfolio)
            .unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_sqrt_decimal() {
        // Test with approximate comparison due to decimal precision
        let sqrt_4 = sqrt_decimal(dec!(4.0));
        assert!((sqrt_4 - dec!(2.0)).abs() < dec!(0.0001));

        let sqrt_9 = sqrt_decimal(dec!(9.0));
        assert!((sqrt_9 - dec!(3.0)).abs() < dec!(0.0001));

        let sqrt_2 = sqrt_decimal(dec!(2.0));
        assert!(sqrt_2 > dec!(1.4));
        assert!(sqrt_2 < dec!(1.5));
    }
}
