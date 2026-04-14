//! Report generation and dashboard integration examples.

use crate::error::Result;

/// Example: Professional report generation
///
/// Demonstrates how to:
/// - Generate cost analysis reports
/// - Create performance benchmark reports
/// - Produce fraud detection summaries
/// - Export reports in multiple formats (Markdown, JSON, CSV)
pub struct ReportGenerationExample;

impl ReportGenerationExample {
    /// Generate a comprehensive cost analysis report
    pub async fn cost_analysis_report() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Cost Analysis Report Generation ===");
        println!();

        // Create a cost analysis report
        let mut report = CostAnalysisReport::new(
            "AI Operations Cost Analysis - Q1 2026".to_string(),
            "January - March 2026".to_string(),
        );

        // Add provider costs
        report.add_provider_cost("OpenAI".to_string(), 1_850.50);
        report.add_provider_cost("Anthropic".to_string(), 1_420.75);
        report.add_provider_cost("Gemini".to_string(), 345.00);
        report.add_provider_cost("DeepSeek".to_string(), 125.25);
        report.add_provider_cost("Ollama".to_string(), 0.0);

        // Add operation costs
        report.add_operation_cost("Code Evaluation".to_string(), 1_680.00);
        report.add_operation_cost("Commitment Verification".to_string(), 895.75);
        report.add_operation_cost("Fraud Detection".to_string(), 540.50);
        report.add_operation_cost("Plagiarism Detection".to_string(), 425.25);
        report.add_operation_cost("Document Analysis".to_string(), 200.00);

        // Set request metrics
        report.set_total_requests(15_450);
        report.cost_trend = Some(-12.5); // 12.5% cost reduction

        // Add recommendations
        report.add_recommendation(
            "Switch 40% of simple tasks to Gemini Flash to reduce costs by ~$600/month".to_string(),
        );
        report.add_recommendation(
            "Use Ollama for development and testing to eliminate dev environment costs".to_string(),
        );
        report.add_recommendation(
            "Implement request batching to reduce API calls by 25%".to_string(),
        );
        report.add_recommendation(
            "Enable caching for repetitive operations (estimated 15% cost savings)".to_string(),
        );

