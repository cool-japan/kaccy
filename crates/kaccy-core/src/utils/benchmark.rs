//! Benchmarking utilities for performance testing and profiling

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

/// Benchmark result for a single operation
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Name of the benchmark
    pub name: String,
    /// Number of iterations
    pub iterations: usize,
    /// Total duration
    pub total_duration: Duration,
    /// Average duration per iteration
    pub avg_duration: Duration,
    /// Minimum duration
    pub min_duration: Duration,
    /// Maximum duration
    pub max_duration: Duration,
    /// Standard deviation
    pub std_dev: Duration,
    /// Operations per second
    pub ops_per_sec: f64,
}

impl BenchmarkResult {
    /// Create a new benchmark result
    pub fn new(name: String, iterations: usize, durations: Vec<Duration>) -> Self {
        let total_duration: Duration = durations.iter().sum();
        let avg_duration = total_duration / iterations as u32;
        let min_duration = durations.iter().min().copied().unwrap_or(Duration::ZERO);
        let max_duration = durations.iter().max().copied().unwrap_or(Duration::ZERO);

        // Calculate standard deviation
        let avg_nanos = avg_duration.as_nanos() as f64;
        let variance: f64 = durations
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>()
            / iterations as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);

        // Calculate operations per second
        let ops_per_sec = if avg_duration.as_secs_f64() > 0.0 {
            1.0 / avg_duration.as_secs_f64()
        } else {
            0.0
        };

        Self {
            name,
            iterations,
            total_duration,
            avg_duration,
            min_duration,
            max_duration,
            std_dev,
            ops_per_sec,
        }
    }

    /// Format duration in human-readable form
    fn format_duration(&self, duration: Duration) -> String {
        let nanos = duration.as_nanos();
        if nanos < 1_000 {
            format!("{}ns", nanos)
        } else if nanos < 1_000_000 {
            format!("{:.2}μs", nanos as f64 / 1_000.0)
        } else if nanos < 1_000_000_000 {
            format!("{:.2}ms", nanos as f64 / 1_000_000.0)
        } else {
            format!("{:.2}s", nanos as f64 / 1_000_000_000.0)
        }
    }
}

impl fmt::Display for BenchmarkResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Benchmark: {}", self.name)?;
        writeln!(f, "  Iterations: {}", self.iterations)?;
        writeln!(
            f,
            "  Total time: {}",
            self.format_duration(self.total_duration)
        )?;
        writeln!(
            f,
            "  Average:    {}",
            self.format_duration(self.avg_duration)
        )?;
        writeln!(
            f,
            "  Min:        {}",
            self.format_duration(self.min_duration)
        )?;
        writeln!(
            f,
            "  Max:        {}",
            self.format_duration(self.max_duration)
        )?;
        writeln!(f, "  Std dev:    {}", self.format_duration(self.std_dev))?;
        writeln!(f, "  Ops/sec:    {:.2}", self.ops_per_sec)?;
        Ok(())
    }
}

/// Benchmark runner for executing performance tests
pub struct Benchmark {
    /// Name of the benchmark
    name: String,
    /// Number of iterations (default: 1000)
    iterations: usize,
    /// Warmup iterations (default: 10)
    warmup_iterations: usize,
    /// Whether to run warmup (default: true)
    warmup: bool,
}

impl Benchmark {
    /// Create a new benchmark
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            iterations: 1000,
            warmup_iterations: 10,
            warmup: true,
        }
    }

    /// Set number of iterations
    pub fn iterations(mut self, iterations: usize) -> Self {
        self.iterations = iterations;
        self
    }

    /// Set number of warmup iterations
    pub fn warmup_iterations(mut self, warmup_iterations: usize) -> Self {
        self.warmup_iterations = warmup_iterations;
        self
    }

    /// Enable or disable warmup
    pub fn warmup(mut self, warmup: bool) -> Self {
        self.warmup = warmup;
        self
    }

    /// Run a synchronous benchmark
    pub fn run<F>(self, mut f: F) -> BenchmarkResult
    where
        F: FnMut(),
    {
        // Warmup
        if self.warmup {
            for _ in 0..self.warmup_iterations {
                f();
            }
        }

        // Actual benchmark
        let mut durations = Vec::with_capacity(self.iterations);
        for _ in 0..self.iterations {
            let start = Instant::now();
            f();
            durations.push(start.elapsed());
        }

        BenchmarkResult::new(self.name, self.iterations, durations)
    }

    /// Run an async benchmark
    pub async fn run_async<F, Fut>(self, mut f: F) -> BenchmarkResult
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        // Warmup
        if self.warmup {
            for _ in 0..self.warmup_iterations {
                f().await;
            }
        }

        // Actual benchmark
        let mut durations = Vec::with_capacity(self.iterations);
        for _ in 0..self.iterations {
            let start = Instant::now();
            f().await;
            durations.push(start.elapsed());
        }

        BenchmarkResult::new(self.name, self.iterations, durations)
    }
}

