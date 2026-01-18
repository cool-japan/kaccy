// Advanced analytics and reporting for custom dashboards and financial reports
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Custom dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: String,
    pub layout: DashboardLayout,
    pub widgets: Vec<Widget>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Dashboard layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    pub grid_columns: u32,
    pub grid_rows: u32,
    pub responsive: bool,
}

impl Dashboard {
    /// Create a new dashboard
    pub fn new(user_id: Uuid, name: String, description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            name,
            description,
            layout: DashboardLayout {
                grid_columns: 12,
                grid_rows: 6,
                responsive: true,
            },
            widgets: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Add a widget to the dashboard
    pub fn add_widget(&mut self, widget: Widget) {
        self.widgets.push(widget);
        self.updated_at = chrono::Utc::now();
    }

    /// Remove a widget by ID
    pub fn remove_widget(&mut self, widget_id: &Uuid) -> bool {
        if let Some(pos) = self.widgets.iter().position(|w| &w.id == widget_id) {
            self.widgets.remove(pos);
            self.updated_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }

    /// Update widget position
    pub fn update_widget_position(&mut self, widget_id: &Uuid, position: GridPosition) -> bool {
        if let Some(widget) = self.widgets.iter_mut().find(|w| &w.id == widget_id) {
            widget.position = position;
            self.updated_at = chrono::Utc::now();
            true
        } else {
            false
        }
    }
}

/// Widget for displaying metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    pub id: Uuid,
    pub widget_type: WidgetType,
    pub position: GridPosition,
    pub size: WidgetSize,
    pub config: WidgetConfig,
    pub refresh_interval_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSize {
    pub width: u32,  // in grid units
    pub height: u32, // in grid units
}

/// Widget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    pub title: String,
    pub data_source: String,
    pub parameters: HashMap<String, String>,
    pub visualization: VisualizationType,
}

/// Types of widgets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetType {
    PriceChart,
    VolumeChart,
    PortfolioSummary,
    RecentTrades,
    OrderBook,
    PerformanceMetrics,
    RiskMetrics,
    MarketDepth,
    CustomMetric,
}

/// Visualization types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizationType {
    LineChart,
    BarChart,
    PieChart,
    Candlestick,
    HeatMap,
    Table,
    Number,
    Gauge,
}

impl Widget {
    /// Create a new widget
    pub fn new(
        widget_type: WidgetType,
        position: GridPosition,
        size: WidgetSize,
        title: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            widget_type,
            position,
            size,
            config: WidgetConfig {
                title,
                data_source: String::new(),
                parameters: HashMap::new(),
                visualization: VisualizationType::LineChart,
            },
            refresh_interval_seconds: 60,
        }
    }

    /// Set data source
    pub fn set_data_source(&mut self, source: String) {
        self.config.data_source = source;
    }

    /// Add parameter
    pub fn add_parameter(&mut self, key: String, value: String) {
        self.config.parameters.insert(key, value);
    }
}

