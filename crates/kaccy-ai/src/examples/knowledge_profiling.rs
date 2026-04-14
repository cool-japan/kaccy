//! Knowledge base, performance profiling, and model version management examples.

use crate::error::Result;

/// Example: Knowledge Base
///
/// This example demonstrates domain-specific knowledge storage and retrieval.
pub struct KnowledgeBaseExample;

impl KnowledgeBaseExample {
    /// Domain-specific knowledge storage
    #[allow(dead_code)]
    pub async fn knowledge_storage() -> Result<()> {
        use crate::knowledge_base::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};

        println!("=== Knowledge Base Example ===");
        println!();

        let mut kb = KnowledgeBase::new();

        // Add entries
        let entry1 = KnowledgeEntry {
            id: "sol-basics".to_string(),
            domain: KnowledgeDomain::Blockchain,
            title: "Solidity Smart Contracts".to_string(),
            content: "Solidity is a statically-typed language for Ethereum...".to_string(),
            tags: vec![
                "solidity".to_string(),
                "ethereum".to_string(),
                "smart contracts".to_string(),
            ],
            source: Some("https://docs.soliditylang.org".to_string()),
            confidence: 0.95,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let _ = kb.add_entry(entry1);

        println!("Knowledge base created:");
        println!("  Domains: Programming, Blockchain, Security, etc.");
        println!("  Entries: Indexed by tags");
        println!("  Search: Fast tag and relevance-based");
        println!();
        println!("Example entry:");
        println!("  Title: Solidity Smart Contracts");
        println!("  Domain: Blockchain");
        println!("  Tags: solidity, ethereum, smart contracts");

        Ok(())
    }

    /// Keyword-based search
    #[allow(dead_code)]
    pub async fn keyword_search() -> Result<()> {
        use crate::knowledge_base::{KnowledgeBase, KnowledgeDomain, KnowledgeEntry};

        println!("=== Keyword-Based Search ===");
        println!();

        let mut kb = KnowledgeBase::new();

        // Add sample entries
        let entry = KnowledgeEntry {
            id: "rust-ownership".to_string(),
            domain: KnowledgeDomain::Programming,
            title: "Rust Ownership System".to_string(),
            content: "Rust's ownership system ensures memory safety...".to_string(),
            tags: vec![
                "rust".to_string(),
                "ownership".to_string(),
                "memory safety".to_string(),
            ],
            source: Some("https://doc.rust-lang.org/book/".to_string()),
            confidence: 0.98,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let _ = kb.add_entry(entry);

        // Search
        let results = kb.search("rust ownership");

        println!("Search: 'rust ownership'");
        println!("Found {} results", results.len());
        for entry in results {
            println!("  * {}", entry.title);
        }

        Ok(())
    }

    /// AI evaluator integration
    #[allow(dead_code)]
    pub async fn ai_integration(api_key: &str) -> Result<()> {
        use crate::ai_evaluator::AiEvaluator;
        use crate::knowledge_base::KnowledgeBase;
        use crate::llm::LlmClientBuilder;

        println!("=== AI Evaluator + Knowledge Base ===");
        println!();

        let _kb = KnowledgeBase::new();
        let llm = LlmClientBuilder::new()
            .openai_api_key(api_key)
            .build()
            .expect("Failed to build LLM client");

        let _evaluator = AiEvaluator::new(llm);

        println!("Integration workflow:");
        println!("  1. Search knowledge base for relevant info");
        println!("  2. Include in evaluation context");
        println!("  3. AI provides domain-aware feedback");
        println!();
        println!("Example:");
        println!("  Code: Solidity smart contract");
        println!("  KB: Retrieve Solidity best practices");
        println!("  AI: Evaluate against known patterns");
        println!("  Result: More accurate, context-aware review");

        Ok(())
    }

    /// Persistence (save/load)
    #[allow(dead_code)]
    pub async fn persistence() -> Result<()> {
        use crate::knowledge_base::KnowledgeBase;

        println!("=== Knowledge Base Persistence ===");
        println!();

        let kb = KnowledgeBase::new();

        // Save to file
        kb.save_to_file("/tmp/knowledge_base.json")?;
        println!("Saved to /tmp/knowledge_base.json");

        // Load from file
        let _loaded_kb = KnowledgeBase::load_from_file("/tmp/knowledge_base.json")?;
        println!("Loaded from /tmp/knowledge_base.json");
        println!();
        println!("Benefits:");
        println!("  * Preserve domain knowledge");
        println!("  * Share across team");
        println!("  * Version control");
        println!("  * Backup and restore");

        Ok(())
    }

    /// Multi-domain categorization
    #[allow(dead_code)]
    pub async fn multi_domain() -> Result<()> {
        use crate::knowledge_base::KnowledgeBase;

        println!("=== Multi-Domain Knowledge Base ===");
        println!();

        let _kb = KnowledgeBase::new();

        println!("Supported domains:");
        println!("  * Programming (Rust, Python, JavaScript, etc.)");
        println!("  * Blockchain (Ethereum, Bitcoin, Solana, etc.)");
        println!("  * Security (OWASP, CVEs, best practices)");
        println!("  * MachineLearning (models, algorithms, frameworks)");
        println!("  * DevOps (CI/CD, containers, cloud)");
        println!("  * General (miscellaneous knowledge)");
        println!();
        println!("Query by domain:");
        println!("  let blockchain = kb.get_by_domain(KnowledgeDomain::Blockchain);");
        println!("  let security = kb.get_by_domain(KnowledgeDomain::Security);");

        Ok(())
    }
}

/// Performance profiling examples
///
/// This example demonstrates how to use the performance profiling utilities
/// to track, measure, and optimize AI service performance.
pub struct PerformanceProfilingExample;

impl PerformanceProfilingExample {
    /// Basic operation profiling
    pub async fn basic_profiling() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Basic Performance Profiling ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Record some operations
        println!("Recording operations...");
        for i in 0..5 {
            let metrics = OperationMetrics::new(
                "code_evaluation".to_string(),
                Duration::from_millis(100 + i * 50),
                true,
            )
            .with_tokens(1500)
            .with_cost(0.002);

            profiler.record(metrics);
        }