/// Benchmark suite for running multiple benchmarks
pub struct BenchmarkSuite {
    /// Name of the suite
    name: String,
    /// Benchmark results
    results: Vec<BenchmarkResult>,
}

impl BenchmarkSuite {
    /// Create a new benchmark suite
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            results: Vec::new(),
        }
    }

    /// Add a benchmark result
    pub fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// Get all results
    pub fn results(&self) -> &[BenchmarkResult] {
        &self.results
    }

    /// Get the fastest benchmark
    pub fn fastest(&self) -> Option<&BenchmarkResult> {
        self.results
            .iter()
            .min_by(|a, b| a.avg_duration.cmp(&b.avg_duration))
    }

    /// Get the slowest benchmark
    pub fn slowest(&self) -> Option<&BenchmarkResult> {
        self.results
            .iter()
            .max_by(|a, b| a.avg_duration.cmp(&b.avg_duration))
    }

    /// Print summary
    pub fn print_summary(&self) {
        println!("Benchmark Suite: {}", self.name);
        println!("{}", "=".repeat(80));

        for result in &self.results {
            println!("{}", result);
        }

        if let Some(fastest) = self.fastest() {
            println!("Fastest: {}", fastest.name);
        }
        if let Some(slowest) = self.slowest() {
            println!("Slowest: {}", slowest.name);
        }
    }
}

/// Load testing helper for simulating concurrent requests
pub struct LoadTest {
    /// Name of the load test
    name: String,
    /// Number of concurrent users
    concurrent_users: usize,
    /// Number of requests per user
    requests_per_user: usize,
    /// Ramp-up time (time to reach full concurrency)
    ramp_up: Duration,
}

impl LoadTest {
    /// Create a new load test
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            concurrent_users: 10,
            requests_per_user: 100,
            ramp_up: Duration::from_secs(0),
        }
    }

    /// Set number of concurrent users
    pub fn concurrent_users(mut self, users: usize) -> Self {
        self.concurrent_users = users;
        self
    }

    /// Set number of requests per user
    pub fn requests_per_user(mut self, requests: usize) -> Self {
        self.requests_per_user = requests;
        self
    }

    /// Set ramp-up time
    pub fn ramp_up(mut self, duration: Duration) -> Self {
        self.ramp_up = duration;
        self
    }

    /// Run the load test
    pub async fn run<F, Fut>(self, f: F) -> LoadTestResult
    where
        F: Fn() -> Fut + Send + Sync + 'static + Clone,
        Fut: std::future::Future<Output = Result<(), String>> + Send,
    {
        let start_time = Instant::now();
        let total_requests = self.concurrent_users * self.requests_per_user;
        let mut handles = Vec::new();

        let ramp_delay = if self.concurrent_users > 0 {
            self.ramp_up / self.concurrent_users as u32
        } else {
            Duration::ZERO
        };

        for _user_id in 0..self.concurrent_users {
            let f = f.clone();
            let requests = self.requests_per_user;

            let handle = tokio::spawn(async move {
                let mut user_results = Vec::new();

                for _ in 0..requests {
                    let start = Instant::now();
                    let result = f().await;
                    let duration = start.elapsed();

                    user_results.push((result.is_ok(), duration));
                }

                user_results
            });

            handles.push(handle);

            // Ramp-up delay
            if ramp_delay > Duration::ZERO {
                tokio::time::sleep(ramp_delay).await;
            }
        }

        // Collect results
        let mut all_results = Vec::new();
        for handle in handles {
            if let Ok(user_results) = handle.await {
                all_results.extend(user_results);
            }
        }

        let total_duration = start_time.elapsed();
        let successful = all_results.iter().filter(|(ok, _)| *ok).count();
        let failed = all_results.len() - successful;

        let durations: Vec<Duration> = all_results.iter().map(|(_, d)| *d).collect();
        let avg_duration = if !durations.is_empty() {
            durations.iter().sum::<Duration>() / durations.len() as u32
        } else {
            Duration::ZERO
        };

        let min_duration = durations.iter().min().copied().unwrap_or(Duration::ZERO);
        let max_duration = durations.iter().max().copied().unwrap_or(Duration::ZERO);

        let throughput = if total_duration.as_secs_f64() > 0.0 {
            successful as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        LoadTestResult {
            name: self.name,
            concurrent_users: self.concurrent_users,
            total_requests,
            successful_requests: successful,
            failed_requests: failed,
            total_duration,
            avg_response_time: avg_duration,
            min_response_time: min_duration,
            max_response_time: max_duration,
            throughput,
        }
    }
}

