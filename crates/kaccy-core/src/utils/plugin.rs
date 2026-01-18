// Plugin and extension system for custom indicators and extensibility
use crate::error::CoreError;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Plugin API version for compatibility checking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ApiVersion {
    /// Create a new API version
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Check if this version is compatible with another
    pub fn is_compatible(&self, other: &ApiVersion) -> bool {
        self.major == other.major && self.minor >= other.minor
    }
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub api_version: ApiVersion,
    pub permissions: Vec<PluginPermission>,
    pub dependencies: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub verified: bool,
}

/// Plugin permissions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginPermission {
    ReadMarketData,
    ReadUserData,
    ExecuteTrades,
    AccessPriceData,
    CreateIndicators,
    NetworkAccess,
    FileSystemAccess,
}

impl PluginMetadata {
    /// Create new plugin metadata
    pub fn new(
        name: String,
        version: String,
        author: String,
        description: String,
        api_version: ApiVersion,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            version,
            author,
            description,
            api_version,
            permissions: Vec::new(),
            dependencies: Vec::new(),
            created_at: chrono::Utc::now(),
            verified: false,
        }
    }

    /// Add a permission
    pub fn add_permission(&mut self, permission: PluginPermission) {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
    }

    /// Check if plugin has permission
    pub fn has_permission(&self, permission: &PluginPermission) -> bool {
        self.permissions.contains(permission)
    }

    /// Verify the plugin
    pub fn verify(&mut self) {
        self.verified = true;
    }
}

/// Plugin execution sandbox
#[derive(Debug, Clone)]
pub struct PluginSandbox {
    pub plugin_id: Uuid,
    pub allowed_permissions: Vec<PluginPermission>,
    pub resource_limits: ResourceLimits,
    pub execution_count: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
}

/// Resource limits for plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_memory_mb: u64,
    pub max_cpu_time_ms: u64,
    pub max_network_calls: u32,
    pub max_file_operations: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 100,
            max_cpu_time_ms: 5000,
            max_network_calls: 10,
            max_file_operations: 0,
        }
    }
}

impl PluginSandbox {
    /// Create a new sandbox
    pub fn new(plugin_id: Uuid, allowed_permissions: Vec<PluginPermission>) -> Self {
        Self {
            plugin_id,
            allowed_permissions,
            resource_limits: ResourceLimits::default(),
            execution_count: 0,
            last_execution: None,
        }
    }

    /// Check if plugin can execute with given permission
    pub fn can_execute(&self, permission: &PluginPermission) -> bool {
        self.allowed_permissions.contains(permission)
    }

    /// Record execution
    pub fn record_execution(&mut self) {
        self.execution_count += 1;
        self.last_execution = Some(chrono::Utc::now());
    }

    /// Set custom resource limits
    pub fn set_limits(&mut self, limits: ResourceLimits) {
        self.resource_limits = limits;
    }
}

/// Plugin registry for discovery and management
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    plugins: HashMap<Uuid, PluginMetadata>,
    current_api_version: ApiVersion,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new(api_version: ApiVersion) -> Self {
        Self {
            plugins: HashMap::new(),
            current_api_version: api_version,
        }
    }

    /// Register a plugin
    pub fn register(&mut self, metadata: PluginMetadata) -> Result<(), CoreError> {
        // Check API compatibility
        if !self
            .current_api_version
            .is_compatible(&metadata.api_version)
        {
            return Err(CoreError::Validation(format!(
                "Plugin API version {:?} is not compatible with current version {:?}",
                metadata.api_version, self.current_api_version
            )));
        }

        // Check dependencies
        for dep in &metadata.dependencies {
            if !self.plugins.values().any(|p| p.name == *dep) {
                return Err(CoreError::Validation(format!(
                    "Missing dependency: {}",
                    dep
                )));
            }
        }

        self.plugins.insert(metadata.id, metadata);
        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister(&mut self, plugin_id: &Uuid) -> Result<(), CoreError> {
        // Check if any other plugins depend on this one
        let plugin_name = self
            .plugins
            .get(plugin_id)
            .map(|p| p.name.clone())
            .ok_or_else(|| CoreError::NotFound("Plugin not found".to_string()))?;

        for plugin in self.plugins.values() {
            if plugin.dependencies.contains(&plugin_name) {
                return Err(CoreError::Validation(format!(
                    "Cannot unregister plugin: {} depends on it",
                    plugin.name
                )));
            }
        }

        self.plugins.remove(plugin_id);
        Ok(())
    }

    /// Get plugin by ID
    pub fn get(&self, plugin_id: &Uuid) -> Option<&PluginMetadata> {
        self.plugins.get(plugin_id)
    }

    /// List all plugins
    pub fn list_all(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().collect()
    }

    /// List verified plugins only
    pub fn list_verified(&self) -> Vec<&PluginMetadata> {
        self.plugins.values().filter(|p| p.verified).collect()
    }
}

