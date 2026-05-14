//! Professional report generation for AI operations
//!
//! This module provides utilities for generating comprehensive reports
//! from AI analysis results in multiple formats (Markdown, JSON, CSV).

use crate::error::AiError;
use crate::fraud::RiskLevel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Output format for reports
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    /// Markdown format for documentation
    Markdown,
    /// JSON format for APIs and data exchange
    Json,
    /// CSV format for spreadsheet analysis
    Csv,
}

/// Cost analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnalysisReport {
    /// Report title
    pub title: String,
    /// Time period covered
    pub period: String,
    /// Total cost in USD
    pub total_cost: f64,
    /// Cost breakdown by provider
    pub by_provider: HashMap<String, f64>,
    /// Cost breakdown by operation type
    pub by_operation: HashMap<String, f64>,
    /// Number of requests
    pub total_requests: usize,
    /// Average cost per request
    pub avg_cost_per_request: f64,
    /// Cost trend (percentage change from previous period)
    pub cost_trend: Option<f64>,
    /// Recommendations for cost optimization
    pub recommendations: Vec<String>,
}

impl CostAnalysisReport {
    /// Create a new cost analysis report
    #[must_use]
    pub fn new(title: String, period: String) -> Self {
        Self {
            title,
            period,
            total_cost: 0.0,
            by_provider: HashMap::new(),
            by_operation: HashMap::new(),
            total_requests: 0,
            avg_cost_per_request: 0.0,
            cost_trend: None,
            recommendations: Vec::new(),
        }
    }

    /// Add provider cost
    pub fn add_provider_cost(&mut self, provider: String, cost: f64) {
        *self.by_provider.entry(provider).or_insert(0.0) += cost;
        self.total_cost += cost;
    }

    /// Add operation cost
    pub fn add_operation_cost(&mut self, operation: String, cost: f64) {
        *self.by_operation.entry(operation).or_insert(0.0) += cost;
    }

    /// Set total requests and calculate average
    pub fn set_total_requests(&mut self, count: usize) {
        self.total_requests = count;
        if count > 0 {
            self.avg_cost_per_request = self.total_cost / count as f64;
        }
    }

    /// Add a cost optimization recommendation
    pub fn add_recommendation(&mut self, recommendation: String) {
        self.recommendations.push(recommendation);
    }

    /// Generate markdown report
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut report = String::new();

        let _ = writeln!(report, "# {}", self.title);
        let _ = writeln!(report, "**Period:** {}", self.period);

        report.push_str("## Summary\n\n");
        let _ = writeln!(report, "- **Total Cost:** ${:.2}", self.total_cost);
        let _ = writeln!(report, "- **Total Requests:** {}", self.total_requests);
        let _ = writeln!(
            report,
            "- **Average Cost per Request:** ${:.4}",
            self.avg_cost_per_request
        );

        if let Some(trend) = self.cost_trend {
            let trend_symbol = if trend > 0.0 { "↑" } else { "↓" };
            let _ = writeln!(
                report,
                "- **Cost Trend:** {}{:.1}%",
                trend_symbol,
                trend.abs()
            );
        }
        report.push('\n');

        if !self.by_provider.is_empty() {
            report.push_str("## Cost by Provider\n\n");
            report.push_str("| Provider | Cost | Percentage |\n");
            report.push_str("|----------|------|------------|\n");

            let mut providers: Vec<_> = self.by_provider.iter().collect();
            providers.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (provider, cost) in providers {
                let percentage = (cost / self.total_cost) * 100.0;
                let _ = writeln!(report, "| {provider} | ${cost:.2} | {percentage:.1}% |");
            }
            report.push('\n');
        }

        if !self.by_operation.is_empty() {
            report.push_str("## Cost by Operation\n\n");
            report.push_str("| Operation | Cost | Percentage |\n");
            report.push_str("|-----------|------|------------|\n");

            let mut operations: Vec<_> = self.by_operation.iter().collect();
            operations.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (operation, cost) in operations {
                let percentage = (cost / self.total_cost) * 100.0;
                let _ = writeln!(report, "| {operation} | ${cost:.2} | {percentage:.1}% |");
            }
            report.push('\n');
        }

        if !self.recommendations.is_empty() {
            report.push_str("## Cost Optimization Recommendations\n\n");
            for (i, rec) in self.recommendations.iter().enumerate() {
                let _ = writeln!(report, "{}. {}", i + 1, rec);
            }
            report.push('\n');
        }