/// Tax report generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxReport {
    pub id: Uuid,
    pub user_id: Uuid,
    pub tax_year: u32,
    pub report_type: TaxReportType,
    pub capital_gains: CapitalGainsReport,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxReportType {
    ShortTerm,
    LongTerm,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalGainsReport {
    pub short_term_gains: Decimal,
    pub long_term_gains: Decimal,
    pub total_gains: Decimal,
    pub total_losses: Decimal,
    pub net_gains: Decimal,
    pub transactions: Vec<TaxTransaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxTransaction {
    pub token_symbol: String,
    pub buy_date: chrono::DateTime<chrono::Utc>,
    pub sell_date: chrono::DateTime<chrono::Utc>,
    pub quantity: Decimal,
    pub cost_basis: Decimal,
    pub proceeds: Decimal,
    pub gain_loss: Decimal,
    pub holding_period_days: i64,
}

impl TaxReport {
    /// Create a new tax report
    pub fn new(user_id: Uuid, tax_year: u32, report_type: TaxReportType) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            tax_year,
            report_type,
            capital_gains: CapitalGainsReport {
                short_term_gains: Decimal::ZERO,
                long_term_gains: Decimal::ZERO,
                total_gains: Decimal::ZERO,
                total_losses: Decimal::ZERO,
                net_gains: Decimal::ZERO,
                transactions: Vec::new(),
            },
            generated_at: chrono::Utc::now(),
        }
    }

    /// Add a transaction to the report
    pub fn add_transaction(&mut self, transaction: TaxTransaction) {
        let is_short_term = transaction.holding_period_days <= 365;

        if transaction.gain_loss > Decimal::ZERO {
            self.capital_gains.total_gains += transaction.gain_loss;
            if is_short_term {
                self.capital_gains.short_term_gains += transaction.gain_loss;
            } else {
                self.capital_gains.long_term_gains += transaction.gain_loss;
            }
        } else {
            self.capital_gains.total_losses += transaction.gain_loss.abs();
        }

        self.capital_gains.transactions.push(transaction);
        self.calculate_net_gains();
    }

    fn calculate_net_gains(&mut self) {
        self.capital_gains.net_gains =
            self.capital_gains.total_gains - self.capital_gains.total_losses;
    }
}

/// Regulatory report generator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegulatoryReport {
    pub id: Uuid,
    pub report_type: RegulatoryReportType,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    pub jurisdiction: String,
    pub data: HashMap<String, String>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryReportType {
    AML,  // Anti-Money Laundering
    KYC,  // Know Your Customer
    SAR,  // Suspicious Activity Report
    CTR,  // Currency Transaction Report
    FBAR, // Foreign Bank Account Report
    Custom(String),
}

impl RegulatoryReport {
    /// Create a new regulatory report
    pub fn new(
        report_type: RegulatoryReportType,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
        jurisdiction: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            report_type,
            period_start,
            period_end,
            jurisdiction,
            data: HashMap::new(),
            generated_at: chrono::Utc::now(),
        }
    }

    /// Add data field
    pub fn add_field(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }
}

/// Performance attribution report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceAttribution {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    pub total_return: Decimal,
    pub attributions: Vec<Attribution>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    pub category: String,
    pub contribution: Decimal,
    pub percentage: Decimal,
}

impl PerformanceAttribution {
    /// Create a new performance attribution report
    pub fn new(
        portfolio_id: Uuid,
        period_start: chrono::DateTime<chrono::Utc>,
        period_end: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            portfolio_id,
            period_start,
            period_end,
            total_return: Decimal::ZERO,
            attributions: Vec::new(),
            generated_at: chrono::Utc::now(),
        }
    }

    /// Add an attribution
    pub fn add_attribution(&mut self, category: String, contribution: Decimal) {
        let percentage = if self.total_return != Decimal::ZERO {
            (contribution / self.total_return) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        self.attributions.push(Attribution {
            category,
            contribution,
            percentage,
        });
    }

    /// Set total return
    pub fn set_total_return(&mut self, total_return: Decimal) {
        self.total_return = total_return;

        // Recalculate percentages
        for attribution in &mut self.attributions {
            attribution.percentage = if total_return != Decimal::ZERO {
                (attribution.contribution / total_return) * Decimal::from(100)
            } else {
                Decimal::ZERO
            };
        }
    }
}

