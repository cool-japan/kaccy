//! Advanced analytics and reporting for custom dashboards and financial reports
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Custom dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    /// Unique identifier of the dashboard
    pub id: Uuid,
    /// User who owns this dashboard
    pub user_id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Optional description of the dashboard's purpose
    pub description: String,
    /// Layout settings
    pub layout: DashboardLayout,
    /// Widgets displayed on this dashboard
    pub widgets: Vec<Widget>,
    /// Timestamp when the dashboard was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp of the most recent modification
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Dashboard layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    /// Number of columns in the grid
    pub grid_columns: u32,
    /// Number of rows in the grid
    pub grid_rows: u32,
    /// Whether the layout should adapt to screen size
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
    /// Unique identifier of the widget
    pub id: Uuid,
    /// Category of widget
    pub widget_type: WidgetType,
    /// Position of the widget in the dashboard grid
    pub position: GridPosition,
    /// Dimensions of the widget in grid units
    pub size: WidgetSize,
    /// Widget display and data configuration
    pub config: WidgetConfig,
    /// How often the widget should refresh its data, in seconds
    pub refresh_interval_seconds: u32,
}

/// Grid position for widget placement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPosition {
    /// Row index in the grid
    pub row: u32,
    /// Column index in the grid
    pub col: u32,
}

/// Widget dimensions in grid units
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSize {
    /// Width in grid units
    pub width: u32,
    /// Height in grid units
    pub height: u32,
}

/// Widget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetConfig {
    /// Display title of the widget
    pub title: String,
    /// Data source identifier
    pub data_source: String,
    /// Key-value parameters for the data source
    pub parameters: HashMap<String, String>,
    /// How the data should be visualised
    pub visualization: VisualizationType,
}

/// Types of widgets
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WidgetType {
    /// Token price history chart
    PriceChart,
    /// Trading volume chart
    VolumeChart,
    /// Portfolio value summary
    PortfolioSummary,
    /// List of recent trades
    RecentTrades,
    /// Order book depth display
    OrderBook,
    /// Key performance metrics
    PerformanceMetrics,
    /// Risk metrics display
    RiskMetrics,
    /// Market depth visualisation
    MarketDepth,
    /// User-defined custom metric
    CustomMetric,
}

/// Visualization types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizationType {
    /// Line chart
    LineChart,
    /// Bar chart
    BarChart,
    /// Pie or donut chart
    PieChart,
    /// OHLCV candlestick chart
    Candlestick,
    /// Heat map
    HeatMap,
    /// Tabular data
    Table,
    /// Single numeric value
    Number,
    /// Gauge/dial indicator
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
    /// Unique identifier of this report
    pub id: Uuid,
    /// User this report belongs to
    pub user_id: Uuid,
    /// Tax year covered by this report
    pub tax_year: u32,
    /// Classification of gains to include
    pub report_type: TaxReportType,
    /// Capital gains summary
    pub capital_gains: CapitalGainsReport,
    /// Timestamp when the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Tax report classification by holding period
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxReportType {
    /// Short-term capital gains only (held <= 365 days)
    ShortTerm,
    /// Long-term capital gains only (held > 365 days)
    LongTerm,
    /// Combined short and long-term report
    Combined,
}

/// Capital gains summary for tax reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapitalGainsReport {
    /// Total short-term capital gains
    pub short_term_gains: Decimal,
    /// Total long-term capital gains
    pub long_term_gains: Decimal,
    /// Total gains before losses
    pub total_gains: Decimal,
    /// Total capital losses
    pub total_losses: Decimal,
    /// Net gains after offsetting losses
    pub net_gains: Decimal,
    /// Individual transactions included in this report
    pub transactions: Vec<TaxTransaction>,
}