        report
    }

    /// Generate JSON report
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize report: {e}")))
    }

    /// Generate CSV report
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();

        csv.push_str("Category,Item,Value\n");
        let _ = writeln!(csv, "Summary,Total Cost,{:.2}", self.total_cost);
        let _ = writeln!(csv, "Summary,Total Requests,{}", self.total_requests);
        let _ = writeln!(
            csv,
            "Summary,Avg Cost per Request,{:.4}",
            self.avg_cost_per_request
        );

        for (provider, cost) in &self.by_provider {
            let _ = writeln!(csv, "Provider,{provider},{cost:.2}");
        }

        for (operation, cost) in &self.by_operation {
            let _ = writeln!(csv, "Operation,{operation},{cost:.2}");
        }

        csv
    }
}

/// Performance benchmark report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBenchmarkReport {
    /// Report title
    pub title: String,
    /// Benchmark date
    pub date: String,
    /// Performance metrics by operation
    pub operations: HashMap<String, OperationBenchmark>,
    /// Overall statistics
    pub summary: BenchmarkSummary,
}

/// Benchmark data for a single operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationBenchmark {
    /// Operation name
    pub name: String,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// Median latency in milliseconds
    pub median_latency_ms: f64,
    /// 95th percentile latency
    pub p95_latency_ms: f64,
    /// 99th percentile latency
    pub p99_latency_ms: f64,
    /// Total operations
    pub total_ops: usize,
    /// Success rate (0-100)
    pub success_rate: f64,
}

/// Overall benchmark summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSummary {
    /// Total operations benchmarked
    pub total_operations: usize,
    /// Overall average latency
    pub overall_avg_latency_ms: f64,
    /// Overall success rate
    pub overall_success_rate: f64,
    /// Slowest operation
    pub slowest_operation: Option<String>,
    /// Fastest operation
    pub fastest_operation: Option<String>,
}

impl PerformanceBenchmarkReport {
    /// Create a new performance benchmark report
    #[must_use]
    pub fn new(title: String, date: String) -> Self {
        Self {
            title,
            date,
            operations: HashMap::new(),
            summary: BenchmarkSummary {
                total_operations: 0,
                overall_avg_latency_ms: 0.0,
                overall_success_rate: 0.0,
                slowest_operation: None,
                fastest_operation: None,
            },
        }
    }

    /// Add operation benchmark
    pub fn add_operation(&mut self, name: String, benchmark: OperationBenchmark) {
        self.operations.insert(name, benchmark);
    }

    /// Calculate summary statistics
    pub fn calculate_summary(&mut self) {
        if self.operations.is_empty() {
            return;
        }

        let total_ops: usize = self.operations.values().map(|b| b.total_ops).sum();
        let total_latency: f64 = self
            .operations
            .values()
            .map(|b| b.avg_latency_ms * b.total_ops as f64)
            .sum();

        let total_success: f64 = self
            .operations
            .values()
            .map(|b| b.success_rate * b.total_ops as f64)
            .sum();

        self.summary.total_operations = total_ops;
        self.summary.overall_avg_latency_ms = if total_ops > 0 {
            total_latency / total_ops as f64
        } else {
            0.0
        };
        self.summary.overall_success_rate = if total_ops > 0 {
            total_success / total_ops as f64
        } else {
            0.0
        };

        // Find slowest and fastest operations
        let mut slowest: Option<(&String, &OperationBenchmark)> = None;
        let mut fastest: Option<(&String, &OperationBenchmark)> = None;

        for (name, bench) in &self.operations {
            if slowest.is_none() || bench.avg_latency_ms > slowest.expect("slowest is Some per is_none check").1.avg_latency_ms {
                slowest = Some((name, bench));
            }
            if fastest.is_none() || bench.avg_latency_ms < fastest.expect("fastest is Some per is_none check").1.avg_latency_ms {
                fastest = Some((name, bench));
            }
        }

        self.summary.slowest_operation = slowest.map(|(name, _)| name.clone());
        self.summary.fastest_operation = fastest.map(|(name, _)| name.clone());
    }

    /// Generate markdown report
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut report = String::new();

        let _ = writeln!(report, "# {}", self.title);
        let _ = writeln!(report, "**Date:** {}", self.date);

        report.push_str("## Summary\n\n");
        let _ = writeln!(
            report,
            "- **Total Operations:** {}",
            self.summary.total_operations
        );
        let _ = writeln!(
            report,
            "- **Overall Avg Latency:** {:.2}ms",
            self.summary.overall_avg_latency_ms
        );
        let _ = writeln!(
            report,
            "- **Overall Success Rate:** {:.1}%",
            self.summary.overall_success_rate
        );

