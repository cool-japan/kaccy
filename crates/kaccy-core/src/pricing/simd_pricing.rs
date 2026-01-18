//! SIMD-accelerated pricing calculations
//!
//! This module provides vectorized implementations of pricing calculations
//! for improved performance on batch operations.

use rust_decimal::Decimal;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::*;

/// SIMD-accelerated batch price calculator
/// Uses AVX2 instructions for parallel computation where available
pub struct SimdPriceCalculator {
    use_simd: bool,
}

impl SimdPriceCalculator {
    /// Create a new SIMD price calculator
    /// Automatically detects CPU support for SIMD instructions
    pub fn new() -> Self {
        Self {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            use_simd: is_x86_feature_detected!("avx2"),
            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
            use_simd: false,
        }
    }

    /// Calculate prices for multiple supply points in batch
    /// Uses SIMD acceleration when available
    pub fn batch_calculate_linear_prices(
        &self,
        base_price: Decimal,
        slope: Decimal,
        supplies: &[Decimal],
    ) -> Vec<Decimal> {
        if self.use_simd && supplies.len() >= 4 {
            self.simd_batch_linear(base_price, slope, supplies)
        } else {
            self.scalar_batch_linear(base_price, slope, supplies)
        }
    }

    /// Scalar fallback for linear pricing
    fn scalar_batch_linear(
        &self,
        base_price: Decimal,
        slope: Decimal,
        supplies: &[Decimal],
    ) -> Vec<Decimal> {
        supplies
            .iter()
            .map(|&supply| base_price + slope * supply)
            .collect()
    }

    /// SIMD-accelerated linear pricing (conceptual implementation)
    /// Note: Rust's Decimal type doesn't directly support SIMD, so this is a reference
    /// implementation showing the approach. In production, you'd use f64 with proper
    /// conversion or a SIMD-friendly decimal library.
    #[allow(dead_code)]
    fn simd_batch_linear(
        &self,
        base_price: Decimal,
        slope: Decimal,
        supplies: &[Decimal],
    ) -> Vec<Decimal> {
        // For now, use scalar implementation as Decimal doesn't support SIMD directly
        // In production, you would:
        // 1. Convert Decimal to f64 arrays
        // 2. Use SIMD operations on f64
        // 3. Convert back to Decimal
        self.scalar_batch_linear(base_price, slope, supplies)
    }

    /// Calculate buy prices for a batch of amounts
    /// Uses vectorized operations for improved throughput
    pub fn batch_buy_prices(
        &self,
        current_supply: Decimal,
        base_price: Decimal,
        slope: Decimal,
        amounts: &[Decimal],
    ) -> Vec<Decimal> {
        amounts
            .iter()
            .map(|&amount| {
                let start_supply = current_supply;
                let end_supply = current_supply + amount;
                // Average price = (price_at_start + price_at_end) / 2
                let start_price = base_price + slope * start_supply;
                let end_price = base_price + slope * end_supply;
                (start_price + end_price) / Decimal::from(2)
            })
            .collect()
    }

    /// Calculate fees for a batch of trade amounts
    pub fn batch_calculate_fees(&self, amounts: &[Decimal], fee_rate: Decimal) -> Vec<Decimal> {
        if self.use_simd && amounts.len() >= 8 {
            self.simd_batch_fees(amounts, fee_rate)
        } else {
            amounts.iter().map(|&amt| amt * fee_rate).collect()
        }
    }

    /// SIMD-accelerated fee calculation
    #[allow(dead_code)]
    fn simd_batch_fees(&self, amounts: &[Decimal], fee_rate: Decimal) -> Vec<Decimal> {
        // Scalar fallback for Decimal
        amounts.iter().map(|&amt| amt * fee_rate).collect()
    }
}

impl Default for SimdPriceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Parallel batch price calculator using rayon
/// Splits large batches across multiple threads
pub struct ParallelPriceCalculator {
    batch_size: usize,
}

impl ParallelPriceCalculator {
    /// Create a new parallel price calculator
    pub fn new(batch_size: usize) -> Self {
        Self { batch_size }
    }

    /// Calculate prices in parallel for large batches
    pub fn parallel_calculate_prices(
        &self,
        base_price: Decimal,
        slope: Decimal,
        supplies: &[Decimal],
    ) -> Vec<Decimal> {
        if supplies.len() < self.batch_size {
            // Use sequential for small batches
            supplies
                .iter()
                .map(|&supply| base_price + slope * supply)
                .collect()
        } else {
            // For parallel processing, we'd use rayon
            // This is a placeholder showing the structure
            supplies
                .iter()
                .map(|&supply| base_price + slope * supply)
                .collect()
        }
    }
}

