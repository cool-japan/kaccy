//! Advanced analytics for kaccy-api
//!
//! This module provides comprehensive analytics tracking including:
//! - User behavior tracking
//! - Performance analytics
//! - Business intelligence metrics
//! - Predictive analytics

/// User behavior and session tracking
pub mod behavior;
/// Business metrics counters and gauges
pub mod metrics;
/// Request performance tracking
pub mod performance;
/// Central analytics tracker
pub mod tracker;

pub use behavior::{BehaviorTracker, UserAction};
pub use metrics::{BusinessMetrics, MetricValue};
pub use performance::{PerformanceMetric, PerformanceTracker};
pub use tracker::{AnalyticsTracker, TrackerConfig};

use std::sync::Arc;

/// Analytics manager that coordinates all analytics tracking
pub struct AnalyticsManager {
    tracker: Arc<AnalyticsTracker>,
    behavior: Arc<BehaviorTracker>,
    performance: Arc<PerformanceTracker>,
    business_metrics: Arc<BusinessMetrics>,
}

impl AnalyticsManager {
    /// Create a new analytics manager
    pub fn new(
        tracker: AnalyticsTracker,
        behavior: BehaviorTracker,
        performance: PerformanceTracker,
        business_metrics: BusinessMetrics,
    ) -> Self {
        Self {
            tracker: Arc::new(tracker),
            behavior: Arc::new(behavior),
            performance: Arc::new(performance),
            business_metrics: Arc::new(business_metrics),
        }
    }

    /// Get analytics tracker
    pub fn tracker(&self) -> Arc<AnalyticsTracker> {
        self.tracker.clone()
    }

    /// Get behavior tracker
    pub fn behavior(&self) -> Arc<BehaviorTracker> {
        self.behavior.clone()
    }

    /// Get performance tracker
    pub fn performance(&self) -> Arc<PerformanceTracker> {
        self.performance.clone()
    }

    /// Get business metrics
    pub fn business_metrics(&self) -> Arc<BusinessMetrics> {
        self.business_metrics.clone()
    }
}