        println!("Report Summary:");
        println!("  * Total Cost: ${:.2}", report.total_cost);
        println!("  * Total Requests: {}", report.total_requests);
        println!("  * Avg Cost/Request: ${:.4}", report.avg_cost_per_request);
        println!("  * Cost Trend: down 12.5%");
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Json,
        )?;
        println!("{}...{}", &json[..200], &json[json.len() - 50..]);
        println!();

        // Generate CSV report
        println!("=== CSV Report ===");
        let csv = ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Csv)?;
        let lines: Vec<&str> = csv.lines().take(8).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("Cost analysis report generated in 3 formats");

        Ok(())
    }

    /// Generate a performance benchmark report
    pub async fn performance_benchmark_report() -> Result<()> {
        use crate::reports::{
            OperationBenchmark, PerformanceBenchmarkReport, ReportFormat, ReportGenerator,
            ReportType,
        };

        println!("=== Performance Benchmark Report Generation ===");
        println!();

        let mut report = PerformanceBenchmarkReport::new(
            "AI Services Performance Benchmarks".to_string(),
            "2026-01-09".to_string(),
        );

        // Add operation benchmarks
        report.add_operation(
            "code_evaluation".to_string(),
            OperationBenchmark {
                name: "Code Evaluation".to_string(),
                avg_latency_ms: 285.5,
                median_latency_ms: 265.0,
                p95_latency_ms: 420.0,
                p99_latency_ms: 580.0,
                total_ops: 5_420,
                success_rate: 99.2,
            },
        );

        report.add_operation(
            "commitment_verification".to_string(),
            OperationBenchmark {
                name: "Commitment Verification".to_string(),
                avg_latency_ms: 320.2,
                median_latency_ms: 295.0,
                p95_latency_ms: 495.0,
                p99_latency_ms: 650.0,
                total_ops: 3_850,
                success_rate: 98.7,
            },
        );

        report.add_operation(
            "fraud_detection".to_string(),
            OperationBenchmark {
                name: "Fraud Detection".to_string(),
                avg_latency_ms: 195.8,
                median_latency_ms: 180.0,
                p95_latency_ms: 280.0,
                p99_latency_ms: 385.0,
                total_ops: 2_240,
                success_rate: 99.5,
            },
        );

        report.add_operation(
            "plagiarism_detection".to_string(),
            OperationBenchmark {
                name: "Plagiarism Detection".to_string(),
                avg_latency_ms: 425.3,
                median_latency_ms: 390.0,
                p95_latency_ms: 620.0,
                p99_latency_ms: 820.0,
                total_ops: 1_650,
                success_rate: 97.8,
            },
        );

        report.add_operation(
            "document_analysis".to_string(),
            OperationBenchmark {
                name: "Document Analysis".to_string(),
                avg_latency_ms: 520.7,
                median_latency_ms: 475.0,
                p95_latency_ms: 780.0,
                p99_latency_ms: 1050.0,
                total_ops: 890,
                success_rate: 98.3,
            },
        );

        // Calculate summary statistics
        report.calculate_summary();

        println!("Performance Summary:");
        println!("  * Total Operations: {}", report.summary.total_operations);
        println!(
            "  * Overall Avg Latency: {:.2}ms",
            report.summary.overall_avg_latency_ms
        );
        println!(
            "  * Overall Success Rate: {:.1}%",
            report.summary.overall_success_rate
        );
        if let Some(ref fastest) = report.summary.fastest_operation {
            println!("  * Fastest Operation: {fastest}");
        }
        if let Some(ref slowest) = report.summary.slowest_operation {
            println!("  * Slowest Operation: {slowest}");
        }
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::PerformanceBenchmark(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(
            ReportType::PerformanceBenchmark(report),
            ReportFormat::Json,
        )?;
        let lines: Vec<&str> = json.lines().take(10).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("Performance benchmark report generated");

        Ok(())
    }

    /// Generate a fraud detection summary report
    pub async fn fraud_summary_report() -> Result<()> {
        use crate::fraud::RiskLevel;
        use crate::reports::{FraudSummaryReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Fraud Detection Summary Report Generation ===");
        println!();

        let mut report = FraudSummaryReport::new(
            "Fraud Detection Analysis - Q1 2026".to_string(),
            "January - March 2026".to_string(),
        );

        // Add fraud cases
        for _ in 0..125 {
            report.add_case(RiskLevel::Low);
        }
        for _ in 0..48 {
            report.add_case(RiskLevel::Medium);
        }
        for _ in 0..23 {
            report.add_case(RiskLevel::High);
        }
        for _ in 0..7 {
            report.add_case(RiskLevel::Critical);
        }

        // Set common fraud types
        report.set_common_fraud_types(vec![
            ("Sybil Attack".to_string(), 45),
            ("Wash Trading".to_string(), 28),
            ("Reputation Gaming".to_string(), 22),
            ("Image Manipulation".to_string(), 18),
            ("Content Plagiarism".to_string(), 15),
        ]);

        // Set detection accuracy
        report.accuracy = Some(96.8);

        // Add insights
        report.add_insight(
            "Sybil attacks increased by 32% compared to Q4 2026, primarily from newly registered accounts".to_string(),
        );
        report.add_insight(
            "Average detection time improved by 45% due to enhanced ML models".to_string(),
        );
        report.add_insight(
            "Image manipulation attempts decreased by 18% after implementing perceptual hashing"
                .to_string(),
        );
        report.add_insight(
            "Critical risk cases require average 15 minutes for manual review and final decision"
                .to_string(),
        );

        println!("Fraud Detection Summary:");
        println!("  * Total Cases Analyzed: {}", report.total_cases);
        println!("  * Detection Accuracy: {:.1}%", report.accuracy.unwrap());
        println!("  * Most Common: Sybil Attack (45 cases)");
        println!("  * Critical Cases: 7");
        println!();

        // Generate Markdown report
        println!("=== Markdown Report ===");
        let markdown = ReportGenerator::generate(
            ReportType::FraudSummary(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("{markdown}");

        // Generate JSON report
        println!("=== JSON Report (Preview) ===");
        let json = ReportGenerator::generate(ReportType::FraudSummary(report), ReportFormat::Json)?;
        let lines: Vec<&str> = json.lines().take(15).collect();
        println!("{}", lines.join("\n"));
        println!("...");
        println!();

        println!("Fraud detection summary report generated");

        Ok(())
    }

    /// Demonstrate multi-format report export
    pub async fn multi_format_export() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Multi-Format Report Export ===");
        println!();

        let mut report = CostAnalysisReport::new(
            "Weekly Cost Report".to_string(),
            "Week of Jan 9, 2026".to_string(),
        );

        report.add_provider_cost("OpenAI".to_string(), 425.00);
        report.add_provider_cost("Anthropic".to_string(), 325.00);
        report.add_provider_cost("Gemini".to_string(), 85.00);
        report.set_total_requests(3_500);

        println!("Exporting report in 3 formats...");
        println!();

        // Export as Markdown
        let markdown = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Markdown,
        )?;
        println!("Markdown export: {} bytes", markdown.len());
        println!("  Use case: Documentation, GitHub issues, Slack messages");
        println!();

        // Export as JSON
        let json = ReportGenerator::generate(
            ReportType::CostAnalysis(report.clone()),
            ReportFormat::Json,
        )?;
        println!("JSON export: {} bytes", json.len());
        println!("  Use case: API responses, data storage, dashboard integration");
        println!();

        // Export as CSV
        let csv = ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Csv)?;
        println!("CSV export: {} bytes", csv.len());
        println!("  Use case: Excel analysis, data science, financial reporting");
        println!();

        println!("All reports generated successfully!");
        println!();
        println!("Integration tips:");
        println!("  * Save to files: write reports to disk for archival");
        println!("  * Send via email: attach reports to automated email notifications");
        println!("  * Post to Slack: use Markdown format for rich formatting");
        println!("  * Store in DB: save JSON format for queryable storage");
        println!("  * Generate dashboards: parse JSON data for real-time visualizations");

        Ok(())
    }

    /// Demonstrate automated report scheduling
    pub async fn automated_reporting() -> Result<()> {
        use crate::reports::{CostAnalysisReport, ReportFormat, ReportGenerator, ReportType};

        println!("=== Automated Report Scheduling ===");
        println!();

        println!("Example: Daily cost monitoring workflow");
        println!();

        // Simulate daily report generation
        let mut report = CostAnalysisReport::new(
            "Daily Cost Report".to_string(),
            format!("Date: {}", chrono::Local::now().format("%Y-%m-%d")),
        );

        report.add_provider_cost("OpenAI".to_string(), 125.50);
        report.add_provider_cost("Anthropic".to_string(), 95.25);
        report.set_total_requests(1_250);
        report.cost_trend = Some(5.2); // 5.2% increase

        // Generate alert if cost exceeds threshold
        if report.total_cost > 200.0 {
            println!("ALERT: Daily cost exceeds $200 threshold!");
            println!("  Current: ${:.2}", report.total_cost);
            println!("  Trend: +{:.1}%", report.cost_trend.unwrap());
            println!();
        }

        // Add automated recommendations
        if let Some(trend) = report.cost_trend {
            if trend > 10.0 {
                report.add_recommendation(
                    "Cost increased significantly. Review recent usage patterns".to_string(),
                );
            }
        }

        let _markdown =
            ReportGenerator::generate(ReportType::CostAnalysis(report), ReportFormat::Markdown)?;

        println!("Report generated and ready for distribution:");
        println!("  * Post to Slack #finance channel");
        println!("  * Email to finance@company.com");
        println!("  * Archive in S3 bucket");
        println!("  * Update monitoring dashboard");
        println!();

        println!("Scheduling options:");
        println!("  * Use cron: Schedule daily/weekly/monthly reports");
        println!("  * Use tokio interval: In-app scheduled reporting");
        println!("  * Use GitHub Actions: Automated CI/CD reports");
        println!("  * Use AWS Lambda: Serverless scheduled reports");

        Ok(())
    }
}