/// Load test result
#[derive(Debug, Clone)]
pub struct LoadTestResult {
    /// Name of the load test
    pub name: String,
    /// Number of concurrent users
    pub concurrent_users: usize,
    /// Total number of requests
    pub total_requests: usize,
    /// Number of successful requests
    pub successful_requests: usize,
    /// Number of failed requests
    pub failed_requests: usize,
    /// Total duration
    pub total_duration: Duration,
    /// Average response time
    pub avg_response_time: Duration,
    /// Minimum response time
    pub min_response_time: Duration,
    /// Maximum response time
    pub max_response_time: Duration,
    /// Throughput (requests per second)
    pub throughput: f64,
}

impl LoadTestResult {
    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.successful_requests as f64 / self.total_requests as f64) * 100.0
        }
    }
}

impl fmt::Display for LoadTestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Load Test: {}", self.name)?;
        writeln!(f, "  Concurrent users: {}", self.concurrent_users)?;
        writeln!(f, "  Total requests:   {}", self.total_requests)?;
        writeln!(
            f,
            "  Successful:       {} ({:.2}%)",
            self.successful_requests,
            self.success_rate()
        )?;
        writeln!(f, "  Failed:           {}", self.failed_requests)?;
        writeln!(f, "  Total duration:   {:?}", self.total_duration)?;
        writeln!(f, "  Avg response:     {:?}", self.avg_response_time)?;
        writeln!(f, "  Min response:     {:?}", self.min_response_time)?;
        writeln!(f, "  Max response:     {:?}", self.max_response_time)?;
        writeln!(f, "  Throughput:       {:.2} req/s", self.throughput)?;
        Ok(())
    }
}

/// Profiler for tracking execution time of code sections
pub struct Profiler {
    /// Profiler name
    name: String,
    /// Sections and their durations
    sections: HashMap<String, Vec<Duration>>,
    /// Current section start time
    current_section: Option<(String, Instant)>,
}

impl Profiler {
    /// Create a new profiler
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sections: HashMap::new(),
            current_section: None,
        }
    }

    /// Start profiling a section
    pub fn start_section(&mut self, name: impl Into<String>) {
        if let Some((prev_name, prev_start)) = self.current_section.take() {
            // End previous section
            let duration = prev_start.elapsed();
            self.sections.entry(prev_name).or_default().push(duration);
        }

        self.current_section = Some((name.into(), Instant::now()));
    }

    /// End the current section
    pub fn end_section(&mut self) {
        if let Some((name, start)) = self.current_section.take() {
            let duration = start.elapsed();
            self.sections.entry(name).or_default().push(duration);
        }
    }

    /// Get profiling results
    pub fn results(&self) -> ProfilerResults {
        let mut section_results = HashMap::new();

        for (name, durations) in &self.sections {
            let total: Duration = durations.iter().sum();
            let avg = total / durations.len() as u32;
            let count = durations.len();

            section_results.insert(
                name.clone(),
                SectionProfile {
                    name: name.clone(),
                    count,
                    total_duration: total,
                    avg_duration: avg,
                },
            );
        }

        ProfilerResults {
            name: self.name.clone(),
            sections: section_results,
        }
    }

    /// Print profiling results
    pub fn print_results(&self) {
        let results = self.results();
        println!("{}", results);
    }
}