        if let Some(ref slowest) = self.summary.slowest_operation {
            let _ = writeln!(report, "- **Slowest Operation:** {slowest}");
        }
        if let Some(ref fastest) = self.summary.fastest_operation {
            let _ = writeln!(report, "- **Fastest Operation:** {fastest}");
        }
        report.push('\n');

        if !self.operations.is_empty() {
            report.push_str("## Operation Benchmarks\n\n");
            report.push_str(
                "| Operation | Avg (ms) | Median (ms) | P95 (ms) | P99 (ms) | Ops | Success % |\n",
            );
            report.push_str(
                "|-----------|----------|-------------|----------|----------|-----|----------|\n",
            );

            let mut ops: Vec<_> = self.operations.iter().collect();
            ops.sort_by(|a, b| b.1.avg_latency_ms.partial_cmp(&a.1.avg_latency_ms).unwrap_or(std::cmp::Ordering::Equal));

            for (name, bench) in ops {
                let _ = writeln!(
                    report,
                    "| {} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {:.1}% |",
                    name,
                    bench.avg_latency_ms,
                    bench.median_latency_ms,
                    bench.p95_latency_ms,
                    bench.p99_latency_ms,
                    bench.total_ops,
                    bench.success_rate
                );
            }
            report.push('\n');
        }

        report
    }

    /// Generate JSON report
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize report: {e}")))
    }
}

/// Fraud analysis summary report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudSummaryReport {
    /// Report title
    pub title: String,
    /// Analysis period
    pub period: String,
    /// Total cases analyzed
    pub total_cases: usize,
    /// Cases by risk level
    pub by_risk_level: HashMap<String, usize>,
    /// Most common fraud types
    pub common_fraud_types: Vec<(String, usize)>,
    /// Detection accuracy (if available)
    pub accuracy: Option<f64>,
    /// Key insights
    pub insights: Vec<String>,
}

impl FraudSummaryReport {
    /// Create a new fraud summary report
    #[must_use]
    pub fn new(title: String, period: String) -> Self {
        Self {
            title,
            period,
            total_cases: 0,
            by_risk_level: HashMap::new(),
            common_fraud_types: Vec::new(),
            accuracy: None,
            insights: Vec::new(),
        }
    }

    /// Add a fraud case
    pub fn add_case(&mut self, risk_level: RiskLevel) {
        self.total_cases += 1;
        let level_str = format!("{risk_level:?}");
        *self.by_risk_level.entry(level_str).or_insert(0) += 1;
    }

    /// Set common fraud types
    pub fn set_common_fraud_types(&mut self, types: Vec<(String, usize)>) {
        self.common_fraud_types = types;
    }

    /// Add an insight
    pub fn add_insight(&mut self, insight: String) {
        self.insights.push(insight);
    }

    /// Generate markdown report
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut report = String::new();

        let _ = writeln!(report, "# {}", self.title);
        let _ = writeln!(report, "**Period:** {}", self.period);

        report.push_str("## Summary\n\n");
        let _ = writeln!(report, "- **Total Cases Analyzed:** {}", self.total_cases);

        if let Some(accuracy) = self.accuracy {
            let _ = writeln!(report, "- **Detection Accuracy:** {accuracy:.1}%");
        }
        report.push('\n');

        if !self.by_risk_level.is_empty() {
            report.push_str("## Risk Level Distribution\n\n");
            report.push_str("| Risk Level | Count | Percentage |\n");
            report.push_str("|------------|-------|------------|\n");

            let mut levels: Vec<_> = self.by_risk_level.iter().collect();
            levels.sort_by(|a, b| b.1.cmp(a.1));

            for (level, count) in levels {
                let percentage = (*count as f64 / self.total_cases as f64) * 100.0;
                let _ = writeln!(report, "| {level} | {count} | {percentage:.1}% |");
            }
            report.push('\n');
        }

        if !self.common_fraud_types.is_empty() {
            report.push_str("## Common Fraud Types\n\n");
            report.push_str("| Fraud Type | Occurrences |\n");
            report.push_str("|------------|-------------|\n");

            for (fraud_type, count) in &self.common_fraud_types {
                let _ = writeln!(report, "| {fraud_type} | {count} |");
            }
            report.push('\n');
        }

        if !self.insights.is_empty() {
            report.push_str("## Key Insights\n\n");
            for (i, insight) in self.insights.iter().enumerate() {
                let _ = writeln!(report, "{}. {}", i + 1, insight);
            }
            report.push('\n');
        }

