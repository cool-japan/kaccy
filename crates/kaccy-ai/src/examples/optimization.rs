//! Optimization and reliability examples: cost optimization, resilience patterns,
//! budget management, and access control.

use crate::error::Result;

/// Example: Cost Optimization
///
/// This example demonstrates various cost optimization strategies including
/// model tier selection, caching, batching, and cost tracking.
pub struct CostOptimizationExample;

impl CostOptimizationExample {
    /// Model tier selection strategies
    #[allow(dead_code)]
    pub async fn model_tier_selection() -> Result<()> {
        println!("=== Model Tier Selection for Cost Optimization ===");
        println!();

        println!("Task-based routing:");
        println!();
        println!("Simple tasks (sentiment, classification):");
        println!("  -> Gemini 1.5 Flash or DeepSeek Chat");
        println!("  -> Cost: ~$0.14/M tokens");
        println!("  -> Use when: Fast, cheap answers needed");
        println!();
        println!("Medium tasks (code review, summarization):");
        println!("  -> Claude 3.5 Sonnet or GPT-4 Turbo");
        println!("  -> Cost: ~$3-10/M tokens");
        println!("  -> Use when: Quality matters, moderate complexity");
        println!();
        println!("Complex tasks (reasoning, critical decisions):");
        println!("  -> Claude 3 Opus or GPT-4");
        println!("  -> Cost: ~$15-30/M tokens");
        println!("  -> Use when: Highest quality required");
        println!();
        println!("Development/testing:");
        println!("  -> Ollama (local, free)");
        println!("  -> Cost: $0");
        println!("  -> Use when: Prototyping, no API costs desired");

        Ok(())
    }

    /// Task complexity-based routing
    #[allow(dead_code)]
    pub async fn complexity_based_routing(openai_key: &str, anthropic_key: &str) -> Result<()> {
        use crate::llm::LlmClientBuilder;

        println!("=== Automatic Complexity-Based Routing ===");
        println!();

        let _client = LlmClientBuilder::new()
            .openai_api_key(openai_key)
            .anthropic_api_key(anthropic_key)
            .build()
            .expect("Failed to build LLM client");

        println!("Automatic routing:");
        println!("  Simple task  -> gemini-1.5-flash");
        println!("  Medium task  -> claude-3.5-sonnet");
        println!("  Complex task -> claude-3-opus");
        println!();
        println!("Estimated savings: 70-90% vs using GPT-4 for everything");

        Ok(())
    }

    /// Response caching implementation
    #[allow(dead_code)]
    pub async fn caching_example(_api_key: &str) -> Result<()> {
        println!("=== Response Caching for Cost Reduction ===");
        println!();

        println!("Note: Caching example (conceptual):");

        println!("Cache configuration:");
        println!("  Max size: 1000 entries");
        println!("  TTL: 1 hour");
        println!("  Semantic matching: Enabled (95% threshold)");
        println!();
        println!("Example:");
        println!("  First request: 'What is blockchain?' -> API call ($)");
        println!("  Second request: 'What is blockchain?' -> Cache hit (FREE)");
        println!("  Similar request: 'Explain blockchain' -> Cache hit (FREE)");
        println!();
        println!("Typical savings: 60-80% for repeated queries");

        Ok(())
    }

    /// Request batching techniques
    #[allow(dead_code)]
    pub async fn batching_example(_api_key: &str) -> Result<()> {
        println!("=== Request Batching for Cost Efficiency ===");
        println!();

        println!("Note: Batch processing example (conceptual):");

        println!("Batch configuration:");
        println!("  Max concurrency: 5 requests in parallel");
        println!("  Delay: 100ms between batches");
        println!();
        println!("Benefits:");
        println!("  * Shared prompt overhead");
        println!("  * Better rate limit utilization");
        println!("  * Reduced API call overhead");
        println!();
        println!("Example: Evaluating 100 code submissions");
        println!("  Without batching: 100 API calls");
        println!("  With batching (20 per batch): 5 API calls");
        println!("  Cost reduction: ~80%");

        Ok(())
    }