        // Get statistics
        if let Some(stats) = profiler.get_stats("code_evaluation") {
            println!("Code Evaluation Statistics:");
            println!("  Total operations: {}", stats.total_count);
            println!("  Success rate: {:.1}%", stats.success_rate * 100.0);
            println!("  Avg duration: {:.2}ms", stats.avg_duration.as_millis());
            println!("  Min duration: {:.2}ms", stats.min_duration.as_millis());
            println!("  Max duration: {:.2}ms", stats.max_duration.as_millis());
            println!("  Total tokens: {}", stats.total_tokens);
            println!("  Total cost: ${:.4}", stats.total_cost);
        }

        Ok(())
    }

    /// Scoped profiling with automatic timing
    pub async fn scoped_profiling() -> Result<()> {
        use crate::profiling::{PerformanceProfiler, ScopedProfiler};
        use std::sync::Arc;

        println!("=== Scoped Profiling ===");
        println!();

        let profiler = Arc::new(PerformanceProfiler::new());

        println!("Profiling code evaluation...");
        {
            let _scope = ScopedProfiler::new("code_evaluation".to_string(), profiler.clone());
            // Simulate work
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            // _scope automatically records when dropped
        }

        println!("Profiling fraud detection...");
        {
            let _scope = ScopedProfiler::new("fraud_detection".to_string(), profiler.clone());
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // Generate report
        let report = profiler.generate_report();
        println!();
        println!("Performance Report:");
        println!("  Total operations: {}", report.total_operations);
        println!("  Total cost: ${:.4}", report.total_cost);
        println!("  Total tokens: {}", report.total_tokens);
        println!("  Operations by type: {}", report.operation_stats.len());

        Ok(())
    }

    /// Comparative performance analysis
    pub async fn comparative_analysis() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Comparative Performance Analysis ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Simulate GPT-4 operations
        println!("Recording GPT-4 operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("gpt-4".to_string(), Duration::from_millis(200), true)
                    .with_tokens(2000)
                    .with_cost(0.04);
            profiler.record(metrics);
        }

        // Simulate Claude operations
        println!("Recording Claude operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("claude".to_string(), Duration::from_millis(180), true)
                    .with_tokens(1800)
                    .with_cost(0.036);
            profiler.record(metrics);
        }

        // Simulate Gemini operations
        println!("Recording Gemini operations...");
        for _ in 0..10 {
            let metrics =
                OperationMetrics::new("gemini".to_string(), Duration::from_millis(150), true)
                    .with_tokens(1900)
                    .with_cost(0.019);
            profiler.record(metrics);
        }

        println!();
        println!("Performance Comparison:");
        println!("+---------+----------+----------+------------+----------+");
        println!("| Model   | Avg Time | Tokens   | Total Cost | Success  |");
        println!("+---------+----------+----------+------------+----------+");

        for model in ["gpt-4", "claude", "gemini"] {
            if let Some(stats) = profiler.get_stats(model) {
                println!(
                    "| {:<7} | {:>6}ms | {:>8} | ${:>9.4} | {:>7.1}% |",
                    model,
                    stats.avg_duration.as_millis(),
                    stats.total_tokens,
                    stats.total_cost,
                    stats.success_rate * 100.0
                );
            }
        }
        println!("+---------+----------+----------+------------+----------+");

        println!();
        println!("Winner: Gemini (fastest + cheapest)");

        Ok(())
    }

