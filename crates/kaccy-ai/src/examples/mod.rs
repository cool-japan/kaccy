//! Usage examples and integration patterns
//!
//! This module provides practical examples of how to use the kaccy-ai crate
//! for various common scenarios. These examples can serve as starting points
//! for your own implementations.

pub mod basic;
pub mod detection;
pub mod integrations;
pub mod knowledge_profiling;
pub mod optimization;
pub mod reports_dashboard;

pub use basic::{
    BasicCodeEvaluationExample, BatchProcessingExample, CompleteServiceExample,
    FraudDetectionExample, IntegrationExample, OracleConsensusExample,
};
pub use detection::{ImageSimilarityExample, PlagiarismDetectionExample};
pub use integrations::{
    DeepSeekIntegrationExample, GeminiIntegrationExample, OllamaIntegrationExample,
};
pub use knowledge_profiling::{
    KnowledgeBaseExample, ModelVersionManagementExample, PerformanceProfilingExample,
};
pub use optimization::{
    AccessControlExample, BudgetManagementExample, CostOptimizationExample, ResilienceExample,
};
pub use reports_dashboard::{DashboardIntegrationExample, ReportGenerationExample};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_examples_compile() {
        // This test just ensures all examples compile
        // Actual execution requires API keys
        let _ = BasicCodeEvaluationExample;
        let _ = BatchProcessingExample;
        let _ = OracleConsensusExample;
        let _ = CompleteServiceExample;
        let _ = FraudDetectionExample;
        let _ = IntegrationExample;
        let _ = GeminiIntegrationExample;
        let _ = DeepSeekIntegrationExample;
        let _ = OllamaIntegrationExample;
        let _ = PlagiarismDetectionExample;
        let _ = ImageSimilarityExample;
        let _ = CostOptimizationExample;
        let _ = ResilienceExample;
        let _ = BudgetManagementExample;
        let _ = AccessControlExample;
        let _ = KnowledgeBaseExample;
        let _ = PerformanceProfilingExample;
        let _ = ModelVersionManagementExample;
        let _ = ReportGenerationExample;
        let _ = DashboardIntegrationExample;
    }
}
