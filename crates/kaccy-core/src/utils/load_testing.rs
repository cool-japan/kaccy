//! Load testing and performance benchmarking utilities
//!
//! This module provides:
//! - Concurrent user simulation
//! - Throughput benchmarking
//! - Latency measurement
//! - Stress testing scenarios

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::time::Instant;

use crate::error::Result;

/// Load test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestConfig {
    /// Number of concurrent users
    pub concurrent_users: usize,
    /// Duration of the test
    pub duration_seconds: u64,
    /// Ramp-up time (gradual increase of users)
    pub ramp_up_seconds: u64,
    /// Think time between requests (in milliseconds)
    pub think_time_ms: u64,
}

impl LoadTestConfig {
    /// Create a default configuration
    pub fn default_config() -> Self {
        Self {
            concurrent_users: 10,
            duration_seconds: 60,
            ramp_up_seconds: 10,
            think_time_ms: 1000,
        }
    }

    /// Create a stress test configuration (high load)
    pub fn stress_test() -> Self {
        Self {
            concurrent_users: 1000,
            duration_seconds: 300,
            ramp_up_seconds: 30,
            think_time_ms: 100,
        }
    }

    /// Create a spike test configuration (sudden load increase)
    pub fn spike_test() -> Self {
        Self {
            concurrent_users: 500,
            duration_seconds: 120,
            ramp_up_seconds: 5, // Very fast ramp-up
            think_time_ms: 500,
        }
    }

    /// Create a soak test configuration (sustained load)
    pub fn soak_test() -> Self {
        Self {
            concurrent_users: 50,
            duration_seconds: 3600, // 1 hour
            ramp_up_seconds: 60,
            think_time_ms: 2000,
        }
    }
}

/// Request result for tracking
#[derive(Debug, Clone)]
struct RequestResult {
    success: bool,
    latency_ms: u64,
    #[allow(dead_code)]
    timestamp: DateTime<Utc>,
    #[allow(dead_code)]
    error: Option<String>,
}

/// Load test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResults {
    /// Total number of requests
    pub total_requests: usize,
    /// Number of successful requests
    pub successful_requests: usize,
    /// Number of failed requests
    pub failed_requests: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average latency in milliseconds
    pub average_latency_ms: f64,
    /// Minimum latency in milliseconds
    pub min_latency_ms: u64,
    /// Maximum latency in milliseconds
    pub max_latency_ms: u64,
    /// 50th percentile (median) latency
    pub p50_latency_ms: u64,
    /// 95th percentile latency
    pub p95_latency_ms: u64,
    /// 99th percentile latency
    pub p99_latency_ms: u64,
    /// Requests per second (throughput)
    pub requests_per_second: f64,
    /// Test duration in seconds
    pub duration_seconds: f64,
}

impl LoadTestResults {
    /// Calculate results from request data
    fn from_results(results: Vec<RequestResult>, duration: std::time::Duration) -> Self {
        let total_requests = results.len();
        let successful_requests = results.iter().filter(|r| r.success).count();
        let failed_requests = total_requests - successful_requests;

        let success_rate = if total_requests > 0 {
            successful_requests as f64 / total_requests as f64
        } else {
            0.0
        };

        let mut latencies: Vec<u64> = results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();

        let average_latency_ms = if !latencies.is_empty() {
            latencies.iter().sum::<u64>() as f64 / latencies.len() as f64
        } else {
            0.0
        };

        let min_latency_ms = latencies.first().copied().unwrap_or(0);
        let max_latency_ms = latencies.last().copied().unwrap_or(0);

        let p50_latency_ms = percentile(&latencies, 50);
        let p95_latency_ms = percentile(&latencies, 95);
        let p99_latency_ms = percentile(&latencies, 99);

        let duration_seconds = duration.as_secs_f64();
        let requests_per_second = if duration_seconds > 0.0 {
            total_requests as f64 / duration_seconds
        } else {
            0.0
        };

        Self {
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            average_latency_ms,
            min_latency_ms,
            max_latency_ms,
            p50_latency_ms,
            p95_latency_ms,
            p99_latency_ms,
            requests_per_second,
            duration_seconds,
        }
    }
}

/// Calculate percentile from sorted data
fn percentile(sorted_data: &[u64], p: usize) -> u64 {
    if sorted_data.is_empty() {
        return 0;
    }

    let index = (sorted_data.len() * p / 100).min(sorted_data.len() - 1);
    sorted_data[index]
}