/// Example: Dashboard integration and monitoring
///
/// Demonstrates how to:
/// - Export metrics to Prometheus
/// - Integrate with Grafana
/// - Generate health check endpoints
/// - Monitor AI operations in real-time
pub struct DashboardIntegrationExample;

impl DashboardIntegrationExample {
    /// Export metrics to Prometheus format
    pub async fn prometheus_export() -> Result<()> {
        use crate::dashboard::DashboardMetrics;

        println!("=== Prometheus Metrics Export ===");
        println!();

        // Create dashboard metrics
        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 15_450;
        metrics.total_cost = 3_741.50;
        metrics.avg_latency_ms = 285.5;
        metrics.error_count = 127;
        metrics.success_rate = 99.18;
        metrics.cache_hit_rate = 42.5;
        metrics.circuit_breaker_open = 3;
        metrics.budget_utilization = 67.8;
        metrics.active_providers = vec![
            "openai".to_string(),
            "anthropic".to_string(),
            "gemini".to_string(),
        ];

        // Add custom metrics
        metrics.add_custom_metric("fraud_detection_accuracy".to_string(), 96.8);
        metrics.add_custom_metric("plagiarism_checks_total".to_string(), 1_250.0);

        println!("Current Metrics:");
        println!("  * Total Requests: {}", metrics.request_count);
        println!("  * Total Cost: ${:.2}", metrics.total_cost);
        println!("  * Avg Latency: {:.2}ms", metrics.avg_latency_ms);
        println!("  * Success Rate: {:.2}%", metrics.success_rate);
        println!("  * Cache Hit Rate: {:.2}%", metrics.cache_hit_rate);
        println!();

        // Export to Prometheus format
        let prometheus_metrics = metrics.to_prometheus();

        println!("=== Prometheus Format Output ===");
        println!();
        for metric in prometheus_metrics.iter().take(3) {
            println!("{}", metric.to_prometheus_format());
        }
        println!("... {} more metrics", prometheus_metrics.len() - 3);
        println!();

        println!("Integration:");
        println!("  * Expose this endpoint at /metrics");
        println!("  * Configure Prometheus to scrape this endpoint");
        println!("  * Default scrape interval: 15s");
        println!();

        println!("Example Prometheus Configuration:");
        println!("```yaml");
        println!("scrape_configs:");
        println!("  - job_name: 'kaccy-ai'");
        println!("    static_configs:");
        println!("      - targets: ['localhost:8080']");
        println!("    metrics_path: '/metrics'");
        println!("    scrape_interval: 15s");
        println!("```");

        Ok(())
    }