/// Risk report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub as_of_date: chrono::DateTime<chrono::Utc>,
    pub value_at_risk: VaRMetrics,
    pub stress_test_results: Vec<RiskStressTestResult>,
    pub concentration_risk: HashMap<String, Decimal>,
    pub liquidity_risk: LiquidityRiskMetrics,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaRMetrics {
    pub var_95: Decimal,
    pub var_99: Decimal,
    pub expected_shortfall: Decimal,
    pub confidence_level: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStressTestResult {
    pub scenario: String,
    pub estimated_loss: Decimal,
    pub loss_percentage: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityRiskMetrics {
    pub liquidity_coverage_ratio: Decimal,
    pub days_to_liquidate: u32,
    pub illiquid_percentage: Decimal,
}

impl RiskReport {
    /// Create a new risk report
    pub fn new(portfolio_id: Uuid, as_of_date: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            portfolio_id,
            as_of_date,
            value_at_risk: VaRMetrics {
                var_95: Decimal::ZERO,
                var_99: Decimal::ZERO,
                expected_shortfall: Decimal::ZERO,
                confidence_level: Decimal::new(95, 2),
            },
            stress_test_results: Vec::new(),
            concentration_risk: HashMap::new(),
            liquidity_risk: LiquidityRiskMetrics {
                liquidity_coverage_ratio: Decimal::ZERO,
                days_to_liquidate: 0,
                illiquid_percentage: Decimal::ZERO,
            },
            generated_at: chrono::Utc::now(),
        }
    }

    /// Add stress test result
    pub fn add_stress_test(
        &mut self,
        scenario: String,
        estimated_loss: Decimal,
        total_value: Decimal,
    ) {
        let loss_percentage = if total_value != Decimal::ZERO {
            (estimated_loss / total_value) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        self.stress_test_results.push(RiskStressTestResult {
            scenario,
            estimated_loss,
            loss_percentage,
        });
    }

    /// Set concentration risk for a category
    pub fn set_concentration(&mut self, category: String, percentage: Decimal) {
        self.concentration_risk.insert(category, percentage);
    }
}

/// Real-time data stream for widgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStream {
    pub id: Uuid,
    pub source: String,
    pub data_type: DataType,
    pub update_frequency_ms: u64,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Price,
    Volume,
    OrderBook,
    Trades,
    Metrics,
}

impl DataStream {
    /// Create a new data stream
    pub fn new(source: String, data_type: DataType, update_frequency_ms: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            data_type,
            update_frequency_ms,
            last_update: None,
        }
    }

    /// Record update
    pub fn record_update(&mut self) {
        self.last_update = Some(chrono::Utc::now());
    }

    /// Check if update is needed
    pub fn needs_update(&self) -> bool {
        match self.last_update {
            None => true,
            Some(last) => {
                let elapsed = chrono::Utc::now()
                    .signed_duration_since(last)
                    .num_milliseconds();
                elapsed >= self.update_frequency_ms as i64
            }
        }
    }
}