/// Profiler results
#[derive(Debug, Clone)]
pub struct ProfilerResults {
    /// Profiler name
    pub name: String,
    /// Section profiles
    pub sections: HashMap<String, SectionProfile>,
}

impl fmt::Display for ProfilerResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Profiler: {}", self.name)?;
        writeln!(f, "{}", "=".repeat(80))?;

        let mut sections: Vec<_> = self.sections.values().collect();
        sections.sort_by_key(|b| std::cmp::Reverse(b.total_duration));

        for section in sections {
            writeln!(f, "{}", section)?;
        }

        Ok(())
    }
}

/// Section profile
#[derive(Debug, Clone)]
pub struct SectionProfile {
    /// Section name
    pub name: String,
    /// Number of executions
    pub count: usize,
    /// Total duration
    pub total_duration: Duration,
    /// Average duration
    pub avg_duration: Duration,
}

impl fmt::Display for SectionProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  {}", self.name)?;
        writeln!(f, "    Count:   {}", self.count)?;
        writeln!(f, "    Total:   {:?}", self.total_duration)?;
        writeln!(f, "    Average: {:?}", self.avg_duration)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_sync() {
        let result = Benchmark::new("test_add")
            .iterations(100)
            .warmup(false)
            .run(|| {
                // Use black_box to prevent optimization and ensure measurable time
                std::hint::black_box(1 + 1);
                // Small sleep to ensure consistent non-zero duration across parallel test runs
                std::thread::sleep(Duration::from_nanos(100));
            });

        assert_eq!(result.name, "test_add");
        assert_eq!(result.iterations, 100);
        // Total duration should always be > 0 with the sleep
        assert!(result.total_duration.as_nanos() > 0);
        assert!(result.avg_duration.as_nanos() > 0);
        assert!(result.ops_per_sec > 0.0);
    }

    #[tokio::test]
    async fn test_benchmark_async() {
        let result = Benchmark::new("test_async")
            .iterations(50)
            .warmup(false)
            .run_async(|| async {
                tokio::time::sleep(Duration::from_micros(1)).await;
            })
            .await;

        assert_eq!(result.name, "test_async");
        assert_eq!(result.iterations, 50);
    }

    #[test]
    fn test_benchmark_suite() {
        let mut suite = BenchmarkSuite::new("test suite");

        let result1 = Benchmark::new("fast")
            .iterations(10)
            .warmup(false)
            .run(|| {});

        let result2 = Benchmark::new("slow").iterations(10).warmup(false).run(|| {
            std::thread::sleep(Duration::from_micros(10));
        });

        suite.add_result(result1);
        suite.add_result(result2);

        assert_eq!(suite.results().len(), 2);
        assert!(suite.fastest().is_some());
        assert!(suite.slowest().is_some());
    }

    #[tokio::test]
    async fn test_load_test() {
        let result = LoadTest::new("test load")
            .concurrent_users(5)
            .requests_per_user(10)
            .run(|| async { Ok(()) })
            .await;

        assert_eq!(result.name, "test load");
        assert_eq!(result.concurrent_users, 5);
        assert_eq!(result.total_requests, 50);
        assert_eq!(result.successful_requests, 50);
        assert_eq!(result.failed_requests, 0);
        assert_eq!(result.success_rate(), 100.0);
    }

    #[test]
    fn test_profiler() {
        let mut profiler = Profiler::new("test profiler");

        profiler.start_section("section1");
        std::thread::sleep(Duration::from_micros(10));
        profiler.end_section();

        profiler.start_section("section2");
        std::thread::sleep(Duration::from_micros(20));
        profiler.end_section();

        let results = profiler.results();
        assert_eq!(results.sections.len(), 2);
        assert!(results.sections.contains_key("section1"));
        assert!(results.sections.contains_key("section2"));
    }
}