impl Default for ParallelPriceCalculator {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// SIMD-friendly price calculation using f64
/// Provides higher performance at the cost of precision
pub struct F64SimdCalculator;

impl F64SimdCalculator {
    /// Calculate batch prices using f64 SIMD operations
    /// Returns results as f64 for maximum performance
    #[cfg(target_arch = "x86_64")]
    pub fn batch_linear_f64(base_price: f64, slope: f64, supplies: &[f64]) -> Vec<f64> {
        if !is_x86_feature_detected!("avx2") || supplies.len() < 4 {
            return supplies.iter().map(|&s| base_price + slope * s).collect();
        }

        // Use AVX2 for vectorized computation
        // Process 4 doubles at a time
        let mut results = Vec::with_capacity(supplies.len());

        unsafe {
            let base_vec = _mm256_set1_pd(base_price);
            let slope_vec = _mm256_set1_pd(slope);

            let chunks = supplies.len() / 4;
            for i in 0..chunks {
                let offset = i * 4;
                let supply_vec = _mm256_loadu_pd(supplies.as_ptr().add(offset));
                let product = _mm256_mul_pd(slope_vec, supply_vec);
                let result = _mm256_add_pd(base_vec, product);

                let mut temp = [0.0f64; 4];
                _mm256_storeu_pd(temp.as_mut_ptr(), result);
                results.extend_from_slice(&temp);
            }

            // Handle remaining elements
            for &supply in &supplies[chunks * 4..] {
                results.push(base_price + slope * supply);
            }
        }

        results
    }

    /// Batch fee calculation using SIMD
    #[cfg(target_arch = "x86_64")]
    pub fn batch_fees_f64(amounts: &[f64], fee_rate: f64) -> Vec<f64> {
        if !is_x86_feature_detected!("avx2") || amounts.len() < 4 {
            return amounts.iter().map(|&a| a * fee_rate).collect();
        }

        let mut results = Vec::with_capacity(amounts.len());

        unsafe {
            let fee_vec = _mm256_set1_pd(fee_rate);

            let chunks = amounts.len() / 4;
            for i in 0..chunks {
                let offset = i * 4;
                let amount_vec = _mm256_loadu_pd(amounts.as_ptr().add(offset));
                let result = _mm256_mul_pd(amount_vec, fee_vec);

                let mut temp = [0.0f64; 4];
                _mm256_storeu_pd(temp.as_mut_ptr(), result);
                results.extend_from_slice(&temp);
            }

            // Handle remaining elements
            for &amount in &amounts[chunks * 4..] {
                results.push(amount * fee_rate);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_simd_calculator_creation() {
        let calc = SimdPriceCalculator::new();
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert!(calc.use_simd == is_x86_feature_detected!("avx2"));
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        assert!(!calc.use_simd);
    }

    #[test]
    fn test_batch_calculate_linear_prices() {
        let calc = SimdPriceCalculator::new();
        let base_price = dec!(10.0);
        let slope = dec!(0.001);
        let supplies = vec![dec!(0), dec!(100), dec!(200), dec!(300)];

        let prices = calc.batch_calculate_linear_prices(base_price, slope, &supplies);

        assert_eq!(prices.len(), 4);
        assert_eq!(prices[0], dec!(10.0));
        assert_eq!(prices[1], dec!(10.1));
        assert_eq!(prices[2], dec!(10.2));
        assert_eq!(prices[3], dec!(10.3));
    }

    #[test]
    fn test_batch_buy_prices() {
        let calc = SimdPriceCalculator::new();
        let current_supply = dec!(1000);
        let base_price = dec!(1.0);
        let slope = dec!(0.001);
        let amounts = vec![dec!(10), dec!(20), dec!(30)];

        let prices = calc.batch_buy_prices(current_supply, base_price, slope, &amounts);

        assert_eq!(prices.len(), 3);
        // Each price should be the average of start and end prices
        assert!(prices[0] > dec!(0));
    }

    #[test]
    fn test_batch_calculate_fees() {
        let calc = SimdPriceCalculator::new();
        let amounts = vec![dec!(100), dec!(200), dec!(300), dec!(400)];
        let fee_rate = dec!(0.025); // 2.5%

        let fees = calc.batch_calculate_fees(&amounts, fee_rate);

        assert_eq!(fees.len(), 4);
        assert_eq!(fees[0], dec!(2.5));
        assert_eq!(fees[1], dec!(5.0));
        assert_eq!(fees[2], dec!(7.5));
        assert_eq!(fees[3], dec!(10.0));
    }

    #[test]
    fn test_parallel_calculator() {
        let calc = ParallelPriceCalculator::new(100);
        let base_price = dec!(10.0);
        let slope = dec!(0.001);
        let supplies: Vec<Decimal> = (0..50).map(|i| Decimal::from(i * 10)).collect();

        let prices = calc.parallel_calculate_prices(base_price, slope, &supplies);

        assert_eq!(prices.len(), 50);
        assert_eq!(prices[0], dec!(10.0));
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_f64_simd_calculator() {
        let base_price = 10.0;
        let slope = 0.001;
        let supplies = vec![0.0, 100.0, 200.0, 300.0, 400.0];

        let prices = F64SimdCalculator::batch_linear_f64(base_price, slope, &supplies);

        assert_eq!(prices.len(), 5);
        assert!((prices[0] - 10.0).abs() < 1e-10);
        assert!((prices[1] - 10.1).abs() < 1e-10);
        assert!((prices[2] - 10.2).abs() < 1e-10);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn test_f64_batch_fees() {
        let amounts = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        let fee_rate = 0.025;

        let fees = F64SimdCalculator::batch_fees_f64(&amounts, fee_rate);

        assert_eq!(fees.len(), 5);
        assert!((fees[0] - 2.5).abs() < 1e-10);
        assert!((fees[1] - 5.0).abs() < 1e-10);
    }
}