/// Plugin marketplace listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceListing {
    pub id: Uuid,
    pub plugin_metadata: PluginMetadata,
    pub price: Decimal,
    pub currency: String,
    pub downloads: u64,
    pub rating: Decimal,
    pub reviews: u32,
    pub status: PluginListingStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginListingStatus {
    Active,
    Pending,
    Suspended,
    Removed,
}

impl MarketplaceListing {
    /// Create a new marketplace listing
    pub fn new(plugin_metadata: PluginMetadata, price: Decimal, currency: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            plugin_metadata,
            price,
            currency,
            downloads: 0,
            rating: Decimal::ZERO,
            reviews: 0,
            status: PluginListingStatus::Pending,
            created_at: chrono::Utc::now(),
        }
    }

    /// Increment download count
    pub fn record_download(&mut self) {
        self.downloads += 1;
    }

    /// Add a review
    pub fn add_review(&mut self, rating: Decimal) {
        let total_rating = self.rating * Decimal::from(self.reviews);
        self.reviews += 1;
        self.rating = (total_rating + rating) / Decimal::from(self.reviews);
    }

    /// Approve listing
    pub fn approve(&mut self) {
        self.status = PluginListingStatus::Active;
    }

    /// Suspend listing
    pub fn suspend(&mut self) {
        self.status = PluginListingStatus::Suspended;
    }
}

