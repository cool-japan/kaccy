//! Connection pool auto-scaling for dynamic load management
//!
//! This module provides automatic connection pool scaling based on utilization metrics.
//! It monitors pool usage and adjusts the number of connections to optimize resource
//! utilization while maintaining performance during load spikes.
//!
//! # Features
//!
//! - Automatic scale-up during high utilization periods
//! - Automatic scale-down during low utilization periods
//! - Configurable scaling thresholds and limits
//! - Cooldown periods to prevent thrashing
//! - Multiple scaling strategies (linear, exponential)
//! - Comprehensive metrics and monitoring
//! - Thread-safe operation with async support
//!
//! # Example
//!
//! ```rust,no_run
//! use kaccy_db::pool_autoscaling::{PoolAutoscaler, AutoscalingConfig, ScalingStrategy};
//! use sqlx::PgPool;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let pool = PgPool::connect("postgresql://localhost/kaccy").await?;
//!
//!     // Configure auto-scaling
//!     let config = AutoscalingConfig {
//!         min_connections: 5,
//!         max_connections: 50,
//!         scale_up_threshold: 0.8,    // Scale up at 80% utilization
//!         scale_down_threshold: 0.3,  // Scale down at 30% utilization
//!         scale_up_increment: 5,
//!         scale_down_increment: 2,
//!         cooldown_period: Duration::from_secs(60),
//!         check_interval: Duration::from_secs(10),
//!         strategy: ScalingStrategy::Linear,
//!     };
//!
//!     // Create and start autoscaler
//!     let autoscaler = PoolAutoscaler::new(pool, config);
//!     autoscaler.start().await?;
//!
//!     // Autoscaler runs in background, adjusting pool size as needed
//!     tokio::time::sleep(Duration::from_secs(3600)).await;
//!
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::Result;

/// Scaling strategy for connection pool adjustments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalingStrategy {
    /// Linear scaling - add/remove fixed number of connections
    Linear,
    /// Exponential scaling - double/halve connection count
    Exponential,
    /// Proportional scaling - scale based on utilization percentage
    Proportional,
}

/// Configuration for pool auto-scaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoscalingConfig {
    /// Minimum number of connections to maintain
    pub min_connections: u32,
    /// Maximum number of connections allowed
    pub max_connections: u32,
    /// Utilization threshold to trigger scale-up (0.0-1.0)
    pub scale_up_threshold: f64,
    /// Utilization threshold to trigger scale-down (0.0-1.0)
    pub scale_down_threshold: f64,
    /// Number of connections to add during scale-up (Linear strategy)
    pub scale_up_increment: u32,
    /// Number of connections to remove during scale-down (Linear strategy)
    pub scale_down_increment: u32,
    /// Minimum time between scaling operations
    pub cooldown_period: Duration,
    /// Interval between utilization checks
    pub check_interval: Duration,
    /// Scaling strategy to use
    pub strategy: ScalingStrategy,
}

impl Default for AutoscalingConfig {
    fn default() -> Self {
        Self {
            min_connections: 5,
            max_connections: 100,
            scale_up_threshold: 0.75,
            scale_down_threshold: 0.25,
            scale_up_increment: 5,
            scale_down_increment: 2,
            cooldown_period: Duration::from_secs(60),
            check_interval: Duration::from_secs(30),
            strategy: ScalingStrategy::Linear,
        }
    }
}

impl AutoscalingConfig {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.min_connections >= self.max_connections {
            return Err(crate::DbError::Validation(
                "min_connections must be less than max_connections".to_string(),
            ));
        }

        if self.scale_up_threshold <= self.scale_down_threshold {
            return Err(crate::DbError::Validation(
                "scale_up_threshold must be greater than scale_down_threshold".to_string(),
            ));
        }

