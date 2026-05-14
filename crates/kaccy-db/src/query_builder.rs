//! Dynamic query builder with type-safe construction and SQL injection prevention.
//!
//! This module provides:
//! - Type-safe query construction
//! - Runtime filter composition
//! - SQL injection prevention

use std::fmt::Write;

/// SQL operator for filters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    /// Equal (=)
    Eq,
    /// Not equal (<>)
    Ne,
    /// Greater than (>)
    Gt,
    /// Greater than or equal (>=)
    Gte,
    /// Less than (<)
    Lt,
    /// Less than or equal (<=)
    Lte,
    /// LIKE
    Like,
    /// ILIKE (case-insensitive)
    ILike,
    /// IN
    In,
    /// NOT IN
    NotIn,
    /// IS NULL
    IsNull,
    /// IS NOT NULL
    IsNotNull,
}

impl Operator {
    /// Get the SQL representation
    pub fn as_sql(&self) -> &str {
        match self {
            Operator::Eq => "=",
            Operator::Ne => "<>",
            Operator::Gt => ">",
            Operator::Gte => ">=",
            Operator::Lt => "<",
            Operator::Lte => "<=",
            Operator::Like => "LIKE",
            Operator::ILike => "ILIKE",
            Operator::In => "IN",
            Operator::NotIn => "NOT IN",
            Operator::IsNull => "IS NULL",
            Operator::IsNotNull => "IS NOT NULL",
        }
    }
}

/// Filter condition
#[derive(Debug, Clone)]
pub struct Filter {
    /// Column name
    pub column: String,
    /// Operator
    pub operator: Operator,
    /// Value (None for IS NULL/IS NOT NULL)
    pub value: Option<FilterValue>,
}

/// Filter value type
#[derive(Debug, Clone)]
pub enum FilterValue {
    /// String value
    String(String),
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// Array of strings
    StringArray(Vec<String>),
    /// Array of integers
    IntArray(Vec<i64>),
}

impl Filter {
    /// Create a new filter
    pub fn new(column: String, operator: Operator, value: Option<FilterValue>) -> Self {
        Self {
            column,
            operator,
            value,
        }
    }

    /// Create an equality filter
    pub fn eq<T: Into<FilterValue>>(column: String, value: T) -> Self {
        Self::new(column, Operator::Eq, Some(value.into()))
    }

    /// Create a not-equal filter
    pub fn ne<T: Into<FilterValue>>(column: String, value: T) -> Self {
        Self::new(column, Operator::Ne, Some(value.into()))
    }

    /// Create a greater-than filter
    pub fn gt<T: Into<FilterValue>>(column: String, value: T) -> Self {
        Self::new(column, Operator::Gt, Some(value.into()))
    }

    /// Create an IN filter
    pub fn in_values(column: String, values: Vec<String>) -> Self {
        Self::new(column, Operator::In, Some(FilterValue::StringArray(values)))
    }

    /// Create an IS NULL filter
    pub fn is_null(column: String) -> Self {
        Self::new(column, Operator::IsNull, None)
    }

    /// Convert to SQL with parameterized value
    pub fn to_sql(&self, param_index: &mut usize) -> String {
        let mut sql = format!("{} {}", self.column, self.operator.as_sql());

        match (&self.operator, &self.value) {
            (Operator::IsNull | Operator::IsNotNull, _) => {
                // No value needed
            }
            (Operator::In | Operator::NotIn, Some(FilterValue::StringArray(values))) => {
                let placeholders: Vec<String> = values
                    .iter()
                    .map(|_| {
                        let placeholder = format!("${}", param_index);
                        *param_index += 1;
                        placeholder
                    })
                    .collect();
                write!(sql, " ({})", placeholders.join(", ")).expect("writing to String is infallible");
            }
            (Operator::In | Operator::NotIn, Some(FilterValue::IntArray(values))) => {
                let placeholders: Vec<String> = values
                    .iter()
                    .map(|_| {
                        let placeholder = format!("${}", param_index);
                        *param_index += 1;
                        placeholder
                    })
                    .collect();
                write!(sql, " ({})", placeholders.join(", ")).expect("writing to String is infallible");
            }
            (_, Some(_)) => {
                write!(sql, " ${}", param_index).expect("writing to String is infallible");
                *param_index += 1;
            }
            _ => {}
        }

        sql
    }
}

