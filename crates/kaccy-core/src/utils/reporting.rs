//! Custom Reporting System
//!
//! This module provides a comprehensive reporting framework including:
//! - Report builder with flexible schema definition
//! - Scheduled report generation
//! - Multi-format export (JSON, CSV, HTML, Markdown)
//! - Interactive dashboard components

use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Report format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportFormat {
    /// JSON format
    Json,
    /// CSV format
    Csv,
    /// HTML format
    Html,
    /// Markdown format
    Markdown,
}

/// Report column data type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnType {
    /// String text
    Text(String),
    /// Numeric value
    Number(f64),
    /// Decimal value
    Decimal(Decimal),
    /// Date/time
    DateTime(DateTime<Utc>),
    /// Boolean
    Boolean(bool),
}

/// Report column definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportColumn {
    pub name: String,
    pub label: String,
    pub data_type: String,
}

impl ReportColumn {
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        data_type: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            data_type: data_type.into(),
        }
    }
}

/// Report row (map of column name to value)
pub type ReportRow = HashMap<String, ColumnType>;

/// Report data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportData {
    pub title: String,
    pub description: Option<String>,
    pub columns: Vec<ReportColumn>,
    pub rows: Vec<ReportRow>,
    pub generated_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl ReportData {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            columns: Vec::new(),
            rows: Vec::new(),
            generated_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn add_column(&mut self, column: ReportColumn) {
        self.columns.push(column);
    }

    pub fn add_row(&mut self, row: ReportRow) {
        self.rows.push(row);
    }

    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
}

/// Report builder for creating custom reports
pub struct ReportBuilder {
    data: ReportData,
}

impl ReportBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            data: ReportData::new(title),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.data.description = Some(description.into());
        self
    }

    pub fn add_column(
        mut self,
        name: impl Into<String>,
        label: impl Into<String>,
        data_type: impl Into<String>,
    ) -> Self {
        self.data
            .add_column(ReportColumn::new(name, label, data_type));
        self
    }

    pub fn add_row(mut self, row: ReportRow) -> Self {
        self.data.add_row(row);
        self
    }

    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.add_metadata(key, value);
        self
    }

    pub fn build(self) -> ReportData {
        self.data
    }
}

/// Report exporter for converting reports to various formats
pub struct ReportExporter;

impl ReportExporter {
    /// Export report to JSON
    pub fn to_json(report: &ReportData) -> Result<String> {
        serde_json::to_string_pretty(report).map_err(Into::into)
    }

    /// Export report to CSV
    pub fn to_csv(report: &ReportData) -> Result<String> {
        let mut output = String::new();

        // Header row
        let headers: Vec<String> = report.columns.iter().map(|col| col.label.clone()).collect();
        output.push_str(&headers.join(","));
        output.push('\n');

        // Data rows
        for row in &report.rows {
            let values: Vec<String> = report
                .columns
                .iter()
                .map(|col| {
                    row.get(&col.name)
                        .map(Self::column_value_to_string)
                        .unwrap_or_default()
                })
                .collect();
            output.push_str(&values.join(","));
            output.push('\n');
        }

        Ok(output)
    }

    /// Export report to HTML
    pub fn to_html(report: &ReportData) -> Result<String> {
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str(&format!("<title>{}</title>\n", report.title));
        html.push_str("<style>\n");
        html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }\n");
        html.push_str("h1 { color: #333; }\n");
        html.push_str("table { border-collapse: collapse; width: 100%; margin-top: 20px; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }\n");
        html.push_str("th { background-color: #4CAF50; color: white; }\n");
        html.push_str("tr:nth-child(even) { background-color: #f2f2f2; }\n");
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str(&format!("<h1>{}</h1>\n", report.title));
        if let Some(desc) = &report.description {
            html.push_str(&format!("<p>{}</p>\n", desc));
        }
        html.push_str(&format!(
            "<p><em>Generated at: {}</em></p>\n",
            report.generated_at
        ));