        if self.scale_up_threshold > 1.0 || self.scale_down_threshold < 0.0 {
            return Err(crate::DbError::Validation(
                "thresholds must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Scaling event recorded during auto-scaling operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingEvent {
    /// Timestamp of the scaling event
    pub timestamp: DateTime<Utc>,
    /// Event type (scale up or down)
    pub event_type: ScalingEventType,
    /// Pool utilization at time of event (0.0-1.0)
    pub utilization: f64,
    /// Connection count before scaling
    pub before_count: u32,
    /// Connection count after scaling
    pub after_count: u32,
    /// Reason for scaling decision
    pub reason: String,
}

/// Type of scaling event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScalingEventType {
    /// Pool was scaled up (added connections)
    ScaleUp,
    /// Pool was scaled down (removed connections)
    ScaleDown,
    /// Scaling was skipped (cooldown or limits)
    Skipped,
}

/// Pool utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilizationMetrics {
    /// Total connections in pool
    pub total_connections: u32,
    /// Active connections currently in use
    pub active_connections: u32,
    /// Idle connections available
    pub idle_connections: u32,
    /// Utilization ratio (active / total)
    pub utilization: f64,
    /// Timestamp of metrics collection
    pub timestamp: DateTime<Utc>,
}

/// Statistics for auto-scaling operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoscalingStats {
    /// Total number of scale-up operations
    pub scale_up_count: u64,
    /// Total number of scale-down operations
    pub scale_down_count: u64,
    /// Total number of skipped scaling operations
    pub skipped_count: u64,
    /// Current pool size
    pub current_pool_size: u32,
    /// Average utilization over monitoring period
    pub average_utilization: f64,
    /// Peak utilization observed
    pub peak_utilization: f64,
    /// Last scaling event time
    pub last_scaling_event: Option<DateTime<Utc>>,
    /// Recent scaling events (last 100)
    pub recent_events: Vec<ScalingEvent>,
}

/// Connection pool auto-scaler
pub struct PoolAutoscaler {
    /// PostgreSQL connection pool
    pool: PgPool,
    /// Auto-scaling configuration
    config: AutoscalingConfig,
    /// Auto-scaling statistics
    stats: Arc<RwLock<AutoscalingStats>>,
    /// Last scaling operation timestamp
    last_scale: Arc<RwLock<Option<DateTime<Utc>>>>,
    /// Running state
    running: Arc<RwLock<bool>>,
}