/// Custom technical indicator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomIndicator {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub creator_id: Uuid,
    pub formula: IndicatorFormula,
    pub parameters: HashMap<String, IndicatorParameter>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Indicator formula definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndicatorFormula {
    Simple {
        expression: String,
    },
    MovingAverage {
        period: u32,
        weight_type: WeightType,
    },
    Composite {
        base_indicators: Vec<Uuid>,
        combination: String,
    },
    Custom {
        code: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WeightType {
    Equal,
    Exponential,
    Weighted,
}

/// Indicator parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorParameter {
    pub name: String,
    pub param_type: ParameterType,
    pub default_value: String,
    pub min_value: Option<Decimal>,
    pub max_value: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParameterType {
    Integer,
    Decimal,
    String,
    Boolean,
}

impl CustomIndicator {
    /// Create a new custom indicator
    pub fn new(
        name: String,
        description: String,
        creator_id: Uuid,
        formula: IndicatorFormula,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            creator_id,
            formula,
            parameters: HashMap::new(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Add a parameter
    pub fn add_parameter(&mut self, parameter: IndicatorParameter) {
        self.parameters.insert(parameter.name.clone(), parameter);
    }

    /// Calculate indicator value (simplified)
    pub fn calculate(&self, price_data: &[Decimal]) -> Result<Vec<Decimal>, CoreError> {
        if price_data.is_empty() {
            return Ok(vec![]);
        }

        match &self.formula {
            IndicatorFormula::Simple { expression: _ } => {
                // In a real implementation, would parse and evaluate expression
                Ok(price_data.to_vec())
            }
            IndicatorFormula::MovingAverage {
                period,
                weight_type,
            } => self.calculate_moving_average(price_data, *period, weight_type),
            IndicatorFormula::Composite {
                base_indicators: _,
                combination: _,
            } => {
                // Would combine multiple indicators
                Ok(price_data.to_vec())
            }
            IndicatorFormula::Custom { code: _ } => {
                // Would execute custom code in sandbox
                Ok(price_data.to_vec())
            }
        }
    }

    fn calculate_moving_average(
        &self,
        price_data: &[Decimal],
        period: u32,
        weight_type: &WeightType,
    ) -> Result<Vec<Decimal>, CoreError> {
        if price_data.len() < period as usize {
            return Err(CoreError::Validation(
                "Insufficient data for moving average".to_string(),
            ));
        }

        let mut result = Vec::new();

        match weight_type {
            WeightType::Equal => {
                // Simple Moving Average
                for i in (period as usize - 1)..price_data.len() {
                    let sum: Decimal = price_data[i - (period as usize - 1)..=i].iter().sum();
                    result.push(sum / Decimal::from(period));
                }
            }
            WeightType::Exponential => {
                // Exponential Moving Average
                let multiplier = Decimal::from(2) / (Decimal::from(period) + Decimal::ONE);
                let mut ema = price_data[0];
                result.push(ema);

                for price in price_data.iter().skip(1) {
                    ema = (price - ema) * multiplier + ema;
                    result.push(ema);
                }
            }
            WeightType::Weighted => {
                // Weighted Moving Average
                for i in (period as usize - 1)..price_data.len() {
                    let mut weighted_sum = Decimal::ZERO;
                    let mut weight_sum = Decimal::ZERO;

                    for (j, price) in price_data[i - (period as usize - 1)..=i].iter().enumerate() {
                        let weight = Decimal::from(j + 1);
                        weighted_sum += price * weight;
                        weight_sum += weight;
                    }

                    result.push(weighted_sum / weight_sum);
                }
            }
        }

        Ok(result)
    }
}

/// Indicator backtesting framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorBacktest {
    pub indicator_id: Uuid,
    pub start_date: chrono::DateTime<chrono::Utc>,
    pub end_date: chrono::DateTime<chrono::Utc>,
    pub results: BacktestResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResults {
    pub total_signals: u32,
    pub profitable_signals: u32,
    pub win_rate: Decimal,
    pub avg_profit: Decimal,
    pub avg_loss: Decimal,
    pub max_drawdown: Decimal,
    pub sharpe_ratio: Decimal,
}

impl BacktestResults {
    /// Create new backtest results
    pub fn new() -> Self {
        Self {
            total_signals: 0,
            profitable_signals: 0,
            win_rate: Decimal::ZERO,
            avg_profit: Decimal::ZERO,
            avg_loss: Decimal::ZERO,
            max_drawdown: Decimal::ZERO,
            sharpe_ratio: Decimal::ZERO,
        }
    }

    /// Calculate metrics
    pub fn calculate_metrics(&mut self, signals: &[SignalResult]) {
        self.total_signals = signals.len() as u32;
        self.profitable_signals =
            signals.iter().filter(|s| s.profit > Decimal::ZERO).count() as u32;

        if self.total_signals > 0 {
            self.win_rate =
                Decimal::from(self.profitable_signals) / Decimal::from(self.total_signals);
        }

        let profits: Vec<_> = signals
            .iter()
            .filter(|s| s.profit > Decimal::ZERO)
            .collect();
        let losses: Vec<_> = signals
            .iter()
            .filter(|s| s.profit < Decimal::ZERO)
            .collect();

        if !profits.is_empty() {
            self.avg_profit =
                profits.iter().map(|s| s.profit).sum::<Decimal>() / Decimal::from(profits.len());
        }

        if !losses.is_empty() {
            self.avg_loss =
                losses.iter().map(|s| s.profit).sum::<Decimal>() / Decimal::from(losses.len());
        }
    }
}

impl Default for BacktestResults {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResult {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub profit: Decimal,
}

/// Indicator composition for combining multiple indicators
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorComposition {
    pub id: Uuid,
    pub name: String,
    pub base_indicators: Vec<Uuid>,
    pub composition_logic: CompositionLogic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompositionLogic {
    And,                                // All indicators must agree
    Or,                                 // Any indicator triggers
    Weighted { weights: Vec<Decimal> }, // Weighted combination
    Custom { expression: String },
}

impl IndicatorComposition {
    /// Create a new composition
    pub fn new(name: String, base_indicators: Vec<Uuid>, logic: CompositionLogic) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            base_indicators,
            composition_logic: logic,
        }
    }

    /// Evaluate composition
    pub fn evaluate(&self, signals: &HashMap<Uuid, bool>) -> bool {
        match &self.composition_logic {
            CompositionLogic::And => self
                .base_indicators
                .iter()
                .all(|id| signals.get(id).copied().unwrap_or(false)),
            CompositionLogic::Or => self
                .base_indicators
                .iter()
                .any(|id| signals.get(id).copied().unwrap_or(false)),
            CompositionLogic::Weighted { weights } => {
                let mut weighted_sum = Decimal::ZERO;
                let mut total_weight = Decimal::ZERO;

                for (indicator_id, weight) in self.base_indicators.iter().zip(weights.iter()) {
                    if signals.get(indicator_id).copied().unwrap_or(false) {
                        weighted_sum += weight;
                    }
                    total_weight += weight;
                }

                weighted_sum / total_weight > Decimal::new(5, 1) // > 0.5
            }
            CompositionLogic::Custom { expression: _ } => {
                // Would evaluate custom expression
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version_compatibility() {
        let v1 = ApiVersion::new(1, 0, 0);
        let v2 = ApiVersion::new(1, 1, 0);
        let v3 = ApiVersion::new(2, 0, 0);

        assert!(v2.is_compatible(&v1));
        assert!(!v1.is_compatible(&v2));
        assert!(!v3.is_compatible(&v1));
    }

    #[test]
    fn test_plugin_metadata() {
        let mut metadata = PluginMetadata::new(
            "Test Plugin".to_string(),
            "1.0.0".to_string(),
            "Author".to_string(),
            "Description".to_string(),
            ApiVersion::new(1, 0, 0),
        );

        metadata.add_permission(PluginPermission::ReadMarketData);
        assert!(metadata.has_permission(&PluginPermission::ReadMarketData));
        assert!(!metadata.has_permission(&PluginPermission::ExecuteTrades));
    }

    #[test]
    fn test_plugin_sandbox() {
        let mut sandbox =
            PluginSandbox::new(Uuid::new_v4(), vec![PluginPermission::ReadMarketData]);

        assert!(sandbox.can_execute(&PluginPermission::ReadMarketData));
        assert!(!sandbox.can_execute(&PluginPermission::ExecuteTrades));

        sandbox.record_execution();
        assert_eq!(sandbox.execution_count, 1);
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new(ApiVersion::new(1, 0, 0));

        let metadata = PluginMetadata::new(
            "Test Plugin".to_string(),
            "1.0.0".to_string(),
            "Author".to_string(),
            "Description".to_string(),
            ApiVersion::new(1, 0, 0),
        );

        let plugin_id = metadata.id;
        assert!(registry.register(metadata).is_ok());
        assert!(registry.get(&plugin_id).is_some());
    }

    #[test]
    fn test_marketplace_listing() {
        let metadata = PluginMetadata::new(
            "Test Plugin".to_string(),
            "1.0.0".to_string(),
            "Author".to_string(),
            "Description".to_string(),
            ApiVersion::new(1, 0, 0),
        );

        let mut listing = MarketplaceListing::new(metadata, Decimal::from(10), "BTC".to_string());

        listing.record_download();
        assert_eq!(listing.downloads, 1);

        listing.add_review(Decimal::from(5));
        assert_eq!(listing.reviews, 1);
        assert_eq!(listing.rating, Decimal::from(5));
    }

    #[test]
    fn test_custom_indicator_sma() {
        let indicator = CustomIndicator::new(
            "SMA".to_string(),
            "Simple Moving Average".to_string(),
            Uuid::new_v4(),
            IndicatorFormula::MovingAverage {
                period: 3,
                weight_type: WeightType::Equal,
            },
        );

        let prices = vec![
            Decimal::from(10),
            Decimal::from(20),
            Decimal::from(30),
            Decimal::from(40),
        ];

        let result = indicator.calculate(&prices).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Decimal::from(20)); // (10+20+30)/3
        assert_eq!(result[1], Decimal::from(30)); // (20+30+40)/3
    }

    #[test]
    fn test_indicator_composition_and() {
        let comp = IndicatorComposition::new(
            "Test".to_string(),
            vec![Uuid::new_v4(), Uuid::new_v4()],
            CompositionLogic::And,
        );

        let mut signals = HashMap::new();
        signals.insert(comp.base_indicators[0], true);
        signals.insert(comp.base_indicators[1], true);

        assert!(comp.evaluate(&signals));

        signals.insert(comp.base_indicators[1], false);
        assert!(!comp.evaluate(&signals));
    }

    #[test]
    fn test_backtest_results() {
        let mut results = BacktestResults::new();

        let signals = vec![
            SignalResult {
                timestamp: chrono::Utc::now(),
                profit: Decimal::from(10),
            },
            SignalResult {
                timestamp: chrono::Utc::now(),
                profit: Decimal::from(-5),
            },
            SignalResult {
                timestamp: chrono::Utc::now(),
                profit: Decimal::from(15),
            },
        ];

        results.calculate_metrics(&signals);
        assert_eq!(results.total_signals, 3);
        assert_eq!(results.profitable_signals, 2);
    }
}
