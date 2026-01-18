//! Query builder utilities for filtering and sorting

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Sort direction
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

impl SortDirection {
    pub fn to_sql(&self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

/// Sort field specification
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SortBy {
    /// Field name to sort by
    pub field: String,
    /// Sort direction
    #[serde(default)]
    pub direction: SortDirection,
}

impl SortBy {
    pub fn new(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
        }
    }

    pub fn asc(field: impl Into<String>) -> Self {
        Self::new(field, SortDirection::Asc)
    }

    pub fn desc(field: impl Into<String>) -> Self {
        Self::new(field, SortDirection::Desc)
    }

    /// Convert to SQL ORDER BY clause
    pub fn to_sql(&self, allowed_fields: &[&str]) -> Result<String, String> {
        if !allowed_fields.contains(&self.field.as_str()) {
            return Err(format!("Invalid sort field: {}", self.field));
        }
        Ok(format!("{} {}", self.field, self.direction.to_sql()))
    }
}

/// Filter operator
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FilterOperator {
    Eq,      // Equal
    Ne,      // Not equal
    Gt,      // Greater than
    Gte,     // Greater than or equal
    Lt,      // Less than
    Lte,     // Less than or equal
    Like,    // SQL LIKE
    In,      // IN (...)
    NotIn,   // NOT IN (...)
    IsNull,  // IS NULL
    NotNull, // IS NOT NULL
}

impl FilterOperator {
    pub fn to_sql(&self) -> &'static str {
        match self {
            FilterOperator::Eq => "=",
            FilterOperator::Ne => "!=",
            FilterOperator::Gt => ">",
            FilterOperator::Gte => ">=",
            FilterOperator::Lt => "<",
            FilterOperator::Lte => "<=",
            FilterOperator::Like => "LIKE",
            FilterOperator::In => "IN",
            FilterOperator::NotIn => "NOT IN",
            FilterOperator::IsNull => "IS NULL",
            FilterOperator::NotNull => "IS NOT NULL",
        }
    }
}

/// Filter condition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Filter {
    /// Field name
    pub field: String,
    /// Operator
    pub operator: FilterOperator,
    /// Value (for operators that need it)
    pub value: Option<serde_json::Value>,
}

impl Filter {
    pub fn new(field: impl Into<String>, operator: FilterOperator) -> Self {
        Self {
            field: field.into(),
            operator,
            value: None,
        }
    }