impl From<String> for FilterValue {
    fn from(s: String) -> Self {
        FilterValue::String(s)
    }
}

impl From<&str> for FilterValue {
    fn from(s: &str) -> Self {
        FilterValue::String(s.to_string())
    }
}

impl From<i64> for FilterValue {
    fn from(i: i64) -> Self {
        FilterValue::Int(i)
    }
}

impl From<i32> for FilterValue {
    fn from(i: i32) -> Self {
        FilterValue::Int(i as i64)
    }
}

impl From<bool> for FilterValue {
    fn from(b: bool) -> Self {
        FilterValue::Bool(b)
    }
}

impl From<f64> for FilterValue {
    fn from(f: f64) -> Self {
        FilterValue::Float(f)
    }
}

/// Logical operator for combining filters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    /// AND
    And,
    /// OR
    Or,
}

impl LogicalOp {
    /// Get the SQL representation
    pub fn as_sql(&self) -> &str {
        match self {
            LogicalOp::And => "AND",
            LogicalOp::Or => "OR",
        }
    }
}

/// Order direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    /// Ascending
    Asc,
    /// Descending
    Desc,
}

impl OrderDirection {
    /// Get the SQL representation
    pub fn as_sql(&self) -> &str {
        match self {
            OrderDirection::Asc => "ASC",
            OrderDirection::Desc => "DESC",
        }
    }
}

/// Order by clause
#[derive(Debug, Clone)]
pub struct OrderBy {
    /// Column name
    pub column: String,
    /// Direction
    pub direction: OrderDirection,
}

impl OrderBy {
    /// Create a new order by clause
    pub fn new(column: String, direction: OrderDirection) -> Self {
        Self { column, direction }
    }

    /// Create ascending order
    pub fn asc(column: String) -> Self {
        Self::new(column, OrderDirection::Asc)
    }

    /// Create descending order
    pub fn desc(column: String) -> Self {
        Self::new(column, OrderDirection::Desc)
    }

    /// Convert to SQL
    pub fn to_sql(&self) -> String {
        format!("{} {}", self.column, self.direction.as_sql())
    }
}

/// Dynamic query builder
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    table: String,
    columns: Vec<String>,
    filters: Vec<Filter>,
    logical_op: LogicalOp,
    order_by: Vec<OrderBy>,
    limit: Option<i64>,
    offset: Option<i64>,
}

impl QueryBuilder {
    /// Create a new query builder for a table
    pub fn new(table: String) -> Self {
        Self {
            table,
            columns: vec!["*".to_string()],
            filters: Vec::new(),
            logical_op: LogicalOp::And,
            order_by: Vec::new(),
            limit: None,
            offset: None,
        }
    }

    /// Select specific columns
    pub fn select(mut self, columns: Vec<String>) -> Self {
        self.columns = columns;
        self
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

    /// Set the logical operator for combining filters (AND/OR)
    pub fn logical_op(mut self, op: LogicalOp) -> Self {
        self.logical_op = op;
        self
    }

    /// Add an order by clause
    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order_by.push(order);
        self
    }