        report
    }

    /// Generate JSON report
    pub fn to_json(&self) -> Result<String, AiError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| AiError::InvalidInput(format!("Failed to serialize report: {e}")))
    }
}

/// Report generator for all report types
pub struct ReportGenerator;

impl ReportGenerator {
    /// Generate a report in the specified format
    pub fn generate(report_type: ReportType, format: ReportFormat) -> Result<String, AiError> {
        match format {
            ReportFormat::Markdown => match report_type {
                ReportType::CostAnalysis(ref report) => Ok(report.to_markdown()),
                ReportType::PerformanceBenchmark(ref report) => Ok(report.to_markdown()),
                ReportType::FraudSummary(ref report) => Ok(report.to_markdown()),
            },
            ReportFormat::Json => match report_type {
                ReportType::CostAnalysis(ref report) => report.to_json(),
                ReportType::PerformanceBenchmark(ref report) => report.to_json(),
                ReportType::FraudSummary(ref report) => report.to_json(),
            },
            ReportFormat::Csv => match report_type {
                ReportType::CostAnalysis(ref report) => Ok(report.to_csv()),
                ReportType::PerformanceBenchmark(_) => Err(AiError::InvalidInput(
                    "CSV format not supported for performance benchmarks".to_string(),
                )),
                ReportType::FraudSummary(_) => Err(AiError::InvalidInput(
                    "CSV format not supported for fraud summaries".to_string(),
                )),
            },
        }
    }
}

/// Report type enum
pub enum ReportType {
    /// Cost analysis report
    CostAnalysis(CostAnalysisReport),
    /// Performance benchmark report
    PerformanceBenchmark(PerformanceBenchmarkReport),
    /// Fraud summary report
    FraudSummary(FraudSummaryReport),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_analysis_report_creation() {
        let mut report = CostAnalysisReport::new(
            "Cost Analysis Q1 2026".to_string(),
            "January - March 2026".to_string(),
        );

        report.add_provider_cost("OpenAI".to_string(), 150.50);
        report.add_provider_cost("Anthropic".to_string(), 120.25);
        report.add_provider_cost("Gemini".to_string(), 45.00);

        report.add_operation_cost("code_evaluation".to_string(), 180.00);
        report.add_operation_cost("verification".to_string(), 95.75);
        report.add_operation_cost("fraud_detection".to_string(), 40.00);

        report.set_total_requests(1_250);
        report.add_recommendation(
            "Consider using Gemini for simple tasks to reduce costs by 40%".to_string(),
        );

        assert_eq!(report.total_cost, 315.75);
        assert_eq!(report.total_requests, 1_250);
        assert!((report.avg_cost_per_request - 0.2526).abs() < 1e-4);
        assert_eq!(report.by_provider.len(), 3);
        assert_eq!(report.by_operation.len(), 3);
        assert_eq!(report.recommendations.len(), 1);
    }

    #[test]
    fn test_cost_analysis_markdown_generation() {
        let mut report =
            CostAnalysisReport::new("Test Report".to_string(), "January 2026".to_string());

        report.add_provider_cost("OpenAI".to_string(), 100.0);
        report.set_total_requests(500);

        let markdown = report.to_markdown();

        assert!(markdown.contains("# Test Report"));
        assert!(markdown.contains("**Period:** January 2026"));
        assert!(markdown.contains("**Total Cost:** $100.00"));
        assert!(markdown.contains("**Total Requests:** 500"));
        assert!(markdown.contains("## Cost by Provider"));
    }

    #[test]
    fn test_cost_analysis_json_generation() {
        let mut report =
            CostAnalysisReport::new("Test Report".to_string(), "January 2026".to_string());

        report.add_provider_cost("OpenAI".to_string(), 100.0);
        report.set_total_requests(500);

        let json = report.to_json().unwrap();

        assert!(json.contains("\"title\""));
        assert!(json.contains("\"Test Report\""));
        assert!(json.contains("\"total_cost\""));
    }

    #[test]
    fn test_cost_analysis_csv_generation() {
        let mut report =
            CostAnalysisReport::new("Test Report".to_string(), "January 2026".to_string());

        report.add_provider_cost("OpenAI".to_string(), 100.0);
        report.add_operation_cost("evaluation".to_string(), 50.0);
        report.set_total_requests(500);

        let csv = report.to_csv();

        assert!(csv.contains("Category,Item,Value"));
        assert!(csv.contains("Summary,Total Cost,100.00"));
        assert!(csv.contains("Provider,OpenAI,100.00"));
        assert!(csv.contains("Operation,evaluation,50.00"));
    }

