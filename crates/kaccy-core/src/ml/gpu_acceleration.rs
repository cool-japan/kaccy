//! GPU-Accelerated Machine Learning
//!
//! This module provides GPU acceleration for computationally intensive ML operations,
//! including matrix operations, feature extraction, and backtesting.
//!
//! NOTE: This is a framework/interface for GPU acceleration. Actual GPU implementations
//! would require CUDA/OpenCL backends which are not included to maintain portability.
//! This module provides CPU fallbacks and abstractions for future GPU integration.

use anyhow::Result;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// GPU compute backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuBackend {
    /// NVIDIA CUDA backend (requires CUDA toolkit)
    Cuda,
    /// OpenCL backend (cross-platform)
    OpenCL,
    /// CPU fallback (always available)
    Cpu,
}

impl fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuBackend::Cuda => write!(f, "CUDA"),
            GpuBackend::OpenCL => write!(f, "OpenCL"),
            GpuBackend::Cpu => write!(f, "CPU"),
        }
    }
}

/// GPU device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDeviceInfo {
    /// Human-readable device name.
    pub name: String,
    /// GPU compute backend in use.
    pub backend: GpuBackend,
    /// Available GPU memory in megabytes.
    pub memory_mb: usize,
    /// Number of compute units (shader processors / CUs).
    pub compute_units: usize,
    /// Maximum work-group size supported by the device.
    pub max_work_group_size: usize,
}

/// GPU-accelerated matrix for ML operations
#[derive(Debug, Clone)]
pub struct GpuMatrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
    backend: GpuBackend,
}

impl GpuMatrix {
    /// Create a new GPU matrix with given dimensions
    pub fn new(rows: usize, cols: usize, backend: GpuBackend) -> Self {
        Self {
            rows,
            cols,
            data: vec![0.0; rows * cols],
            backend,
        }
    }

    /// Create matrix from data
    pub fn from_data(
        rows: usize,
        cols: usize,
        data: Vec<f64>,
        backend: GpuBackend,
    ) -> Result<Self> {
        if data.len() != rows * cols {
            anyhow::bail!(
                "Data length {} doesn't match dimensions {}x{}",
                data.len(),
                rows,
                cols
            );
        }
        Ok(Self {
            rows,
            cols,
            data,
            backend,
        })
    }

    /// Get matrix dimensions
    pub fn shape(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }

    /// Get element at position
    pub fn get(&self, row: usize, col: usize) -> Result<f64> {
        if row >= self.rows || col >= self.cols {
            anyhow::bail!(
                "Index out of bounds: ({}, {}) for shape ({}, {})",
                row,
                col,
                self.rows,
                self.cols
            );
        }
        Ok(self.data[row * self.cols + col])
    }

    /// Set element at position
    pub fn set(&mut self, row: usize, col: usize, value: f64) -> Result<()> {
        if row >= self.rows || col >= self.cols {
            anyhow::bail!("Index out of bounds");
        }
        self.data[row * self.cols + col] = value;
        Ok(())
    }

    /// Matrix multiplication (GPU-accelerated when available)
    pub fn matmul(&self, other: &GpuMatrix) -> Result<GpuMatrix> {
        if self.cols != other.rows {
            anyhow::bail!(
                "Matrix dimensions incompatible for multiplication: {}x{} and {}x{}",
                self.rows,
                self.cols,
                other.rows,
                other.cols
            );
        }

        match self.backend {
            GpuBackend::Cuda => self.matmul_cuda(other),
            GpuBackend::OpenCL => self.matmul_opencl(other),
            GpuBackend::Cpu => self.matmul_cpu(other),
        }
    }