    /// Cost tracking and monitoring
    #[allow(dead_code)]
    pub async fn cost_tracking_example() -> Result<()> {
        use crate::llm::MetricsCollector;

        println!("=== Cost Tracking and Monitoring ===");
        println!();

        let _metrics = MetricsCollector::new();

        println!("Track costs in real-time:");
        println!("  * Total API calls made");
        println!("  * Tokens used (input + output)");
        println!("  * Estimated cost per provider");
        println!("  * Cost per operation type");
        println!();
        println!("Example metrics:");
        println!("  Total requests: 1,250");
        println!("  Total tokens: 5.2M");
        println!("  Estimated cost: $12.50");
        println!("  Average per request: $0.01");
        println!();
        println!("Set budgets and alerts:");
        println!("  * Daily budget: $100");
        println!("  * Warning at 75% ($75)");
        println!("  * Stop at 100% ($100)");

        Ok(())
    }

    /// Complete cost optimization case study
    #[allow(dead_code)]
    pub async fn cost_optimization_case_study() -> Result<()> {
        println!("=== Cost Optimization Case Study ===");
        println!();

        println!("Scenario: Code review platform (10,000 reviews/month)");
        println!();
        println!("BEFORE optimization:");
        println!("  Model: GPT-4 Turbo for all reviews");
        println!("  Avg tokens: 4,000 per review (2K in, 2K out)");
        println!("  Cost per review: $0.54");
        println!("  Monthly cost: $5,400");
        println!();
        println!("AFTER optimization:");
        println!("  70% simple reviews -> Gemini Flash ($0.04 each)");
        println!("  25% medium reviews -> Claude Sonnet ($0.24 each)");
        println!("  5% complex reviews -> GPT-4 ($0.54 each)");
        println!("  50% cache hits -> FREE");
        println!();
        println!("New monthly cost:");
        println!("  Simple (3,500 reviews): $140");
        println!("  Medium (1,250 reviews): $300");
        println!("  Complex (250 reviews): $135");
        println!("  Cache savings: -$125");
        println!("  Total: $450/month");
        println!();
        println!("SAVINGS: $4,950/month (92% reduction!)");

        Ok(())
    }
}

/// Example: Resilience and Reliability
///
/// This example demonstrates resilience patterns including circuit breakers,
/// retry logic, health monitoring, and rate limiting.
pub struct ResilienceExample;

impl ResilienceExample {
    /// Circuit breaker pattern demonstration
    #[allow(dead_code)]
    pub async fn circuit_breaker_example() -> Result<()> {
        use crate::llm::{CircuitBreaker, CircuitBreakerConfig};

        println!("=== Circuit Breaker Pattern ===");
        println!();

        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: std::time::Duration::from_secs(60),
            failure_window: std::time::Duration::from_secs(60),
        };

        let _breaker = CircuitBreaker::new("openai".to_string(), config);

        println!("Circuit breaker configuration:");
        println!("  Failure threshold: 5 consecutive failures");
        println!("  Success threshold: 2 successes to recover");
        println!("  Timeout: 60 seconds in open state");
        println!();
        println!("States:");
        println!("  CLOSED: Normal operation, all requests pass");
        println!("  OPEN: Too many failures, reject requests");
        println!("  HALF-OPEN: Testing recovery, limited requests");
        println!();
        println!("Benefits:");
        println!("  * Prevent cascading failures");
        println!("  * Fail fast when service is down");
        println!("  * Automatic recovery detection");
        println!("  * Protect downstream services");