    /// Cost tracking and optimization
    pub async fn cost_optimization() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Cost Optimization Tracking ===");
        println!();

        let profiler = PerformanceProfiler::new();

        // Track operations over time
        println!("Tracking costs over 24 hours simulation...");

        // Hour 1-8: Expensive operations
        for _ in 0..100 {
            let metrics =
                OperationMetrics::new("evaluation".to_string(), Duration::from_millis(200), true)
                    .with_tokens(2000)
                    .with_cost(0.04);
            profiler.record(metrics);
        }

        let report_before = profiler.generate_report();
        println!("Before optimization:");
        println!("  Total cost: ${:.2}", report_before.total_cost);
        println!(
            "  Avg cost/op: ${:.4}",
            report_before.total_cost / report_before.total_operations as f64
        );

        // Hour 9-24: Optimized operations (cheaper model, caching)
        for _ in 0..100 {
            let metrics = OperationMetrics::new(
                "evaluation_optimized".to_string(),
                Duration::from_millis(150),
                true,
            )
            .with_tokens(1800)
            .with_cost(0.018); // 55% cost reduction
            profiler.record(metrics);
        }

        println!();
        println!("After optimization:");
        if let Some(stats) = profiler.get_stats("evaluation_optimized") {
            let avg_cost = if stats.total_count > 0 {
                stats.total_cost / stats.total_count as f64
            } else {
                0.0
            };
            println!("  Total cost: ${:.2}", stats.total_cost);
            println!("  Avg cost/op: ${avg_cost:.4}");
            println!("  Cost savings: 55%");
            println!("  Monthly projection: ${:.2}", stats.total_cost * 15.0); // 30 days worth
        }

        println!();
        println!("Optimization strategies applied:");
        println!("  * Switched to cheaper model for simple tasks");
        println!("  * Implemented response caching");
        println!("  * Batched similar requests");

        Ok(())
    }

    /// Real-time performance monitoring
    pub async fn realtime_monitoring() -> Result<()> {
        use crate::profiling::{OperationMetrics, PerformanceProfiler};
        use std::time::Duration;

        println!("=== Real-time Performance Monitoring ===");
        println!();

        let mut profiler = PerformanceProfiler::new();
        profiler.enable();

        println!("Processing operations...");
        for i in 0..5 {
            let success = i < 4; // Last one fails
            let metrics = OperationMetrics::new(
                "api_call".to_string(),
                Duration::from_millis(100 + i * 30),
                success,
            )
            .with_tokens((1000 + i * 200) as u32)
            .with_cost(0.02 + i as f64 * 0.005);

            if success {
                profiler.record(metrics);
            } else {
                let metrics = metrics.with_error("Rate limit exceeded".to_string());
                profiler.record(metrics);
            }

            // Show live stats
            if let Some(stats) = profiler.get_stats("api_call") {
                println!(
                    "[{}] Requests: {} | Success: {:.1}% | Avg latency: {}ms | Cost: ${:.4}",
                    i + 1,
                    stats.total_count,
                    stats.success_rate * 100.0,
                    stats.avg_duration.as_millis(),
                    stats.total_cost
                );
            }
        }

        println!();
        println!("Monitoring alerts:");
        if let Some(stats) = profiler.get_stats("api_call") {
            if stats.success_rate < 0.95 {
                println!("  Warning: Success rate below 95%");
            }
            if stats.avg_duration.as_millis() > 150 {
                println!("  Warning: Average latency above threshold");
            }
        }

        Ok(())
    }
}

/// Model version management examples
///
/// This example demonstrates how to track, compare, and manage
/// different AI model versions for optimal performance.
pub struct ModelVersionManagementExample;

impl ModelVersionManagementExample {
    /// Basic model version tracking
    pub async fn basic_version_tracking() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Basic Model Version Tracking ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register multiple model versions
        println!("Registering model versions...");

        let gpt4_v1 = ModelVersion::new("gpt-4-turbo", "20240301", "GPT-4 Turbo March 2024");
        registry.register_version(gpt4_v1)?;

        let gpt4_v2 = ModelVersion::new(
            "gpt-4-turbo",
            "20240601",
            "GPT-4 Turbo June 2024 - Improved reasoning",
        );
        registry.register_version(gpt4_v2)?;