    /// CPU fallback for matrix multiplication
    fn matmul_cpu(&self, other: &GpuMatrix) -> Result<GpuMatrix> {
        let mut result = GpuMatrix::new(self.rows, other.cols, GpuBackend::Cpu);

        for i in 0..self.rows {
            for j in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.data[i * self.cols + k] * other.data[k * other.cols + j];
                }
                result.data[i * other.cols + j] = sum;
            }
        }

        Ok(result)
    }

    /// CUDA-accelerated matrix multiplication (requires CUDA feature flag).
    ///
    /// This build does not include CUDA support (`cuda` feature not enabled).
    /// Falls back to the CPU implementation, which is functionally identical
    /// but runs on the host CPU rather than a GPU device.
    fn matmul_cuda(&self, other: &GpuMatrix) -> Result<GpuMatrix> {
        tracing::debug!(
            "CUDA not available in this build; using CPU fallback for matmul ({}x{} * {}x{})",
            self.rows,
            self.cols,
            other.rows,
            other.cols
        );
        self.matmul_cpu(other)
    }

    /// OpenCL-accelerated matrix multiplication (requires OpenCL feature flag).
    ///
    /// This build does not include OpenCL support (`opencl` feature not enabled).
    /// Falls back to the CPU implementation.
    fn matmul_opencl(&self, other: &GpuMatrix) -> Result<GpuMatrix> {
        tracing::debug!(
            "OpenCL not available in this build; using CPU fallback for matmul ({}x{} * {}x{})",
            self.rows,
            self.cols,
            other.rows,
            other.cols
        );
        self.matmul_cpu(other)
    }

    /// Element-wise operations (GPU-accelerated)
    pub fn add(&self, other: &GpuMatrix) -> Result<GpuMatrix> {
        if self.rows != other.rows || self.cols != other.cols {
            anyhow::bail!("Matrix dimensions must match for addition");
        }

        let mut result = GpuMatrix::new(self.rows, self.cols, self.backend);
        for i in 0..self.data.len() {
            result.data[i] = self.data[i] + other.data[i];
        }
        Ok(result)
    }

    /// Transpose matrix
    pub fn transpose(&self) -> GpuMatrix {
        let mut result = GpuMatrix::new(self.cols, self.rows, self.backend);
        for i in 0..self.rows {
            for j in 0..self.cols {
                result.data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        result
    }

    /// Get raw data
    pub fn data(&self) -> &[f64] {
        &self.data
    }
}

/// GPU-accelerated feature extractor
pub struct GpuFeatureExtractor {
    backend: GpuBackend,
}

impl GpuFeatureExtractor {
    /// Create new GPU feature extractor
    pub fn new(backend: GpuBackend) -> Self {
        Self { backend }
    }

    /// Extract features from price data in parallel on GPU
    pub fn extract_parallel(
        &self,
        prices: &[Decimal],
        window_size: usize,
    ) -> Result<Vec<Vec<f64>>> {
        if prices.len() < window_size {
            anyhow::bail!("Not enough data for window size {}", window_size);
        }

        match self.backend {
            GpuBackend::Cuda => self.extract_cuda(prices, window_size),
            GpuBackend::OpenCL => self.extract_opencl(prices, window_size),
            GpuBackend::Cpu => self.extract_cpu(prices, window_size),
        }
    }

    fn extract_cpu(&self, prices: &[Decimal], window_size: usize) -> Result<Vec<Vec<f64>>> {
        let mut features = Vec::new();

        for i in window_size..prices.len() {
            let window = &prices[i - window_size..i];
            let mut feature_vec = Vec::new();

            // Simple moving average
            let sma = window
                .iter()
                .map(|p| p.to_string().parse::<f64>().unwrap_or(0.0))
                .sum::<f64>()
                / window_size as f64;
            feature_vec.push(sma);

            // Return
            let price_current = prices[i].to_string().parse::<f64>().unwrap_or(0.0);
            let price_prev = prices[i - 1].to_string().parse::<f64>().unwrap_or(1.0);
            let return_val = (price_current - price_prev) / price_prev;
            feature_vec.push(return_val);

            features.push(feature_vec);
        }

        Ok(features)
    }

    /// CUDA-accelerated parallel feature extraction (requires CUDA feature flag).
    ///
    /// `cuda` feature is not enabled in this build; computation runs on the CPU.
    fn extract_cuda(&self, prices: &[Decimal], window_size: usize) -> Result<Vec<Vec<f64>>> {
        tracing::debug!(
            "CUDA not available in this build; using CPU fallback for feature extraction \
             (prices={}, window={})",
            prices.len(),
            window_size
        );
        self.extract_cpu(prices, window_size)
    }

    /// OpenCL-accelerated parallel feature extraction (requires OpenCL feature flag).
    ///
    /// `opencl` feature is not enabled in this build; computation runs on the CPU.
    fn extract_opencl(&self, prices: &[Decimal], window_size: usize) -> Result<Vec<Vec<f64>>> {
        tracing::debug!(
            "OpenCL not available in this build; using CPU fallback for feature extraction \
             (prices={}, window={})",
            prices.len(),
            window_size
        );
        self.extract_cpu(prices, window_size)
    }
}

/// GPU-accelerated backtester
pub struct GpuBacktester {
    backend: GpuBackend,
}

impl GpuBacktester {
    /// Create new GPU backtester
    pub fn new(backend: GpuBackend) -> Self {
        Self { backend }
    }

    /// Run backtest on GPU for multiple parameter combinations in parallel
    pub fn backtest_parallel(
        &self,
        data: &[Decimal],
        parameter_sets: &[Vec<f64>],
    ) -> Result<Vec<BacktestResult>> {
        match self.backend {
            GpuBackend::Cuda => self.backtest_cuda(data, parameter_sets),
            GpuBackend::OpenCL => self.backtest_opencl(data, parameter_sets),
            GpuBackend::Cpu => self.backtest_cpu(data, parameter_sets),
        }
    }

    fn backtest_cpu(
        &self,
        data: &[Decimal],
        parameter_sets: &[Vec<f64>],
    ) -> Result<Vec<BacktestResult>> {
        let mut results = Vec::new();

        for params in parameter_sets {
            // Simple moving average crossover strategy
            let short_period = params.first().copied().unwrap_or(10.0) as usize;
            let long_period = params.get(1).copied().unwrap_or(30.0) as usize;

            let mut pnl = 0.0;
            let mut trades = 0;
            let mut position = 0.0;

            for i in long_period..data.len() {
                let short_sma = data[i - short_period..i]
                    .iter()
                    .map(|p| p.to_string().parse::<f64>().unwrap_or(0.0))
                    .sum::<f64>()
                    / short_period as f64;

                let long_sma = data[i - long_period..i]
                    .iter()
                    .map(|p| p.to_string().parse::<f64>().unwrap_or(0.0))
                    .sum::<f64>()
                    / long_period as f64;

                let price = data[i].to_string().parse::<f64>().unwrap_or(0.0);

                if short_sma > long_sma && position == 0.0 {
                    // Buy signal
                    position = price;
                    trades += 1;
                } else if short_sma < long_sma && position > 0.0 {
                    // Sell signal
                    pnl += price - position;
                    position = 0.0;
                    trades += 1;
                }
            }

            results.push(BacktestResult {
                parameters: params.clone(),
                total_pnl: pnl,
                num_trades: trades,
                sharpe_ratio: if trades > 0 { pnl / trades as f64 } else { 0.0 },
            });
        }

        Ok(results)
    }

    /// CUDA-accelerated parallel backtesting (requires CUDA feature flag).
    ///
    /// `cuda` feature is not enabled in this build; backtesting runs on the CPU
    /// sequentially across the provided parameter sets.
    fn backtest_cuda(
        &self,
        data: &[Decimal],
        parameter_sets: &[Vec<f64>],
    ) -> Result<Vec<BacktestResult>> {
        tracing::debug!(
            "CUDA not available in this build; using CPU fallback for backtesting \
             ({} parameter sets, {} data points)",
            parameter_sets.len(),
            data.len()
        );
        self.backtest_cpu(data, parameter_sets)
    }

    /// OpenCL-accelerated parallel backtesting (requires OpenCL feature flag).
    ///
    /// `opencl` feature is not enabled in this build; backtesting runs on the CPU.
    fn backtest_opencl(
        &self,
        data: &[Decimal],
        parameter_sets: &[Vec<f64>],
    ) -> Result<Vec<BacktestResult>> {
        tracing::debug!(
            "OpenCL not available in this build; using CPU fallback for backtesting \
             ({} parameter sets, {} data points)",
            parameter_sets.len(),
            data.len()
        );
        self.backtest_cpu(data, parameter_sets)
    }
}

/// Backtest result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    /// Strategy parameter values that produced this result.
    pub parameters: Vec<f64>,
    /// Total profit and loss over the backtest period.
    pub total_pnl: f64,
    /// Number of trades executed during the backtest.
    pub num_trades: usize,
    /// Annualized Sharpe ratio.
    pub sharpe_ratio: f64,
}