    #[test]
    fn test_performance_benchmark_report_creation() {
        let mut report = PerformanceBenchmarkReport::new(
            "Performance Benchmarks".to_string(),
            "2026-01-09".to_string(),
        );

        let bench1 = OperationBenchmark {
            name: "code_evaluation".to_string(),
            avg_latency_ms: 250.5,
            median_latency_ms: 240.0,
            p95_latency_ms: 350.0,
            p99_latency_ms: 450.0,
            total_ops: 1_000,
            success_rate: 99.5,
        };

        let bench2 = OperationBenchmark {
            name: "fraud_detection".to_string(),
            avg_latency_ms: 180.2,
            median_latency_ms: 170.0,
            p95_latency_ms: 250.0,
            p99_latency_ms: 320.0,
            total_ops: 500,
            success_rate: 98.8,
        };

        report.add_operation("code_evaluation".to_string(), bench1);
        report.add_operation("fraud_detection".to_string(), bench2);
        report.calculate_summary();

        assert_eq!(report.operations.len(), 2);
        assert_eq!(report.summary.total_operations, 1_500);
        assert!(report.summary.slowest_operation.is_some());
        assert!(report.summary.fastest_operation.is_some());
    }

    #[test]
    fn test_performance_benchmark_markdown_generation() {
        let mut report = PerformanceBenchmarkReport::new(
            "Test Benchmarks".to_string(),
            "2026-01-09".to_string(),
        );

        let bench = OperationBenchmark {
            name: "test_op".to_string(),
            avg_latency_ms: 100.0,
            median_latency_ms: 95.0,
            p95_latency_ms: 120.0,
            p99_latency_ms: 150.0,
            total_ops: 100,
            success_rate: 99.0,
        };

        report.add_operation("test_op".to_string(), bench);
        report.calculate_summary();

        let markdown = report.to_markdown();

        assert!(markdown.contains("# Test Benchmarks"));
        assert!(markdown.contains("**Date:** 2026-01-09"));
        assert!(markdown.contains("## Summary"));
        assert!(markdown.contains("## Operation Benchmarks"));
    }

    #[test]
    fn test_fraud_summary_report_creation() {
        let mut report =
            FraudSummaryReport::new("Fraud Analysis".to_string(), "Q1 2026".to_string());

        report.add_case(RiskLevel::Low);
        report.add_case(RiskLevel::Low);
        report.add_case(RiskLevel::Medium);
        report.add_case(RiskLevel::High);
        report.add_case(RiskLevel::Critical);

        report.set_common_fraud_types(vec![
            ("Sybil Attack".to_string(), 15),
            ("Wash Trading".to_string(), 8),
        ]);

        report
            .add_insight("Sybil attacks increased by 25% compared to previous quarter".to_string());
        report.accuracy = Some(94.5);

        assert_eq!(report.total_cases, 5);
        assert_eq!(report.common_fraud_types.len(), 2);
        assert_eq!(report.insights.len(), 1);
        assert_eq!(report.accuracy, Some(94.5));
    }

    #[test]
    fn test_fraud_summary_markdown_generation() {
        let mut report =
            FraudSummaryReport::new("Test Fraud Report".to_string(), "January 2026".to_string());

        report.add_case(RiskLevel::Low);
        report.add_case(RiskLevel::High);
        report.accuracy = Some(95.0);
        report.add_insight("Test insight".to_string());

        let markdown = report.to_markdown();

        assert!(markdown.contains("# Test Fraud Report"));
        assert!(markdown.contains("**Period:** January 2026"));
        assert!(markdown.contains("**Total Cases Analyzed:** 2"));
        assert!(markdown.contains("**Detection Accuracy:** 95.0%"));
        assert!(markdown.contains("## Key Insights"));
    }

    #[test]
    fn test_report_generator_markdown() {
        let report = CostAnalysisReport::new("Test".to_string(), "2026".to_string());

        let result =
            ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Markdown);

        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("# Test"));
    }

    #[test]
    fn test_report_generator_json() {
        let report = CostAnalysisReport::new("Test".to_string(), "2026".to_string());

        let result =
            ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Json);

        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("\"title\""));
    }

    #[test]
    fn test_report_generator_csv_unsupported() {
        let report = PerformanceBenchmarkReport::new("Test".to_string(), "2026".to_string());

        let result =
            ReportGenerator::generate(ReportType::PerformanceBenchmark(report), ReportFormat::Csv);

        assert!(result.is_err());
    }
}