        html.push_str("<table>\n<thead>\n<tr>\n");
        for col in &report.columns {
            html.push_str(&format!("<th>{}</th>\n", col.label));
        }
        html.push_str("</tr>\n</thead>\n<tbody>\n");

        for row in &report.rows {
            html.push_str("<tr>\n");
            for col in &report.columns {
                let value = row
                    .get(&col.name)
                    .map(Self::column_value_to_string)
                    .unwrap_or_default();
                html.push_str(&format!("<td>{}</td>\n", value));
            }
            html.push_str("</tr>\n");
        }

        html.push_str("</tbody>\n</table>\n</body>\n</html>");

        Ok(html)
    }

    /// Export report to Markdown
    pub fn to_markdown(report: &ReportData) -> Result<String> {
        let mut md = String::new();

        md.push_str(&format!("# {}\n\n", report.title));
        if let Some(desc) = &report.description {
            md.push_str(&format!("{}\n\n", desc));
        }
        md.push_str(&format!("*Generated at: {}*\n\n", report.generated_at));

        // Table header
        let headers: Vec<String> = report.columns.iter().map(|col| col.label.clone()).collect();
        md.push_str(&format!("| {} |\n", headers.join(" | ")));

        // Separator
        let separators: Vec<&str> = report.columns.iter().map(|_| "---").collect();
        md.push_str(&format!("| {} |\n", separators.join(" | ")));

        // Data rows
        for row in &report.rows {
            let values: Vec<String> = report
                .columns
                .iter()
                .map(|col| {
                    row.get(&col.name)
                        .map(Self::column_value_to_string)
                        .unwrap_or_default()
                })
                .collect();
            md.push_str(&format!("| {} |\n", values.join(" | ")));
        }

        Ok(md)
    }

    fn column_value_to_string(value: &ColumnType) -> String {
        match value {
            ColumnType::Text(s) => s.clone(),
            ColumnType::Number(n) => n.to_string(),
            ColumnType::Decimal(d) => d.to_string(),
            ColumnType::DateTime(dt) => dt.to_rfc3339(),
            ColumnType::Boolean(b) => b.to_string(),
        }
    }
}

/// Report schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSchedule {
    pub report_name: String,
    pub frequency: ScheduleFrequency,
    pub format: ReportFormat,
    pub recipients: Vec<String>,
    pub enabled: bool,
}

/// Schedule frequency
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScheduleFrequency {
    /// Daily at specific hour
    Daily { hour: u8 },
    /// Weekly on specific day
    Weekly { day: u8, hour: u8 },
    /// Monthly on specific day
    Monthly { day: u8, hour: u8 },
    /// Custom cron expression
    Custom,
}

/// Dashboard widget type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DashboardWidget {
    /// Metric card showing a single value
    MetricCard {
        title: String,
        value: String,
        change: Option<f64>,
        change_period: Option<String>,
    },
    /// Chart widget
    Chart {
        title: String,
        chart_type: ChartType,
        data: Vec<ChartDataPoint>,
    },
    /// Table widget
    Table {
        title: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Progress bar
    ProgressBar {
        title: String,
        current: f64,
        total: f64,
        unit: String,
    },
}

/// Chart type for visualizations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    Line,
    Bar,
    Pie,
    Area,
    Scatter,
}

/// Chart data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
}

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dashboard {
    pub id: String,
    pub name: String,
    pub layout: Vec<DashboardWidget>,
    pub refresh_interval_seconds: Option<u64>,
}

impl Dashboard {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            layout: Vec::new(),
            refresh_interval_seconds: None,
        }
    }

    pub fn add_widget(mut self, widget: DashboardWidget) -> Self {
        self.layout.push(widget);
        self
    }

    pub fn with_refresh_interval(mut self, seconds: u64) -> Self {
        self.refresh_interval_seconds = Some(seconds);
        self
    }
}

/// Report generator trait for custom report types
pub trait ReportGenerator {
    fn generate(&self) -> Result<ReportData>;
}