        Ok(())
    }

    /// Retry logic with exponential backoff
    #[allow(dead_code)]
    pub async fn retry_example() -> Result<()> {
        use crate::llm::RetryConfig;

        println!("=== Retry Logic with Exponential Backoff ===");
        println!();

        let _config = RetryConfig {
            max_attempts: 3,
            initial_delay: std::time::Duration::from_millis(1000),
            max_delay: std::time::Duration::from_millis(10000),
            backoff_multiplier: 2.0,
            use_jitter: true,
        };

        // Note: RetryExecutor would be created with config

        println!("Retry configuration:");
        println!("  Max retries: 3");
        println!("  Initial delay: 1 second");
        println!("  Backoff multiplier: 2.0");
        println!("  Max delay: 10 seconds");
        println!("  Jitter: Enabled");
        println!();
        println!("Retry timeline:");
        println!("  Attempt 1: Immediate");
        println!("  Attempt 2: Wait 1s (+jitter)");
        println!("  Attempt 3: Wait 2s (+jitter)");
        println!("  Attempt 4: Wait 4s (+jitter)");
        println!();
        println!("Retryable errors:");
        println!("  * Rate limit exceeded (429)");
        println!("  * Service unavailable (503)");
        println!("  * Gateway timeout (504)");
        println!("  * Network errors");

        Ok(())
    }

    /// Health monitoring and failover
    #[allow(dead_code)]
    pub async fn health_monitoring_example() -> Result<()> {
        println!("=== Health Monitoring and Failover ===");
        println!();

        println!("Note: Health monitoring example (conceptual):");
        println!("In production, you would:");
        println!("  1. Create HealthMonitor with config");
        println!("  2. Register providers");
        println!("  3. Monitor health metrics");

        println!("Health monitoring configuration:");
        println!("  Check interval: 60 seconds");
        println!("  Unhealthy threshold: 3 failures");
        println!("  Healthy threshold: 2 successes");
        println!();
        println!("Tracked metrics:");
        println!("  * Success rate");
        println!("  * Average response time");
        println!("  * Consecutive failures");
        println!("  * Health score (0-100)");
        println!();
        println!("Automatic failover:");
        println!("  If OpenAI unhealthy -> Use Anthropic");
        println!("  If Anthropic unhealthy -> Use Gemini");
        println!("  Select healthiest provider automatically");

        Ok(())
    }

    /// Rate limiting implementation
    #[allow(dead_code)]
    pub async fn rate_limiting_example() -> Result<()> {
        println!("=== Rate Limiting ===");
        println!();

        // Note: RateLimiter config (conceptual)

        println!("Rate limiter configuration:");
        println!("  Requests per second: 10");
        println!("  Burst size: 20");
        println!();
        println!("Token bucket algorithm:");
        println!("  * Bucket holds 20 tokens (burst)");
        println!("  * Refills at 10 tokens/second");
        println!("  * Each request consumes 1 token");
        println!();
        println!("Example:");
        println!("  Burst: Process 20 requests immediately");
        println!("  Sustained: 10 requests/second max");
        println!("  Over limit: Wait for tokens to refill");
        println!();
        println!("Benefits:");
        println!("  * Prevent API rate limit errors");
        println!("  * Smooth traffic spikes");
        println!("  * Predictable throughput");

        Ok(())
    }

    /// Complete resilience stack
    #[allow(dead_code)]
    pub async fn complete_resilience_stack(_api_key: &str) -> Result<()> {
        println!("=== Complete Resilience Stack ===");
        println!();

        println!("Layered resilience approach:");
        println!();
        println!("1. Rate Limiting (outermost)");
        println!("   -> Prevent overwhelming APIs");
        println!();
        println!("2. Circuit Breaker");
        println!("   -> Fail fast when service is down");
        println!();
        println!("3. Retry Logic");
        println!("   -> Handle transient failures");
        println!();
        println!("4. Health Monitoring");
        println!("   -> Track provider availability");
        println!();
        println!("5. Multi-Provider Fallback");
        println!("   -> Switch to backup provider");
        println!();
        println!("Result:");
        println!("  * 99.9% success rate (vs 95% without)");
        println!("  * Mean time to recovery: <1 minute");
        println!("  * Zero manual intervention");
        println!("  * Cost-efficient failover");

        Ok(())
    }
}