        println!("Registered versions:");
        for version in registry.get_model_versions("gpt-4-turbo") {
            println!("  * {} ({})", version.version, version.description);
        }

        // Update metrics
        println!();
        println!("Recording performance metrics...");
        let metrics_v1 = ModelMetrics::new(94.5, 1500, 0.04);
        registry.update_metrics("gpt-4-turbo", "20240301", metrics_v1)?;

        let metrics_v2 = ModelMetrics::new(96.2, 1400, 0.042);
        registry.update_metrics("gpt-4-turbo", "20240601", metrics_v2)?;

        println!("Metrics updated successfully");

        Ok(())
    }

    /// Version comparison and recommendation
    pub async fn version_comparison() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Model Version Comparison ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register Claude versions
        let claude_opus =
            ModelVersion::new("claude-3-opus", "20240229", "Claude 3 Opus - Most capable");
        registry.register_version(claude_opus)?;

        let claude_sonnet = ModelVersion::new(
            "claude-3-5-sonnet",
            "20241022",
            "Claude 3.5 Sonnet - Balanced",
        );
        registry.register_version(claude_sonnet)?;

        let claude_haiku =
            ModelVersion::new("claude-3-haiku", "20240307", "Claude 3 Haiku - Fastest");
        registry.register_version(claude_haiku)?;

        // Add realistic metrics
        registry.update_metrics(
            "claude-3-opus",
            "20240229",
            ModelMetrics::new(97.8, 2000, 0.075),
        )?;

        registry.update_metrics(
            "claude-3-5-sonnet",
            "20241022",
            ModelMetrics::new(96.5, 1800, 0.045),
        )?;

        registry.update_metrics(
            "claude-3-haiku",
            "20240307",
            ModelMetrics::new(93.2, 1200, 0.0125),
        )?;

        // Compare versions
        println!("Claude Model Comparison:");
        println!("+------------------+----------+--------+----------+-------------+");
        println!("| Model            | Accuracy | Tokens | Avg Cost | Cost/Acc    |");
        println!("+------------------+----------+--------+----------+-------------+");

        for (model, version) in [
            ("claude-3-opus", "20240229"),
            ("claude-3-5-sonnet", "20241022"),
            ("claude-3-haiku", "20240307"),
        ] {
            if let Some(v) = registry.get_version(model, version) {
                println!(
                    "| {:<16} | {:>7.1}% | {:>6} | ${:>7.4} | {:>11.2} |",
                    model,
                    v.metrics.accuracy,
                    v.metrics.avg_tokens,
                    v.metrics.avg_cost,
                    v.metrics.cost_effectiveness()
                );
            }
        }
        println!("+------------------+----------+--------+----------+-------------+");

        println!();
        println!("Recommendations:");
        println!("  * For maximum accuracy: claude-3-opus (97.8%)");
        println!("  * For best value: claude-3-5-sonnet (2144 cost-effectiveness)");
        println!("  * For lowest cost: claude-3-haiku ($0.0125/request)");

        Ok(())
    }

    /// Model performance over time
    pub async fn performance_tracking() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== Model Performance Over Time ===");
        println!();

        let mut registry = ModelRegistry::new();

        let gpt4 = ModelVersion::new("gpt-4-turbo", "20240801", "GPT-4 Turbo August 2024");
        registry.register_version(gpt4)?;

        println!("Tracking performance over time...");
        println!();

        // Simulate performance data collection
        let mut metrics = ModelMetrics::new(95.0, 1500, 0.04);

        // Week 1
        for _ in 0..100 {
            metrics.record_request(1500, 0.04, 200.0, true);
        }
        println!(
            "Week 1: Accuracy={:.1}%, Requests={}, Cost=${:.2}",
            metrics.accuracy,
            metrics.total_requests,
            metrics.avg_cost * metrics.total_requests as f64
        );

        // Week 2 - some errors
        for i in 0..100 {
            let success = i % 10 != 0; // 10% error rate
            metrics.record_request(1500, 0.04, 220.0, success);
        }
        println!(
            "Week 2: Accuracy={:.1}%, Requests={}, Errors={}",
            metrics.accuracy, metrics.total_requests, metrics.total_errors
        );

        // Week 3 - optimized
        for _ in 0..100 {
            metrics.record_request(1400, 0.038, 180.0, true);
        }
        println!(
            "Week 3: Accuracy={:.1}%, Avg Latency={:.1}ms, Cost=${:.4}",
            metrics.accuracy, metrics.avg_latency_ms, metrics.avg_cost
        );

        registry.update_metrics("gpt-4-turbo", "20240801", metrics)?;

        println!();
        println!("Analysis:");
        if let Some(v) = registry.get_version("gpt-4-turbo", "20240801") {
            println!("  Total requests: {}", v.metrics.total_requests);
            println!("  Error rate: {:.2}%", v.metrics.error_rate());
            println!("  Average latency: {:.1}ms", v.metrics.avg_latency_ms);
            println!(
                "  Cost effectiveness: {:.2}",
                v.metrics.cost_effectiveness()
            );
        }

        Ok(())
    }

    /// A/B testing model versions
    pub async fn ab_testing() -> Result<()> {
        use crate::model_version::{ModelMetrics, ModelRegistry, ModelVersion};

        println!("=== A/B Testing Model Versions ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register two versions for testing
        let version_a = ModelVersion::new("custom-model", "v1.0", "Baseline model");
        registry.register_version(version_a)?;

        let version_b = ModelVersion::new(
            "custom-model",
            "v1.1",
            "Optimized model with new training data",
        );
        registry.register_version(version_b)?;

        println!("A/B Test: custom-model v1.0 vs v1.1");
        println!();

        // Simulate A/B test traffic split (50/50)
        let mut metrics_a = ModelMetrics::new(94.0, 1600, 0.035);
        let mut metrics_b = ModelMetrics::new(95.5, 1550, 0.034);

        println!("Running A/B test with 1000 requests...");
        for _ in 0..500 {
            metrics_a.record_request(1600, 0.035, 195.0, true);
            metrics_b.record_request(1550, 0.034, 185.0, true);
        }

        registry.update_metrics("custom-model", "v1.0", metrics_a)?;
        registry.update_metrics("custom-model", "v1.1", metrics_b)?;

        println!();
        println!("A/B Test Results:");
        println!("+-----------+----------+----------+----------+------------+");
        println!("| Version   | Accuracy | Latency  | Avg Cost | Recommend  |");
        println!("+-----------+----------+----------+----------+------------+");

        if let Some(v1) = registry.get_version("custom-model", "v1.0") {
            println!(
                "| v1.0 (A)  | {:>7.1}% | {:>6.0}ms | ${:>6.4} |            |",
                v1.metrics.accuracy, v1.metrics.avg_latency_ms, v1.metrics.avg_cost
            );
        }
        if let Some(v2) = registry.get_version("custom-model", "v1.1") {
            println!(
                "| v1.1 (B)  | {:>7.1}% | {:>6.0}ms | ${:>6.4} | Winner     |",
                v2.metrics.accuracy, v2.metrics.avg_latency_ms, v2.metrics.avg_cost
            );
        }
        println!("+-----------+----------+----------+----------+------------+");

        println!();
        println!("Decision: Promote v1.1 to production");
        println!("Improvements:");
        println!("  * +1.5% accuracy improvement");
        println!("  * -10ms latency reduction");
        println!("  * -2.9% cost reduction");

        // Set active version
        registry.set_active_version("custom-model", "v1.1")?;
        println!();
        println!("v1.1 is now the active version");

        Ok(())
    }

    /// Model deprecation workflow
    pub async fn deprecation_workflow() -> Result<()> {
        use crate::model_version::{ModelRegistry, ModelVersion};

        println!("=== Model Deprecation Workflow ===");
        println!();

        let mut registry = ModelRegistry::new();

        // Register old and new versions
        let old_version = ModelVersion::new("gpt-3.5-turbo", "0301", "GPT-3.5 Turbo - March 2023");
        registry.register_version(old_version)?;

        let new_version = ModelVersion::new("gpt-4-turbo", "20240401", "GPT-4 Turbo - April 2024");
        registry.register_version(new_version)?;

        println!("Models registered:");
        println!("  * gpt-3.5-turbo (0301) - Old version");
        println!("  * gpt-4-turbo (20240401) - New version");

        // Mark old version as deprecated
        println!();
        println!("Deprecating gpt-3.5-turbo...");
        registry.deprecate_version("gpt-3.5-turbo", "0301")?;

        println!("Model marked as deprecated");
        println!();
        println!("Migration path:");
        println!("  1. Update applications to use gpt-4-turbo");
        println!("  2. Monitor performance metrics");
        println!("  3. Gradual traffic shift (0% -> 100%)");
        println!("  4. Remove deprecated model after 90 days");

        // Set new version as active
        registry.set_active_version("gpt-4-turbo", "20240401")?;

        println!();
        println!("Active models:");
        if let Some(active) = registry.get_active_version("gpt-4-turbo") {
            println!("  * {} - {}", active.version, active.description);
        }

        println!();
        println!("Deprecated models:");
        println!("  gpt-3.5-turbo (0301) - Deprecated");

        Ok(())
    }
}