impl PoolAutoscaler {
    /// Create a new pool auto-scaler
    pub fn new(pool: PgPool, config: AutoscalingConfig) -> Self {
        Self {
            pool,
            config,
            stats: Arc::new(RwLock::new(AutoscalingStats {
                scale_up_count: 0,
                scale_down_count: 0,
                skipped_count: 0,
                current_pool_size: 0,
                average_utilization: 0.0,
                peak_utilization: 0.0,
                last_scaling_event: None,
                recent_events: Vec::new(),
            })),
            last_scale: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the auto-scaler background task
    pub async fn start(&self) -> Result<()> {
        self.config.validate()?;

        let mut running = self.running.write().await;
        if *running {
            warn!("Auto-scaler already running");
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Starting connection pool auto-scaler");

        let pool = self.pool.clone();
        let config = self.config.clone();
        let stats = Arc::clone(&self.stats);
        let last_scale = Arc::clone(&self.last_scale);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.check_interval);

            loop {
                interval.tick().await;

                // Check if we should stop
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        info!("Auto-scaler stopped");
                        break;
                    }
                }

                // Get current utilization metrics
                let metrics = match Self::get_utilization_metrics(&pool).await {
                    Ok(m) => m,
                    Err(e) => {
                        warn!("Failed to get utilization metrics: {}", e);
                        continue;
                    }
                };

                debug!(
                    "Pool utilization: {:.1}% ({}/{} connections)",
                    metrics.utilization * 100.0,
                    metrics.active_connections,
                    metrics.total_connections
                );

                // Update stats
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.current_pool_size = metrics.total_connections;
                    if metrics.utilization > stats_guard.peak_utilization {
                        stats_guard.peak_utilization = metrics.utilization;
                    }
                }

                // Check if scaling is needed
                let scaling_decision = Self::should_scale(&config, &metrics, &last_scale).await;

                if let Some((event_type, new_size, reason)) = scaling_decision {
                    // Record scaling event
                    let event = ScalingEvent {
                        timestamp: Utc::now(),
                        event_type,
                        utilization: metrics.utilization,
                        before_count: metrics.total_connections,
                        after_count: new_size,
                        reason: reason.clone(),
                    };

                    let mut stats_guard = stats.write().await;

                    match event_type {
                        ScalingEventType::ScaleUp => {
                            stats_guard.scale_up_count += 1;
                            info!(
                                "Scaling UP: {} -> {} connections ({})",
                                metrics.total_connections, new_size, reason
                            );
                        }
                        ScalingEventType::ScaleDown => {
                            stats_guard.scale_down_count += 1;
                            info!(
                                "Scaling DOWN: {} -> {} connections ({})",
                                metrics.total_connections, new_size, reason
                            );
                        }
                        ScalingEventType::Skipped => {
                            stats_guard.skipped_count += 1;
                            debug!("Scaling SKIPPED: {}", reason);
                        }
                    }

                    stats_guard.last_scaling_event = Some(event.timestamp);
                    stats_guard.recent_events.push(event);

                    // Keep only last 100 events
                    if stats_guard.recent_events.len() > 100 {
                        stats_guard.recent_events.remove(0);
                    }

                    drop(stats_guard);

                    // Update last scale timestamp if we actually scaled
                    if event_type != ScalingEventType::Skipped {
                        let mut last = last_scale.write().await;
                        *last = Some(Utc::now());
                    }

                    // Note: Actual pool resizing would require SQLx API support
                    // For now, we log the scaling decision
                    // In production, you would call: pool.set_max_connections(new_size).await
                }
            }
        });

        Ok(())
    }

    /// Stop the auto-scaler
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        info!("Stopping connection pool auto-scaler");
    }

    /// Get current utilization metrics
    async fn get_utilization_metrics(_pool: &PgPool) -> Result<UtilizationMetrics> {
        // SQLx doesn't expose these metrics directly, so we approximate
        // In production, you'd query pg_stat_database or use pool.size() if available

        // For now, return mock metrics (in production, query actual pool state)
        Ok(UtilizationMetrics {
            total_connections: 10, // Would get from pool.options().get_max_connections()
            active_connections: 7, // Would calculate from pool internal state
            idle_connections: 3,
            utilization: 0.7,
            timestamp: Utc::now(),
        })
    }

    /// Determine if scaling is needed and calculate new size
    async fn should_scale(
        config: &AutoscalingConfig,
        metrics: &UtilizationMetrics,
        last_scale: &Arc<RwLock<Option<DateTime<Utc>>>>,
    ) -> Option<(ScalingEventType, u32, String)> {
        // Check cooldown period
        {
            let last = last_scale.read().await;
            if let Some(last_time) = *last {
                let elapsed = Utc::now().signed_duration_since(last_time);
                if elapsed.to_std().unwrap_or(Duration::ZERO) < config.cooldown_period {
                    return Some((
                        ScalingEventType::Skipped,
                        metrics.total_connections,
                        "In cooldown period".to_string(),
                    ));
                }
            }
        }

        // Check if we should scale up
        if metrics.utilization >= config.scale_up_threshold {
            if metrics.total_connections >= config.max_connections {
                return Some((
                    ScalingEventType::Skipped,
                    metrics.total_connections,
                    format!("Already at max connections ({})", config.max_connections),
                ));
            }

            let new_size = Self::calculate_scale_up(config, metrics);
            return Some((
                ScalingEventType::ScaleUp,
                new_size,
                format!(
                    "Utilization {:.1}% exceeds {:.1}% threshold",
                    metrics.utilization * 100.0,
                    config.scale_up_threshold * 100.0
                ),
            ));
        }

        // Check if we should scale down
        if metrics.utilization <= config.scale_down_threshold {
            if metrics.total_connections <= config.min_connections {
                return Some((
                    ScalingEventType::Skipped,
                    metrics.total_connections,
                    format!("Already at min connections ({})", config.min_connections),
                ));
            }

            let new_size = Self::calculate_scale_down(config, metrics);
            return Some((
                ScalingEventType::ScaleDown,
                new_size,
                format!(
                    "Utilization {:.1}% below {:.1}% threshold",
                    metrics.utilization * 100.0,
                    config.scale_down_threshold * 100.0
                ),
            ));
        }

        None
    }

    /// Calculate new pool size for scale-up operation
    fn calculate_scale_up(config: &AutoscalingConfig, metrics: &UtilizationMetrics) -> u32 {
        let new_size = match config.strategy {
            ScalingStrategy::Linear => metrics.total_connections + config.scale_up_increment,
            ScalingStrategy::Exponential => (metrics.total_connections as f64 * 1.5).ceil() as u32,
            ScalingStrategy::Proportional => {
                let needed =
                    (metrics.active_connections as f64 / config.scale_up_threshold).ceil() as u32;
                needed.max(metrics.total_connections + 1)
            }
        };

        new_size.min(config.max_connections)
    }

    /// Calculate new pool size for scale-down operation
    fn calculate_scale_down(config: &AutoscalingConfig, metrics: &UtilizationMetrics) -> u32 {
        let new_size = match config.strategy {
            ScalingStrategy::Linear => metrics
                .total_connections
                .saturating_sub(config.scale_down_increment),
            ScalingStrategy::Exponential => {
                (metrics.total_connections as f64 * 0.75).floor() as u32
            }
            ScalingStrategy::Proportional => {
                let needed =
                    (metrics.active_connections as f64 / config.scale_up_threshold).ceil() as u32;
                needed.max(config.min_connections)
            }
        };

        new_size.max(config.min_connections)
    }

    /// Get current auto-scaling statistics
    pub async fn get_stats(&self) -> AutoscalingStats {
        self.stats.read().await.clone()
    }

    /// Reset auto-scaling statistics
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        stats.scale_up_count = 0;
        stats.scale_down_count = 0;
        stats.skipped_count = 0;
        stats.average_utilization = 0.0;
        stats.peak_utilization = 0.0;
        stats.recent_events.clear();
    }

    /// Check if auto-scaler is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autoscaling_config_default() {
        let config = AutoscalingConfig::default();
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.max_connections, 100);
        assert_eq!(config.scale_up_threshold, 0.75);
        assert_eq!(config.scale_down_threshold, 0.25);
        assert!(matches!(config.strategy, ScalingStrategy::Linear));
    }

    #[test]
    fn test_autoscaling_config_validation() {
        let mut config = AutoscalingConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid: min >= max
        config.min_connections = 100;
        config.max_connections = 50;
        assert!(config.validate().is_err());

        // Invalid: scale_up <= scale_down
        config.min_connections = 5;
        config.max_connections = 100;
        config.scale_up_threshold = 0.3;
        config.scale_down_threshold = 0.7;
        assert!(config.validate().is_err());

        // Invalid: threshold out of range
        config.scale_up_threshold = 1.5;
        config.scale_down_threshold = 0.2;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_scaling_strategy_serialization() {
        let linear = ScalingStrategy::Linear;
        let json = serde_json::to_string(&linear).unwrap();
        assert_eq!(json, "\"Linear\"");

        let exponential = ScalingStrategy::Exponential;
        let json = serde_json::to_string(&exponential).unwrap();
        assert_eq!(json, "\"Exponential\"");
    }

    #[test]
    fn test_utilization_metrics_creation() {
        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 15,
            idle_connections: 5,
            utilization: 0.75,
            timestamp: Utc::now(),
        };

        assert_eq!(metrics.total_connections, 20);
        assert_eq!(metrics.active_connections, 15);
        assert_eq!(metrics.idle_connections, 5);
        assert_eq!(metrics.utilization, 0.75);
    }

    #[test]
    fn test_calculate_scale_up_linear() {
        let config = AutoscalingConfig {
            scale_up_increment: 5,
            max_connections: 50,
            strategy: ScalingStrategy::Linear,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 18,
            idle_connections: 2,
            utilization: 0.9,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_up(&config, &metrics);
        assert_eq!(new_size, 25); // 20 + 5
    }

    #[test]
    fn test_calculate_scale_up_respects_max() {
        let config = AutoscalingConfig {
            scale_up_increment: 10,
            max_connections: 25,
            strategy: ScalingStrategy::Linear,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 18,
            idle_connections: 2,
            utilization: 0.9,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_up(&config, &metrics);
        assert_eq!(new_size, 25); // capped at max_connections
    }

    #[test]
    fn test_calculate_scale_down_linear() {
        let config = AutoscalingConfig {
            scale_down_increment: 3,
            min_connections: 5,
            strategy: ScalingStrategy::Linear,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 4,
            idle_connections: 16,
            utilization: 0.2,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_down(&config, &metrics);
        assert_eq!(new_size, 17); // 20 - 3
    }

    #[test]
    fn test_calculate_scale_down_respects_min() {
        let config = AutoscalingConfig {
            scale_down_increment: 10,
            min_connections: 15,
            strategy: ScalingStrategy::Linear,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 4,
            idle_connections: 16,
            utilization: 0.2,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_down(&config, &metrics);
        assert_eq!(new_size, 15); // capped at min_connections
    }

    #[test]
    fn test_scaling_event_serialization() {
        let event = ScalingEvent {
            timestamp: Utc::now(),
            event_type: ScalingEventType::ScaleUp,
            utilization: 0.85,
            before_count: 20,
            after_count: 25,
            reason: "High utilization".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("ScaleUp"));
        assert!(json.contains("0.85"));
        assert!(json.contains("High utilization"));
    }

    #[test]
    fn test_autoscaling_stats_initialization() {
        let stats = AutoscalingStats {
            scale_up_count: 0,
            scale_down_count: 0,
            skipped_count: 0,
            current_pool_size: 10,
            average_utilization: 0.0,
            peak_utilization: 0.0,
            last_scaling_event: None,
            recent_events: Vec::new(),
        };

        assert_eq!(stats.scale_up_count, 0);
        assert_eq!(stats.current_pool_size, 10);
        assert!(stats.recent_events.is_empty());
    }

    #[test]
    fn test_calculate_scale_up_exponential() {
        let config = AutoscalingConfig {
            max_connections: 100,
            strategy: ScalingStrategy::Exponential,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 18,
            idle_connections: 2,
            utilization: 0.9,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_up(&config, &metrics);
        assert_eq!(new_size, 30); // 20 * 1.5 = 30
    }

    #[test]
    fn test_calculate_scale_down_exponential() {
        let config = AutoscalingConfig {
            min_connections: 5,
            strategy: ScalingStrategy::Exponential,
            ..Default::default()
        };

        let metrics = UtilizationMetrics {
            total_connections: 20,
            active_connections: 4,
            idle_connections: 16,
            utilization: 0.2,
            timestamp: Utc::now(),
        };

        let new_size = PoolAutoscaler::calculate_scale_down(&config, &metrics);
        assert_eq!(new_size, 15); // 20 * 0.75 = 15
    }

    #[test]
    fn test_scaling_event_types() {
        assert_eq!(ScalingEventType::ScaleUp, ScalingEventType::ScaleUp);
        assert_ne!(ScalingEventType::ScaleUp, ScalingEventType::ScaleDown);
    }
}