/// Trading volume report generator
pub struct TradingVolumeReport {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

impl ReportGenerator for TradingVolumeReport {
    fn generate(&self) -> Result<ReportData> {
        let report = ReportBuilder::new("Trading Volume Report")
            .description(format!(
                "Volume from {} to {}",
                self.start_date, self.end_date
            ))
            .add_column("token_symbol", "Token", "text")
            .add_column("total_volume", "Total Volume", "decimal")
            .add_column("trade_count", "Number of Trades", "number")
            .add_column("avg_trade_size", "Average Trade Size", "decimal")
            .metadata("start_date", self.start_date.to_rfc3339())
            .metadata("end_date", self.end_date.to_rfc3339())
            .build();

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_report_builder() {
        let mut row = HashMap::new();
        row.insert(
            "name".to_string(),
            ColumnType::Text("Test Token".to_string()),
        );
        row.insert("volume".to_string(), ColumnType::Decimal(dec!(1000)));

        let report = ReportBuilder::new("Test Report")
            .description("A test report")
            .add_column("name", "Token Name", "text")
            .add_column("volume", "Volume", "decimal")
            .add_row(row)
            .metadata("author", "System")
            .build();

        assert_eq!(report.title, "Test Report");
        assert_eq!(report.columns.len(), 2);
        assert_eq!(report.rows.len(), 1);
    }

    #[test]
    fn test_json_export() {
        let report = ReportBuilder::new("JSON Test").build();

        let json = ReportExporter::to_json(&report).unwrap();
        assert!(json.contains("JSON Test"));
    }

    #[test]
    fn test_csv_export() {
        let mut row = HashMap::new();
        row.insert("name".to_string(), ColumnType::Text("Token A".to_string()));
        row.insert("price".to_string(), ColumnType::Number(100.5));

        let report = ReportBuilder::new("CSV Test")
            .add_column("name", "Name", "text")
            .add_column("price", "Price", "number")
            .add_row(row)
            .build();

        let csv = ReportExporter::to_csv(&report).unwrap();
        assert!(csv.contains("Name,Price"));
        assert!(csv.contains("Token A,100.5"));
    }

    #[test]
    fn test_html_export() {
        let report = ReportBuilder::new("HTML Test")
            .add_column("col1", "Column 1", "text")
            .build();

        let html = ReportExporter::to_html(&report).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("HTML Test"));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn test_markdown_export() {
        let mut row = HashMap::new();
        row.insert("item".to_string(), ColumnType::Text("Item 1".to_string()));

        let report = ReportBuilder::new("Markdown Test")
            .add_column("item", "Item", "text")
            .add_row(row)
            .build();

        let md = ReportExporter::to_markdown(&report).unwrap();
        assert!(md.contains("# Markdown Test"));
        assert!(md.contains("| Item |"));
        assert!(md.contains("Item 1"));
    }

    #[test]
    fn test_dashboard_creation() {
        let dashboard = Dashboard::new("dash1", "My Dashboard")
            .add_widget(DashboardWidget::MetricCard {
                title: "Total Volume".to_string(),
                value: "$1,000,000".to_string(),
                change: Some(5.2),
                change_period: Some("24h".to_string()),
            })
            .with_refresh_interval(30);

        assert_eq!(dashboard.name, "My Dashboard");
        assert_eq!(dashboard.layout.len(), 1);
        assert_eq!(dashboard.refresh_interval_seconds, Some(30));
    }

    #[test]
    fn test_report_schedule() {
        let schedule = ReportSchedule {
            report_name: "Daily Volume".to_string(),
            frequency: ScheduleFrequency::Daily { hour: 9 },
            format: ReportFormat::Json,
            recipients: vec!["admin@example.com".to_string()],
            enabled: true,
        };

        assert_eq!(schedule.report_name, "Daily Volume");
        assert!(schedule.enabled);
    }

    #[test]
    fn test_trading_volume_report_generator() {
        let report_gen = TradingVolumeReport {
            start_date: Utc::now(),
            end_date: Utc::now(),
        };

        let report = report_gen.generate().unwrap();
        assert_eq!(report.title, "Trading Volume Report");
        assert!(!report.columns.is_empty());
    }
}