    /// Set limit
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set offset
    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Build the SQL query
    pub fn build(&self) -> String {
        let mut sql = format!("SELECT {} FROM {}", self.columns.join(", "), self.table);

        if !self.filters.is_empty() {
            sql.push_str(" WHERE ");
            let mut param_index = 1;

            for (i, filter) in self.filters.iter().enumerate() {
                if i > 0 {
                    write!(sql, " {} ", self.logical_op.as_sql()).expect("writing to String is infallible");
                }
                write!(sql, "{}", filter.to_sql(&mut param_index)).expect("writing to String is infallible");
            }
        }

        if !self.order_by.is_empty() {
            sql.push_str(" ORDER BY ");
            let order_clauses: Vec<String> = self.order_by.iter().map(|o| o.to_sql()).collect();
            sql.push_str(&order_clauses.join(", "));
        }

        if let Some(limit) = self.limit {
            write!(sql, " LIMIT {}", limit).expect("writing to String is infallible");
        }

        if let Some(offset) = self.offset {
            write!(sql, " OFFSET {}", offset).expect("writing to String is infallible");
        }

        sql
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operator_as_sql() {
        assert_eq!(Operator::Eq.as_sql(), "=");
        assert_eq!(Operator::Ne.as_sql(), "<>");
        assert_eq!(Operator::Gt.as_sql(), ">");
        assert_eq!(Operator::Like.as_sql(), "LIKE");
        assert_eq!(Operator::In.as_sql(), "IN");
        assert_eq!(Operator::IsNull.as_sql(), "IS NULL");
    }

    #[test]
    fn test_filter_eq() {
        let filter = Filter::eq("name".to_string(), "John");
        assert_eq!(filter.column, "name");
        assert_eq!(filter.operator, Operator::Eq);
    }

    #[test]
    fn test_filter_is_null() {
        let filter = Filter::is_null("deleted_at".to_string());
        assert_eq!(filter.column, "deleted_at");
        assert_eq!(filter.operator, Operator::IsNull);
        assert!(filter.value.is_none());
    }

    #[test]
    fn test_filter_to_sql() {
        let mut param_index = 1;
        let filter = Filter::eq("age".to_string(), 25);
        let sql = filter.to_sql(&mut param_index);
        assert_eq!(sql, "age = $1");
        assert_eq!(param_index, 2);
    }

    #[test]
    fn test_filter_to_sql_is_null() {
        let mut param_index = 1;
        let filter = Filter::is_null("deleted_at".to_string());
        let sql = filter.to_sql(&mut param_index);
        assert_eq!(sql, "deleted_at IS NULL");
        assert_eq!(param_index, 1); // No parameter added
    }

    #[test]
    fn test_order_by() {
        let order = OrderBy::asc("created_at".to_string());
        assert_eq!(order.to_sql(), "created_at ASC");

        let order = OrderBy::desc("updated_at".to_string());
        assert_eq!(order.to_sql(), "updated_at DESC");
    }

    #[test]
    fn test_query_builder_simple() {
        let query = QueryBuilder::new("users".to_string()).build();
        assert_eq!(query, "SELECT * FROM users");
    }

    #[test]
    fn test_query_builder_with_filter() {
        let query = QueryBuilder::new("users".to_string())
            .filter(Filter::eq("email".to_string(), "test@example.com"))
            .build();

        assert!(query.contains("SELECT * FROM users"));
        assert!(query.contains("WHERE email = $1"));
    }

    #[test]
    fn test_query_builder_with_multiple_filters() {
        let query = QueryBuilder::new("users".to_string())
            .filter(Filter::eq("active".to_string(), true))
            .filter(Filter::gt("age".to_string(), 18))
            .build();

        assert!(query.contains("WHERE active = $1 AND age > $2"));
    }

    #[test]
    fn test_query_builder_with_order() {
        let query = QueryBuilder::new("users".to_string())
            .order_by(OrderBy::desc("created_at".to_string()))
            .build();

        assert!(query.contains("ORDER BY created_at DESC"));
    }

    #[test]
    fn test_query_builder_with_limit_offset() {
        let query = QueryBuilder::new("users".to_string())
            .limit(10)
            .offset(20)
            .build();

        assert!(query.contains("LIMIT 10"));
        assert!(query.contains("OFFSET 20"));
    }

    #[test]
    fn test_query_builder_full() {
        let query = QueryBuilder::new("users".to_string())
            .select(vec![
                "id".to_string(),
                "name".to_string(),
                "email".to_string(),
            ])
            .filter(Filter::eq("active".to_string(), true))
            .filter(Filter::gt("age".to_string(), 18))
            .order_by(OrderBy::desc("created_at".to_string()))
            .limit(10)
            .offset(0)
            .build();

        assert!(query.contains("SELECT id, name, email FROM users"));
        assert!(query.contains("WHERE active = $1 AND age > $2"));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(query.contains("LIMIT 10"));
        assert!(query.contains("OFFSET 0"));
    }
}