    /// Integrate with Grafana dashboard
    pub async fn grafana_integration() -> Result<()> {
        use crate::dashboard::{DashboardMetrics, to_grafana_format};

        println!("=== Grafana Dashboard Integration ===");
        println!();

        let mut metrics = DashboardMetrics::new();
        metrics.request_count = 8_500;
        metrics.total_cost = 1_847.25;
        metrics.avg_latency_ms = 320.5;
        metrics.success_rate = 98.9;
        metrics.cache_hit_rate = 38.2;
        metrics.budget_utilization = 52.1;

        // Convert to Grafana format
        let datapoints = to_grafana_format(&metrics);

        println!("Grafana Data Points (JSON Simple Format):");
        println!();
        for dp in datapoints.iter().take(3) {
            let json = dp.to_json()?;
            let lines: Vec<&str> = json.lines().take(5).collect();
            println!("{}", lines.join("\n"));
            println!("  ...");
            println!();
        }

        println!("Dashboard Setup:");
        println!("  1. Add JSON data source in Grafana");
        println!("  2. Configure endpoint: http://your-api/grafana-metrics");
        println!("  3. Set refresh interval: 10s");
        println!("  4. Create panels for each metric");
        println!();

        println!("Recommended Grafana Panels:");
        println!("  * Request Rate (Time Series)");
        println!("  * Cost Tracking (Stat + Time Series)");
        println!("  * Latency Distribution (Histogram)");
        println!("  * Success Rate (Gauge)");
        println!("  * Cache Hit Rate (Bar Gauge)");
        println!("  * Budget Utilization (Gauge with thresholds)");

        Ok(())
    }

    /// Health check endpoint for monitoring
    pub async fn health_check_endpoint() -> Result<()> {
        use crate::dashboard::{ComponentHealth, HealthCheckStatus};
        use std::time::{SystemTime, UNIX_EPOCH};

        println!("=== Health Check Endpoint ===");
        println!();

        let mut status = HealthCheckStatus::new();

        // Check LLM client health
        let llm_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(45.2),
            error: None,
        };
        status.add_component("llm_client".to_string(), llm_health);