/// Example: Budget Management
///
/// This example demonstrates budget tracking, alerting, and cost control.
pub struct BudgetManagementExample;

impl BudgetManagementExample {
    /// Multi-period budget configuration
    #[allow(dead_code)]
    pub async fn budget_configuration() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager, BudgetPeriod};

        println!("=== Budget Configuration ===");
        println!();

        let mut config = BudgetConfig::default();
        config.set_limit(BudgetPeriod::Hourly, 10.0);
        config.set_limit(BudgetPeriod::Daily, 100.0);
        config.set_limit(BudgetPeriod::Weekly, 500.0);
        config.set_limit(BudgetPeriod::Monthly, 2000.0);

        let _manager = BudgetManager::new(config);

        println!("Budget limits configured:");
        println!("  Hourly:  $10");
        println!("  Daily:   $100");
        println!("  Weekly:  $500");
        println!("  Monthly: $2,000");
        println!();
        println!("Auto-reset:");
        println!("  * Hourly budget resets every hour");
        println!("  * Daily budget resets at midnight");
        println!("  * Weekly budget resets on Monday");
        println!("  * Monthly budget resets on 1st");

        Ok(())
    }

    /// Usage recording and tracking
    #[allow(dead_code)]
    pub async fn usage_tracking() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager};

        println!("=== Usage Tracking ===");
        println!();

        let manager = BudgetManager::new(BudgetConfig::default());

        // Simulate usage
        manager.record_cost(15.50).await?;
        manager.record_cost(8.25).await?;
        manager.record_cost(12.00).await?;

        let daily_usage = manager.get_usage(crate::llm::BudgetPeriod::Daily).await;
        let daily = daily_usage.map_or(0.0, |u| u.total_cost);

        println!("Current usage:");
        println!("  Today: ${daily:.2}");
        println!();
        println!("Tracked metrics:");
        println!("  * Number of requests");
        println!("  * Tokens consumed");
        println!("  * Cost per provider");
        println!("  * Cost per period");

        Ok(())
    }

    /// Alert system (Info, Warning, Critical, Exceeded)
    #[allow(dead_code)]
    pub async fn alert_system() -> Result<()> {
        println!("=== Budget Alert System ===");
        println!();

        println!("Alert levels:");
        println!();
        println!("INFO (50%):");
        println!("  * Budget: $100, Used: $50");
        println!("  * Action: Log for awareness");
        println!();
        println!("WARNING (75%):");
        println!("  * Budget: $100, Used: $75");
        println!("  * Action: Send notification");
        println!();
        println!("CRITICAL (90%):");
        println!("  * Budget: $100, Used: $90");
        println!("  * Action: Alert on-call team");
        println!();
        println!("EXCEEDED (100%):");
        println!("  * Budget: $100, Used: $100+");
        println!("  * Action: Block new requests (optional)");
        println!();
        println!("Custom alert handlers:");
        println!("  manager.on_alert(|alert| {{");
        println!("    match alert.level {{");
        println!("      AlertLevel::Warning => send_email(),");
        println!("      AlertLevel::Critical => send_sms(),");
        println!("      AlertLevel::Exceeded => pause_service(),");
        println!("      _ => log_info(),");
        println!("    }}");
        println!("  }});");

        Ok(())
    }

    /// Budget monitoring and reporting
    #[allow(dead_code)]
    pub async fn monitoring_and_reporting() -> Result<()> {
        use crate::llm::{BudgetConfig, BudgetManager, BudgetPeriod};

        println!("=== Budget Monitoring and Reporting ===");
        println!();

        let mut config = BudgetConfig::default();
        config.set_limit(BudgetPeriod::Daily, 100.0);

        let manager = BudgetManager::new(config);

        let remaining = manager
            .get_remaining(BudgetPeriod::Daily)
            .await
            .unwrap_or(100.0);
        let utilization = manager
            .get_utilization(BudgetPeriod::Daily)
            .await
            .unwrap_or(0.0);

        println!("Daily budget report:");
        println!("  Limit: $100.00");
        println!("  Used: $67.50");
        println!("  Remaining: ${remaining:.2}");
        println!("  Utilization: {:.1}%", utilization * 100.0);
        println!();
        println!("Trend analysis:");
        println!("  * Average daily spend: $65");
        println!("  * Projected monthly: $1,950");
        println!("  * vs Monthly budget: $2,000 (check)");
        println!();
        println!("Recommendations:");
        println!("  * Current pace is sustainable");
        println!("  * Consider caching to reduce costs");
        println!("  * Peak usage: 2-4pm UTC");

        Ok(())
    }

    /// Cost projection and optimization
    #[allow(dead_code)]
    pub async fn cost_projection() -> Result<()> {
        println!("=== Cost Projection and Optimization ===");
        println!();

        println!("Current usage pattern:");
        println!("  Week 1: $450");
        println!("  Week 2: $480");
        println!("  Week 3: $520");
        println!("  Week 4: $550");
        println!();
        println!("Projection:");
        println!("  Month total: ~$2,000");
        println!("  Next month: ~$2,200 (10% growth)");
        println!();
        println!("Optimization opportunities:");
        println!("  1. Enable caching -> Save $400/month (20%)");
        println!("  2. Use Gemini Flash for simple tasks -> Save $300/month (15%)");
        println!("  3. Batch requests -> Save $200/month (10%)");
        println!();
        println!("Optimized projection:");
        println!("  Current: $2,000/month");
        println!("  Optimized: $1,100/month");
        println!("  Savings: $900/month (45%)");

        Ok(())
    }
}