/// Virtual user for load testing
struct VirtualUser {
    #[allow(dead_code)]
    id: usize,
    results: Vec<RequestResult>,
}

impl VirtualUser {
    fn new(id: usize) -> Self {
        Self {
            id,
            results: Vec::new(),
        }
    }

    async fn execute_request<F, Fut>(&mut self, request_fn: &F) -> Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let start = Instant::now();

        let result = request_fn().await;

        let latency_ms = start.elapsed().as_millis() as u64;

        self.results.push(RequestResult {
            success: result.is_ok(),
            latency_ms,
            timestamp: Utc::now(),
            error: result.err().map(|e| e.to_string()),
        });

        Ok(())
    }
}

/// Load test executor
pub struct LoadTester {
    config: LoadTestConfig,
    results: Arc<RwLock<Vec<RequestResult>>>,
}

impl LoadTester {
    /// Create a new load tester
    pub fn new(config: LoadTestConfig) -> Self {
        Self {
            config,
            results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Run load test with given request function
    pub async fn run<F, Fut>(&self, request_fn: F) -> LoadTestResults
    where
        F: Fn() -> Fut + Send + Sync + 'static + Clone,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let start_time = Instant::now();
        let test_duration = std::time::Duration::from_secs(self.config.duration_seconds);
        let ramp_up_duration = std::time::Duration::from_secs(self.config.ramp_up_seconds);
        let think_time = std::time::Duration::from_millis(self.config.think_time_ms);

        let mut handles = Vec::new();

        // Spawn virtual users with ramp-up
        for user_id in 0..self.config.concurrent_users {
            let request_fn_clone = request_fn.clone();
            let results_clone = self.results.clone();
            let test_duration_clone = test_duration;
            let think_time_clone = think_time;

            // Calculate ramp-up delay for this user
            let ramp_up_delay = if self.config.ramp_up_seconds > 0 {
                ramp_up_duration.mul_f64(user_id as f64 / self.config.concurrent_users as f64)
            } else {
                std::time::Duration::from_secs(0)
            };

            let handle = tokio::spawn(async move {
                // Wait for ramp-up delay
                if ramp_up_delay > std::time::Duration::from_secs(0) {
                    tokio::time::sleep(ramp_up_delay).await;
                }

                let mut user = VirtualUser::new(user_id);
                let user_start = Instant::now();

                // Execute requests until test duration expires
                while user_start.elapsed() < test_duration_clone {
                    let _ = user.execute_request(&request_fn_clone).await;

                    // Think time between requests
                    if think_time_clone > std::time::Duration::from_secs(0) {
                        tokio::time::sleep(think_time_clone).await;
                    }
                }

                // Store results
                let mut all_results = results_clone.write().unwrap_or_else(|e| e.into_inner());
                all_results.extend(user.results);
            });

            handles.push(handle);
        }

        // Wait for all users to complete
        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start_time.elapsed();

        // Collect and analyze results
        let results = self
            .results
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        LoadTestResults::from_results(results, elapsed)
    }

    /// Run throughput test (measure max requests per second)
    pub async fn measure_throughput<F, Fut>(&self, request_fn: F, duration_seconds: u64) -> f64
    where
        F: Fn() -> Fut + Send + Sync + 'static + Clone,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let start_time = Instant::now();
        let test_duration = std::time::Duration::from_secs(duration_seconds);

        let request_count = Arc::new(RwLock::new(0usize));

        let mut handles = Vec::new();

        // Spawn many concurrent tasks
        for _ in 0..1000 {
            let request_fn_clone = request_fn.clone();
            let count_clone = request_count.clone();

            let handle = tokio::spawn(async move {
                let task_start = Instant::now();

                while task_start.elapsed() < test_duration {
                    if request_fn_clone().await.is_ok() {
                        let mut count = count_clone.write().unwrap_or_else(|e| e.into_inner());
                        *count += 1;
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all tasks
        for handle in handles {
            let _ = handle.await;
        }

        let elapsed = start_time.elapsed().as_secs_f64();
        let total_requests = *request_count.read().unwrap_or_else(|e| e.into_inner());

        if elapsed > 0.0 {
            total_requests as f64 / elapsed
        } else {
            0.0
        }
    }
}

/// Stress test scenario
#[derive(Debug, Clone)]
pub struct StressTestScenario {
    /// Scenario name
    pub name: String,
    /// Initial concurrent users
    pub initial_users: usize,
    /// Maximum concurrent users
    pub max_users: usize,
    /// Step size for increasing users
    pub step_size: usize,
    /// Duration at each step (seconds)
    pub step_duration_seconds: u64,
}

impl StressTestScenario {
    /// Create a gradual ramp-up scenario
    pub fn gradual_ramp_up() -> Self {
        Self {
            name: "Gradual Ramp-Up".to_string(),
            initial_users: 10,
            max_users: 1000,
            step_size: 50,
            step_duration_seconds: 60,
        }
    }

    /// Create a spike scenario
    pub fn spike() -> Self {
        Self {
            name: "Spike Test".to_string(),
            initial_users: 10,
            max_users: 500,
            step_size: 490, // Jump directly
            step_duration_seconds: 120,
        }
    }
}

/// Stress test executor
pub struct StressTester;

impl StressTester {
    /// Run stress test with scenario
    pub async fn run_scenario<F, Fut>(
        scenario: StressTestScenario,
        request_fn: F,
    ) -> Vec<(usize, LoadTestResults)>
    where
        F: Fn() -> Fut + Send + Sync + 'static + Clone,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let mut results = Vec::new();
        let mut current_users = scenario.initial_users;

        while current_users <= scenario.max_users {
            let config = LoadTestConfig {
                concurrent_users: current_users,
                duration_seconds: scenario.step_duration_seconds,
                ramp_up_seconds: 5,
                think_time_ms: 1000,
            };

            let tester = LoadTester::new(config);
            let test_results = tester.run(request_fn.clone()).await;

            results.push((current_users, test_results));

            current_users += scenario.step_size;
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    async fn mock_request() -> Result<()> {
        // Simulate some work
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(())
    }

    async fn mock_failing_request() -> Result<()> {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Err(CoreError::Validation("Test error".to_string()))
    }

    #[tokio::test]
    async fn test_load_test_config() {
        let config = LoadTestConfig::default_config();
        assert_eq!(config.concurrent_users, 10);
        assert_eq!(config.duration_seconds, 60);
    }

    #[tokio::test]
    async fn test_stress_test_config() {
        let config = LoadTestConfig::stress_test();
        assert_eq!(config.concurrent_users, 1000);
    }

    #[tokio::test]
    async fn test_load_tester() {
        let config = LoadTestConfig {
            concurrent_users: 5,
            duration_seconds: 2,
            ramp_up_seconds: 0,
            think_time_ms: 100,
        };

        let tester = LoadTester::new(config);
        let results = tester.run(mock_request).await;

        assert!(results.total_requests > 0);
        assert!(results.success_rate > 0.0);
        assert!(results.requests_per_second > 0.0);
    }

    #[tokio::test]
    async fn test_load_tester_with_failures() {
        let config = LoadTestConfig {
            concurrent_users: 3,
            duration_seconds: 1,
            ramp_up_seconds: 0,
            think_time_ms: 100,
        };

        let tester = LoadTester::new(config);
        let results = tester.run(mock_failing_request).await;

        assert!(results.total_requests > 0);
        assert_eq!(results.success_rate, 0.0);
        assert!(results.failed_requests > 0);
    }

    #[tokio::test]
    async fn test_percentile_calculation() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let p50 = percentile(&data, 50);
        assert!((5..=6).contains(&p50));

        let p95 = percentile(&data, 95);
        assert!(p95 >= 9);
    }

    #[tokio::test]
    async fn test_throughput_measurement() {
        let config = LoadTestConfig {
            concurrent_users: 10,
            duration_seconds: 2,
            ramp_up_seconds: 0,
            think_time_ms: 0,
        };

        let tester = LoadTester::new(config);
        let throughput = tester.measure_throughput(mock_request, 1).await;

        assert!(throughput > 0.0);
    }

    #[test]
    fn test_stress_test_scenario() {
        let scenario = StressTestScenario::gradual_ramp_up();
        assert_eq!(scenario.initial_users, 10);
        assert_eq!(scenario.max_users, 1000);
        assert_eq!(scenario.step_size, 50);
    }

    #[test]
    fn test_spike_scenario() {
        let scenario = StressTestScenario::spike();
        assert_eq!(scenario.step_size, 490);
    }
}