/// Individual taxable transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxTransaction {
    /// Token ticker symbol
    pub token_symbol: String,
    /// Date the asset was purchased
    pub buy_date: chrono::DateTime<chrono::Utc>,
    /// Date the asset was sold
    pub sell_date: chrono::DateTime<chrono::Utc>,
    /// Quantity of tokens transacted
    pub quantity: Decimal,
    /// Original purchase cost
    pub cost_basis: Decimal,
    /// Sale proceeds
    pub proceeds: Decimal,
    /// Net gain or loss
    pub gain_loss: Decimal,
    /// Number of days the asset was held
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
    /// Unique identifier of this report
    pub id: Uuid,
    /// Type of regulatory filing
    pub report_type: RegulatoryReportType,
    /// Start of the reporting period
    pub period_start: chrono::DateTime<chrono::Utc>,
    /// End of the reporting period
    pub period_end: chrono::DateTime<chrono::Utc>,
    /// Regulatory jurisdiction (e.g. "US", "EU")
    pub jurisdiction: String,
    /// Arbitrary key-value data fields for the report
    pub data: HashMap<String, String>,
    /// Timestamp when the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Type of regulatory report
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryReportType {
    /// Anti-Money Laundering report
    AML,
    /// Know Your Customer report
    KYC,
    /// Suspicious Activity Report
    SAR,
    /// Currency Transaction Report
    CTR,
    /// Foreign Bank Account Report
    FBAR,
    /// Custom regulatory report with a specific name
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
    /// Unique identifier of this attribution report
    pub id: Uuid,
    /// Portfolio being analysed
    pub portfolio_id: Uuid,
    /// Start of the attribution period
    pub period_start: chrono::DateTime<chrono::Utc>,
    /// End of the attribution period
    pub period_end: chrono::DateTime<chrono::Utc>,
    /// Total portfolio return over the period
    pub total_return: Decimal,
    /// Per-category attribution breakdown
    pub attributions: Vec<Attribution>,
    /// Timestamp when the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Single attribution entry for performance decomposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    /// Category or factor name
    pub category: String,
    /// Absolute contribution to total return
    pub contribution: Decimal,
    /// Percentage contribution to total return
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
    /// Unique identifier of this risk report
    pub id: Uuid,
    /// Portfolio being assessed
    pub portfolio_id: Uuid,
    /// Date and time as of which risk metrics were computed
    pub as_of_date: chrono::DateTime<chrono::Utc>,
    /// Value-at-Risk metrics
    pub value_at_risk: VaRMetrics,
    /// Results of individual stress-test scenarios
    pub stress_test_results: Vec<RiskStressTestResult>,
    /// Concentration risk percentages keyed by category
    pub concentration_risk: HashMap<String, Decimal>,
    /// Liquidity risk metrics
    pub liquidity_risk: LiquidityRiskMetrics,
    /// Timestamp when the report was generated
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// Value-at-Risk metrics for a portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaRMetrics {
    /// Value-at-Risk at 95% confidence level
    pub var_95: Decimal,
    /// Value-at-Risk at 99% confidence level
    pub var_99: Decimal,
    /// Expected Shortfall (Conditional VaR)
    pub expected_shortfall: Decimal,
    /// Confidence level used for VaR computation
    pub confidence_level: Decimal,
}

/// Result of a single stress test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskStressTestResult {
    /// Name of the stress scenario
    pub scenario: String,
    /// Estimated monetary loss under this scenario
    pub estimated_loss: Decimal,
    /// Loss expressed as a percentage of portfolio value
    pub loss_percentage: Decimal,
}

/// Liquidity risk metrics for a portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityRiskMetrics {
    /// Ratio of liquid assets to near-term obligations
    pub liquidity_coverage_ratio: Decimal,
    /// Estimated days required to fully liquidate the portfolio
    pub days_to_liquidate: u32,
    /// Percentage of portfolio considered illiquid
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
    /// Unique identifier of this stream
    pub id: Uuid,
    /// Data source identifier (e.g. trading pair symbol)
    pub source: String,
    /// Category of data carried by this stream
    pub data_type: DataType,
    /// Target update interval in milliseconds
    pub update_frequency_ms: u64,
    /// Timestamp of the most recent data update
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
}

/// Type of data carried by a stream
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    /// Token price data
    Price,
    /// Trading volume data
    Volume,
    /// Order book depth data
    OrderBook,
    /// Individual trade data
    Trades,
    /// Aggregate performance metrics
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
    /// Unique identifier of this schedule
    pub id: Uuid,
    /// Owner of the scheduled report
    pub user_id: Uuid,
    /// Human-readable name of the report
    pub report_name: String,
    /// How often the report should be generated
    pub schedule: ScheduleFrequency,
    /// Email addresses to deliver the report to
    pub recipients: Vec<String>,
    /// Whether this schedule is active
    pub enabled: bool,
    /// Timestamp of the most recent execution
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of the next scheduled execution
    pub next_run: chrono::DateTime<chrono::Utc>,
}

/// Frequency at which a scheduled report runs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleFrequency {
    /// Run once per day
    Daily,
    /// Run once per week
    Weekly,
    /// Run once per month
    Monthly,
    /// Run once per quarter
    Quarterly,
    /// Run once per year
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