/// Example: Access Control
///
/// This example demonstrates token-gated AI access with tier-based features
/// and custom AI agents.
pub struct AccessControlExample;

impl AccessControlExample {
    /// Token-gated access configuration
    #[allow(dead_code)]
    pub async fn access_tiers() -> Result<()> {
        println!("=== Token-Gated AI Access Tiers ===");
        println!();

        println!("Access tiers based on token holdings:");
        println!();
        println!("FREE (0-99 tokens):");
        println!("  * Basic code evaluation");
        println!("  * 10 requests/day");
        println!("  * No custom agents");
        println!();
        println!("BRONZE (100-999 tokens):");
        println!("  * Code + content evaluation");
        println!("  * 100 requests/day");
        println!("  * 1 custom agent");
        println!();
        println!("SILVER (1,000-9,999 tokens):");
        println!("  * All evaluations + fraud detection");
        println!("  * 1,000 requests/day");
        println!("  * 3 custom agents");
        println!();
        println!("GOLD (10,000-99,999 tokens):");
        println!("  * All features + batch processing");
        println!("  * 10,000 requests/day");
        println!("  * 10 custom agents");
        println!();
        println!("PLATINUM (100,000+ tokens):");
        println!("  * Unlimited access");
        println!("  * Priority processing");
        println!("  * Unlimited custom agents");

        Ok(())
    }

    /// Tier-based feature quotas
    #[allow(dead_code)]
    pub async fn feature_quotas() -> Result<()> {
        use crate::access_control::{AccessControlManager, AiFeature, TokenHolder};
        use rust_decimal::Decimal;
        use uuid::Uuid;

        println!("=== Tier-Based Feature Quotas ===");
        println!();

        let holder = TokenHolder {
            user_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            balance: Decimal::from(5000),
            tier: crate::access_control::AccessTier::Silver,
        };

        let manager = AccessControlManager::new();

        // Check access
        let can_evaluate = manager.can_access_feature(&holder, AiFeature::CodeEvaluation)?;
        let can_fraud = manager.can_access_feature(&holder, AiFeature::FraudDetection)?;

        println!("User: {} (SILVER tier, 5,000 tokens)", holder.user_id);
        println!();
        println!("Feature access:");
        println!(
            "  Code evaluation: {}",
            if can_evaluate { "Allowed" } else { "Denied" }
        );
        println!(
            "  Fraud detection: {}",
            if can_fraud { "Allowed" } else { "Denied" }
        );
        println!();
        println!("Daily quotas:");
        println!("  Code evaluations: 1,000/day");
        println!("  Fraud checks: 500/day");
        println!("  Custom agent calls: 2,000/day");

        Ok(())
    }