/// Report scheduler for automated report generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSchedule {
    pub id: Uuid,
    pub user_id: Uuid,
    pub report_name: String,
    pub schedule: ScheduleFrequency,
    pub recipients: Vec<String>,
    pub enabled: bool,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub next_run: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleFrequency {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl ReportSchedule {
    /// Create a new report schedule
    pub fn new(user_id: Uuid, report_name: String, schedule: ScheduleFrequency) -> Self {
        let next_run = Self::calculate_next_run(&schedule, chrono::Utc::now());

        Self {
            id: Uuid::new_v4(),
            user_id,
            report_name,
            schedule,
            recipients: Vec::new(),
            enabled: true,
            last_run: None,
            next_run,
        }
    }

    /// Add a recipient
    pub fn add_recipient(&mut self, email: String) {
        if !self.recipients.contains(&email) {
            self.recipients.push(email);
        }
    }

    /// Record execution
    pub fn record_execution(&mut self) {
        self.last_run = Some(chrono::Utc::now());
        self.next_run = Self::calculate_next_run(&self.schedule, chrono::Utc::now());
    }

    fn calculate_next_run(
        frequency: &ScheduleFrequency,
        from: chrono::DateTime<chrono::Utc>,
    ) -> chrono::DateTime<chrono::Utc> {
        match frequency {
            ScheduleFrequency::Daily => from + chrono::Duration::days(1),
            ScheduleFrequency::Weekly => from + chrono::Duration::weeks(1),
            ScheduleFrequency::Monthly => from + chrono::Duration::days(30),
            ScheduleFrequency::Quarterly => from + chrono::Duration::days(90),
            ScheduleFrequency::Yearly => from + chrono::Duration::days(365),
        }
    }

    /// Check if report should run
    pub fn should_run(&self) -> bool {
        self.enabled && chrono::Utc::now() >= self.next_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        let dashboard = Dashboard::new(
            Uuid::new_v4(),
            "My Dashboard".to_string(),
            "Test dashboard".to_string(),
        );

        assert_eq!(dashboard.name, "My Dashboard");
        assert_eq!(dashboard.widgets.len(), 0);
    }

    #[test]
    fn test_dashboard_add_widget() {
        let mut dashboard = Dashboard::new(Uuid::new_v4(), "Test".to_string(), "Test".to_string());

        let widget = Widget::new(
            WidgetType::PriceChart,
            GridPosition { row: 0, col: 0 },
            WidgetSize {
                width: 6,
                height: 4,
            },
            "Price Chart".to_string(),
        );

        dashboard.add_widget(widget);
        assert_eq!(dashboard.widgets.len(), 1);
    }

    #[test]
    fn test_tax_report() {
        let mut report = TaxReport::new(Uuid::new_v4(), 2026, TaxReportType::Combined);

        let transaction = TaxTransaction {
            token_symbol: "BTC".to_string(),
            buy_date: chrono::Utc::now() - chrono::Duration::days(400),
            sell_date: chrono::Utc::now(),
            quantity: Decimal::from(1),
            cost_basis: Decimal::from(30000),
            proceeds: Decimal::from(50000),
            gain_loss: Decimal::from(20000),
            holding_period_days: 400,
        };

        report.add_transaction(transaction);
        assert_eq!(report.capital_gains.long_term_gains, Decimal::from(20000));
        assert_eq!(report.capital_gains.net_gains, Decimal::from(20000));
    }

    #[test]
    fn test_regulatory_report() {
        let report = RegulatoryReport::new(
            RegulatoryReportType::AML,
            chrono::Utc::now() - chrono::Duration::days(30),
            chrono::Utc::now(),
            "US".to_string(),
        );

        assert_eq!(report.jurisdiction, "US");
        assert_eq!(report.report_type, RegulatoryReportType::AML);
    }

    #[test]
    fn test_performance_attribution() {
        let mut report = PerformanceAttribution::new(
            Uuid::new_v4(),
            chrono::Utc::now() - chrono::Duration::days(30),
            chrono::Utc::now(),
        );

        report.set_total_return(Decimal::from(100));
        report.add_attribution("Stocks".to_string(), Decimal::from(60));
        report.add_attribution("Crypto".to_string(), Decimal::from(40));

        assert_eq!(report.attributions.len(), 2);
        assert_eq!(report.attributions[0].percentage, Decimal::from(60));
    }

    #[test]
    fn test_risk_report() {
        let mut report = RiskReport::new(Uuid::new_v4(), chrono::Utc::now());

        report.add_stress_test(
            "Market Crash".to_string(),
            Decimal::from(10000),
            Decimal::from(100000),
        );

        assert_eq!(report.stress_test_results.len(), 1);
        assert_eq!(
            report.stress_test_results[0].loss_percentage,
            Decimal::from(10)
        );
    }

    #[test]
    fn test_data_stream() {
        let mut stream = DataStream::new("BTC/USD".to_string(), DataType::Price, 1000);

        assert!(stream.needs_update());

        stream.record_update();
        assert!(stream.last_update.is_some());
    }

    #[test]
    fn test_report_schedule() {
        let mut schedule = ReportSchedule::new(
            Uuid::new_v4(),
            "Monthly Report".to_string(),
            ScheduleFrequency::Monthly,
        );

        schedule.add_recipient("test@example.com".to_string());
        assert_eq!(schedule.recipients.len(), 1);

        assert!(schedule.enabled);
        assert!(schedule.next_run > chrono::Utc::now());
    }
}