        // Check cache health
        let cache_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(2.1),
            error: None,
        };
        status.add_component("cache".to_string(), cache_health);

        // Check database health (simulating degraded state)
        let db_health = ComponentHealth {
            healthy: true,
            last_check: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            response_time_ms: Some(125.8),
            error: None,
        };
        status.add_component("database".to_string(), db_health);

        println!("Health Check Status:");
        println!(
            "  * Overall: {}",
            if status.healthy {
                "HEALTHY"
            } else {
                "UNHEALTHY"
            }
        );
        println!("  * Components: {}", status.components.len());
        println!();

        let json = status.to_json()?;
        println!("=== JSON Response ===");
        let lines: Vec<&str> = json.lines().take(15).collect();
        println!("{}", lines.join("\n"));
        println!("  ...");
        println!();

        println!("Integration:");
        println!("  * Expose at /health or /healthz");
        println!("  * Used by Kubernetes liveness/readiness probes");
        println!("  * Monitored by uptime services (StatusPage, Pingdom)");
        println!("  * Return HTTP 200 if healthy, 503 if unhealthy");

        Ok(())
    }

    /// Real-time metrics monitoring
    pub async fn realtime_monitoring() -> Result<()> {
        use crate::dashboard::DashboardMetrics;

        println!("=== Real-Time Metrics Monitoring ===");
        println!();

        println!("Simulating real-time metric updates...");
        println!();

        // Simulate collecting metrics over time
        let mut metrics_history: Vec<DashboardMetrics> = Vec::new();

        for i in 0..5 {
            let mut metrics = DashboardMetrics::new();
            metrics.request_count = 1000 + i * 250;
            metrics.total_cost = 250.0 + (i as f64 * 62.5);
            metrics.avg_latency_ms = 280.0 + (i as f64 * 10.0);
            metrics.success_rate = 99.5 - (i as f64 * 0.1);
            metrics.cache_hit_rate = 40.0 + (i as f64 * 2.5);

            metrics_history.push(metrics);
        }

        println!("+---------+-----------+------------+------------+-------------+");
        println!("| Time    | Requests  | Cost       | Latency    | Success %   |");
        println!("+---------+-----------+------------+------------+-------------+");

        for (i, m) in metrics_history.iter().enumerate() {
            println!(
                "| T+{}min  | {:>9} | ${:>9.2} | {:>8.1}ms | {:>10.2}% |",
                i * 5,
                m.request_count,
                m.total_cost,
                m.avg_latency_ms,
                m.success_rate
            );
        }
        println!("+---------+-----------+------------+------------+-------------+");
        println!();

        println!("Monitoring Strategy:");
        println!("  * Update metrics every 10-30 seconds");
        println!("  * Store historical data for trend analysis");
        println!("  * Alert on threshold violations:");
        println!("    - Success rate < 95%");
        println!("    - Latency > 500ms (P95)");
        println!("    - Cost growth > 20% per hour");
        println!("    - Error rate > 5%");
        println!();

        println!("Alert Channels:");
        println!("  * PagerDuty for critical alerts");
        println!("  * Slack for warnings");
        println!("  * Email for daily summaries");
        println!("  * Webhook for custom integrations");

        Ok(())
    }

    /// Custom metrics and instrumentation
    pub async fn custom_instrumentation() -> Result<()> {
        use crate::dashboard::{DashboardMetrics, MetricType, PrometheusMetric, TimeSeriesPoint};

        println!("=== Custom Metrics and Instrumentation ===");
        println!();

        let mut metrics = DashboardMetrics::new();

        // Add domain-specific custom metrics
        metrics.add_custom_metric("code_evaluations_completed".to_string(), 5_420.0);
        metrics.add_custom_metric("fraud_cases_detected".to_string(), 127.0);
        metrics.add_custom_metric("plagiarism_similarity_avg".to_string(), 15.3);
        metrics.add_custom_metric("github_verifications".to_string(), 3_850.0);
        metrics.add_custom_metric("document_pages_analyzed".to_string(), 12_450.0);

        println!("Custom Metrics Registered:");
        for (name, value) in &metrics.custom_metrics {
            println!("  * {name}: {value}");
        }
        println!();

        // Create labeled metrics for detailed tracking
        let mut provider_latency = PrometheusMetric::new(
            "kaccy_ai_provider_latency_ms".to_string(),
            MetricType::Histogram,
            "LLM provider latency in milliseconds".to_string(),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(285.5)
                .with_label("provider".to_string(), "openai".to_string())
                .with_label("model".to_string(), "gpt-4-turbo".to_string()),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(320.2)
                .with_label("provider".to_string(), "anthropic".to_string())
                .with_label("model".to_string(), "claude-3-opus".to_string()),
        );

        provider_latency.add_point(
            TimeSeriesPoint::now(195.8)
                .with_label("provider".to_string(), "gemini".to_string())
                .with_label("model".to_string(), "gemini-1.5-flash".to_string()),
        );

        println!("=== Labeled Metrics (Prometheus Format) ===");
        println!("{}", provider_latency.to_prometheus_format());

        println!("Benefits of Custom Metrics:");
        println!("  * Track domain-specific KPIs");
        println!("  * Monitor business logic performance");
        println!("  * Identify bottlenecks and optimization opportunities");
        println!("  * Support data-driven decision making");
        println!("  * Enable advanced alerting rules");
        println!();

        println!("Best Practices:");
        println!("  * Use meaningful metric names (snake_case)");
        println!("  * Add appropriate labels for filtering");
        println!("  * Choose correct metric type (Counter/Gauge/Histogram)");
        println!("  * Document metrics in code comments");
        println!("  * Set up retention policies for historical data");

        Ok(())
    }
}