    /// Custom AI agent creation
    #[allow(dead_code)]
    pub async fn custom_agents(_api_key: &str) -> Result<()> {
        use crate::access_control::{AccessTier, CustomAgentConfig};
        use uuid::Uuid;

        println!("=== Custom AI Agents ===");
        println!();

        let agent = CustomAgentConfig {
            agent_id: Uuid::new_v4(),
            token_id: Uuid::new_v4(),
            name: "My Coding Assistant".to_string(),
            description: Some("Specialized in Rust code review".to_string()),
            system_prompt: "You are an expert Rust developer...".to_string(),
            model: "gpt-4-turbo-preview".to_string(),
            temperature: 0.3,
            is_active: true,
            min_tier: AccessTier::Silver,
            custom_endpoint: None,
            personalization: None,
        };

        println!("Custom agent created:");
        println!("  Name: {}", agent.name);
        println!("  Model: {}", agent.model);
        println!("  Specialization: {}", agent.description.as_ref().unwrap());
        println!("  Min tier: {:?}", agent.min_tier);
        println!();
        println!("Usage:");
        println!("  let client = LlmClientBuilder::new()");
        println!("      .openai_api_key(api_key)");
        println!("      .openai_model(&agent.model)");
        println!("      .build();");
        println!();
        println!("Benefits:");
        println!("  * Tailored to your domain");
        println!("  * Consistent responses");
        println!("  * Custom instructions");
        println!("  * Token holder exclusive");

        Ok(())
    }

    /// Usage monitoring per tier
    #[allow(dead_code)]
    pub async fn usage_monitoring() -> Result<()> {
        use crate::access_control::AccessControlManager;

        println!("=== Usage Monitoring Per Tier ===");
        println!();

        let _manager = AccessControlManager::new();

        println!("Track usage by tier:");
        println!();
        println!("FREE tier:");
        println!("  Active users: 1,250");
        println!("  Avg requests/user: 8/day");
        println!("  Total requests: 10,000/day");
        println!();
        println!("SILVER tier:");
        println!("  Active users: 150");
        println!("  Avg requests/user: 450/day");
        println!("  Total requests: 67,500/day");
        println!();
        println!("PLATINUM tier:");
        println!("  Active users: 10");
        println!("  Avg requests/user: 5,000/day");
        println!("  Total requests: 50,000/day");
        println!();
        println!("Insights:");
        println!("  * PLATINUM users drive 40% of usage");
        println!("  * Most common: Code evaluation (65%)");
        println!("  * Peak hours: 9am-5pm UTC");

        Ok(())
    }

    /// Network effects and economics
    #[allow(dead_code)]
    pub async fn network_effects() -> Result<()> {
        println!("=== Access Control Economics ===");
        println!();

        println!("Token utility:");
        println!("  * Gate premium AI features");
        println!("  * Create custom AI agents");
        println!("  * Priority processing queue");
        println!("  * Access to advanced models");
        println!();
        println!("Network effects:");
        println!("  * More users -> More token demand");
        println!("  * Higher tier -> Better features");
        println!("  * Custom agents -> Unique value");
        println!("  * Exclusive access -> Premium pricing");
        println!();
        println!("Economics:");
        println!("  * Free tier: Low-cost models (Gemini Flash)");
        println!("  * Paid tiers: Premium models (GPT-4, Claude Opus)");
        println!("  * Cost covered by: Token purchase requirements");
        println!("  * Revenue model: Token sales + transaction fees");

        Ok(())
    }
}