/// GPU device manager
pub struct GpuDeviceManager;

impl GpuDeviceManager {
    /// List available compute devices.
    ///
    /// In this pure-Rust build only the CPU device is reported.  CUDA and
    /// OpenCL device enumeration requires the respective feature flags
    /// (`cuda`, `opencl`) which are not enabled by default to satisfy the
    /// COOLJAPAN Pure Rust Policy.  When those features are enabled, this
    /// function would query the installed runtime and append real GPU entries.
    pub fn list_devices() -> Vec<GpuDeviceInfo> {
        // CPU fallback device is always present.
        let devices = vec![GpuDeviceInfo {
            name: "CPU".to_string(),
            backend: GpuBackend::Cpu,
            memory_mb: 0,
            compute_units: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            max_work_group_size: 1,
        }];

        // CUDA device detection: not available in this build.
        // Enable the `cuda` feature to activate real CUDA device enumeration.
        tracing::trace!("CUDA device detection skipped: `cuda` feature not enabled in this build");

        // OpenCL device detection: not available in this build.
        // Enable the `opencl` feature to activate real OpenCL device enumeration.
        tracing::trace!(
            "OpenCL device detection skipped: `opencl` feature not enabled in this build"
        );

        devices
    }

    /// Get best available device
    pub fn get_best_device() -> GpuDeviceInfo {
        let devices = Self::list_devices();

        // Prefer CUDA > OpenCL > CPU
        devices
            .into_iter()
            .min_by_key(|d| match d.backend {
                GpuBackend::Cuda => 0,
                GpuBackend::OpenCL => 1,
                GpuBackend::Cpu => 2,
            })
            .unwrap_or(GpuDeviceInfo {
                name: "CPU".to_string(),
                backend: GpuBackend::Cpu,
                memory_mb: 0,
                compute_units: std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1),
                max_work_group_size: 1,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_gpu_matrix_creation() {
        let matrix = GpuMatrix::new(3, 3, GpuBackend::Cpu);
        assert_eq!(matrix.shape(), (3, 3));
    }

    #[test]
    fn test_gpu_matrix_from_data() {
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let matrix = GpuMatrix::from_data(2, 2, data, GpuBackend::Cpu).unwrap();
        assert_eq!(matrix.get(0, 0).unwrap(), 1.0);
        assert_eq!(matrix.get(1, 1).unwrap(), 4.0);
    }

    #[test]
    fn test_matrix_multiplication() {
        let a = GpuMatrix::from_data(2, 2, vec![1.0, 2.0, 3.0, 4.0], GpuBackend::Cpu).unwrap();
        let b = GpuMatrix::from_data(2, 2, vec![5.0, 6.0, 7.0, 8.0], GpuBackend::Cpu).unwrap();
        let c = a.matmul(&b).unwrap();

        assert_eq!(c.get(0, 0).unwrap(), 19.0); // 1*5 + 2*7
        assert_eq!(c.get(0, 1).unwrap(), 22.0); // 1*6 + 2*8
        assert_eq!(c.get(1, 0).unwrap(), 43.0); // 3*5 + 4*7
        assert_eq!(c.get(1, 1).unwrap(), 50.0); // 3*6 + 4*8
    }

    #[test]
    fn test_matrix_transpose() {
        let a = GpuMatrix::from_data(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], GpuBackend::Cpu)
            .unwrap();
        let t = a.transpose();

        assert_eq!(t.shape(), (3, 2));
        assert_eq!(t.get(0, 0).unwrap(), 1.0);
        assert_eq!(t.get(1, 0).unwrap(), 2.0);
        assert_eq!(t.get(2, 1).unwrap(), 6.0);
    }

    #[test]
    fn test_matrix_addition() {
        let a = GpuMatrix::from_data(2, 2, vec![1.0, 2.0, 3.0, 4.0], GpuBackend::Cpu).unwrap();
        let b = GpuMatrix::from_data(2, 2, vec![5.0, 6.0, 7.0, 8.0], GpuBackend::Cpu).unwrap();
        let c = a.add(&b).unwrap();

        assert_eq!(c.get(0, 0).unwrap(), 6.0);
        assert_eq!(c.get(1, 1).unwrap(), 12.0);
    }

    #[test]
    fn test_gpu_feature_extractor() {
        let prices = vec![dec!(100), dec!(101), dec!(102), dec!(103), dec!(104)];
        let extractor = GpuFeatureExtractor::new(GpuBackend::Cpu);
        let features = extractor.extract_parallel(&prices, 3).unwrap();

        assert!(!features.is_empty());
        assert_eq!(features.len(), 2); // 5 - 3 = 2 feature vectors
    }

    #[test]
    fn test_gpu_backtester() {
        let data = vec![
            dec!(100),
            dec!(101),
            dec!(99),
            dec!(102),
            dec!(103),
            dec!(101),
            dec!(104),
            dec!(105),
            dec!(103),
            dec!(106),
        ];
        let backtester = GpuBacktester::new(GpuBackend::Cpu);

        let parameter_sets = vec![
            vec![2.0, 5.0], // short=2, long=5
            vec![3.0, 7.0], // short=3, long=7
        ];

        let results = backtester
            .backtest_parallel(&data, &parameter_sets)
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_device_manager() {
        let devices = GpuDeviceManager::list_devices();
        assert!(!devices.is_empty());

        let best = GpuDeviceManager::get_best_device();
        assert!(!best.name.is_empty());
    }
}