    pub fn with_value(mut self, value: serde_json::Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Create equality filter
    pub fn eq(field: impl Into<String>, value: serde_json::Value) -> Self {
        Self::new(field, FilterOperator::Eq).with_value(value)
    }

    /// Create greater than filter
    pub fn gt(field: impl Into<String>, value: serde_json::Value) -> Self {
        Self::new(field, FilterOperator::Gt).with_value(value)
    }

    /// Create less than filter
    pub fn lt(field: impl Into<String>, value: serde_json::Value) -> Self {
        Self::new(field, FilterOperator::Lt).with_value(value)
    }

    /// Create LIKE filter
    pub fn like(field: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::new(field, FilterOperator::Like).with_value(serde_json::Value::String(pattern.into()))
    }

    /// Create IS NULL filter
    pub fn is_null(field: impl Into<String>) -> Self {
        Self::new(field, FilterOperator::IsNull)
    }

    /// Create IS NOT NULL filter
    pub fn not_null(field: impl Into<String>) -> Self {
        Self::new(field, FilterOperator::NotNull)
    }

    /// Validate filter field
    pub fn validate(&self, allowed_fields: &[&str]) -> Result<(), String> {
        if !allowed_fields.contains(&self.field.as_str()) {
            return Err(format!("Invalid filter field: {}", self.field));
        }

        // Check if value is required for operator
        match self.operator {
            FilterOperator::IsNull | FilterOperator::NotNull => {
                // These don't need values
                Ok(())
            }
            _ => {
                if self.value.is_none() {
                    Err(format!(
                        "Value required for {} operator",
                        self.operator.to_sql()
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Date range filter helper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DateRange {
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

impl DateRange {
    pub fn new(from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Self {
        Self { from, to }
    }

    pub fn from(from: DateTime<Utc>) -> Self {
        Self {
            from: Some(from),
            to: None,
        }
    }

    pub fn to(to: DateTime<Utc>) -> Self {
        Self {
            from: None,
            to: Some(to),
        }
    }

    pub fn between(from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        Self {
            from: Some(from),
            to: Some(to),
        }
    }

    /// Convert to filters
    pub fn to_filters(&self, field: &str) -> Vec<Filter> {
        let mut filters = Vec::new();

        if let Some(from) = self.from {
            filters.push(
                Filter::new(field, FilterOperator::Gte)
                    .with_value(serde_json::to_value(from.to_rfc3339()).unwrap()),
            );
        }

        if let Some(to) = self.to {
            filters.push(
                Filter::new(field, FilterOperator::Lte)
                    .with_value(serde_json::to_value(to.to_rfc3339()).unwrap()),
            );
        }

        filters
    }
}

/// Numeric range filter helper
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NumericRange {
    pub min: Option<Decimal>,
    pub max: Option<Decimal>,
}

impl NumericRange {
    pub fn new(min: Option<Decimal>, max: Option<Decimal>) -> Self {
        Self { min, max }
    }

    pub fn min(min: Decimal) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    pub fn max(max: Decimal) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }

    pub fn between(min: Decimal, max: Decimal) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Convert to filters
    pub fn to_filters(&self, field: &str) -> Vec<Filter> {
        let mut filters = Vec::new();

        if let Some(min) = self.min {
            filters.push(
                Filter::new(field, FilterOperator::Gte)
                    .with_value(serde_json::to_value(min).unwrap()),
            );
        }

        if let Some(max) = self.max {
            filters.push(
                Filter::new(field, FilterOperator::Lte)
                    .with_value(serde_json::to_value(max).unwrap()),
            );
        }

        filters
    }
}

/// Query builder for complex queries
#[derive(Debug, Clone, Default)]
pub struct QueryBuilder {
    filters: Vec<Filter>,
    sort_by: Vec<SortBy>,
}

impl QueryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a filter
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    /// Add multiple filters
    pub fn filters(mut self, filters: Vec<Filter>) -> Self {
        self.filters.extend(filters);
        self
    }

    /// Add sorting
    pub fn sort(mut self, sort: SortBy) -> Self {
        self.sort_by.push(sort);
        self
    }

    /// Add sorting by field (descending by default)
    pub fn sort_by(mut self, field: impl Into<String>) -> Self {
        self.sort_by.push(SortBy::desc(field));
        self
    }

    /// Add ascending sort
    pub fn sort_asc(mut self, field: impl Into<String>) -> Self {
        self.sort_by.push(SortBy::asc(field));
        self
    }

    /// Add descending sort
    pub fn sort_desc(mut self, field: impl Into<String>) -> Self {
        self.sort_by.push(SortBy::desc(field));
        self
    }

    /// Add date range filter
    pub fn date_range(self, field: &str, range: DateRange) -> Self {
        self.filters(range.to_filters(field))
    }

    /// Add numeric range filter
    pub fn numeric_range(self, field: &str, range: NumericRange) -> Self {
        self.filters(range.to_filters(field))
    }

    /// Add filter for UUID field
    pub fn filter_uuid(self, field: impl Into<String>, uuid: Uuid) -> Self {
        self.filter(Filter::eq(field, serde_json::to_value(uuid).unwrap()))
    }

    /// Add filter for string field
    pub fn filter_string(self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.filter(Filter::eq(field, serde_json::Value::String(value.into())))
    }

    /// Add search filter (LIKE)
    pub fn search(self, field: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.filter(Filter::like(field, format!("%{}%", pattern.into())))
    }

    /// Get all filters
    pub fn get_filters(&self) -> &[Filter] {
        &self.filters
    }

    /// Get all sorting
    pub fn get_sort(&self) -> &[SortBy] {
        &self.sort_by
    }

    /// Validate all filters and sorts
    pub fn validate(&self, allowed_fields: &[&str]) -> Result<(), String> {
        for filter in &self.filters {
            filter.validate(allowed_fields)?;
        }

        for sort in &self.sort_by {
            sort.to_sql(allowed_fields)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_sort_direction() {
        assert_eq!(SortDirection::Asc.to_sql(), "ASC");
        assert_eq!(SortDirection::Desc.to_sql(), "DESC");
    }

    #[test]
    fn test_sort_by() {
        let sort = SortBy::asc("created_at");
        assert_eq!(sort.field, "created_at");
        assert_eq!(sort.direction, SortDirection::Asc);

        let allowed = &["created_at", "updated_at"];
        assert!(sort.to_sql(allowed).is_ok());

        let invalid = SortBy::desc("invalid_field");
        assert!(invalid.to_sql(allowed).is_err());
    }

    #[test]
    fn test_filter_operator() {
        assert_eq!(FilterOperator::Eq.to_sql(), "=");
        assert_eq!(FilterOperator::Gt.to_sql(), ">");
        assert_eq!(FilterOperator::Like.to_sql(), "LIKE");
    }

    #[test]
    fn test_filter_creation() {
        let filter = Filter::eq("status", serde_json::Value::String("active".to_string()));
        assert_eq!(filter.field, "status");
        assert_eq!(filter.operator, FilterOperator::Eq);
        assert!(filter.value.is_some());

        let null_filter = Filter::is_null("deleted_at");
        assert_eq!(null_filter.operator, FilterOperator::IsNull);
        assert!(null_filter.value.is_none());
    }

    #[test]
    fn test_date_range() {
        let now = Utc::now();
        let range = DateRange::between(now, now);

        let filters = range.to_filters("created_at");
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].operator, FilterOperator::Gte);
        assert_eq!(filters[1].operator, FilterOperator::Lte);
    }

    #[test]
    fn test_numeric_range() {
        let range = NumericRange::between(dec!(10), dec!(100));

        let filters = range.to_filters("price");
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0].operator, FilterOperator::Gte);
        assert_eq!(filters[1].operator, FilterOperator::Lte);
    }

    #[test]
    fn test_query_builder() {
        let query = QueryBuilder::new()
            .filter_string("status", "active")
            .search("name", "test")
            .sort_desc("created_at")
            .sort_asc("name");

        assert_eq!(query.get_filters().len(), 2);
        assert_eq!(query.get_sort().len(), 2);
    }

    #[test]
    fn test_query_validation() {
        let query = QueryBuilder::new()
            .filter_string("status", "active")
            .sort_desc("created_at");

        let allowed = &["status", "created_at", "name"];
        assert!(query.validate(allowed).is_ok());

        let invalid_query = QueryBuilder::new().filter_string("invalid", "value");
        assert!(invalid_query.validate(allowed).is_err());
    }
}
