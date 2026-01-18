//! Cursor-based pagination for efficient navigation of large datasets.
//!
//! This module provides:
//! - Keyset pagination for large datasets
//! - Bidirectional cursor support
//! - Stable ordering guarantees

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Cursor for pagination
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor(String);

impl Cursor {
    /// Create a new cursor from a string
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Encode a cursor value to base64
    pub fn encode(value: &str) -> Self {
        let encoded = general_purpose::STANDARD.encode(value.as_bytes());
        Self(encoded)
    }

    /// Decode a cursor from base64
    pub fn decode(&self) -> Result<String, String> {
        general_purpose::STANDARD
            .decode(&self.0)
            .map_err(|e| format!("Failed to decode cursor: {}", e))
            .and_then(|bytes| {
                String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8 in cursor: {}", e))
            })
    }

    /// Get the raw cursor value
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Cursor {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Direction for cursor pagination
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Forward pagination (next page)
    Forward,
    /// Backward pagination (previous page)
    Backward,
}

/// Cursor pagination parameters
#[derive(Debug, Clone)]
pub struct CursorPagination {
    /// Cursor to start from
    pub cursor: Option<Cursor>,
    /// Number of items to fetch
    pub limit: u64,
    /// Direction of pagination
    pub direction: Direction,
}

impl Default for CursorPagination {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 20,
            direction: Direction::Forward,
        }
    }
}

impl CursorPagination {
    /// Create a new cursor pagination
    pub fn new(cursor: Option<Cursor>, limit: u64, direction: Direction) -> Self {
        Self {
            cursor,
            limit,
            direction,
        }
    }

    /// Create forward pagination
    pub fn forward(limit: u64) -> Self {
        Self {
            cursor: None,
            limit,
            direction: Direction::Forward,
        }
    }

    /// Create forward pagination with cursor
    pub fn after(cursor: Cursor, limit: u64) -> Self {
        Self {
            cursor: Some(cursor),
            limit,
            direction: Direction::Forward,
        }
    }

    /// Create backward pagination with cursor
    pub fn before(cursor: Cursor, limit: u64) -> Self {
        Self {
            cursor: Some(cursor),
            limit,
            direction: Direction::Backward,
        }
    }

    /// Get the SQL limit clause
    pub fn sql_limit(&self) -> i64 {
        // Fetch one extra to determine if there are more pages
        (self.limit + 1) as i64
    }
}

/// Edge in a cursor-paginated result
#[derive(Debug, Clone, Serialize)]
pub struct Edge<T> {
    /// The cursor for this edge
    pub cursor: Cursor,
    /// The node (actual data)
    pub node: T,
}

impl<T> Edge<T> {
    /// Create a new edge
    pub fn new(cursor: Cursor, node: T) -> Self {
        Self { cursor, node }
    }
}

/// Page information for cursor pagination
#[derive(Debug, Clone, Serialize)]
pub struct PageInfo {
    /// Whether there is a next page
    pub has_next_page: bool,
    /// Whether there is a previous page
    pub has_previous_page: bool,
    /// Cursor of the first edge
    pub start_cursor: Option<Cursor>,
    /// Cursor of the last edge
    pub end_cursor: Option<Cursor>,
}

impl PageInfo {
    /// Create a new page info
    pub fn new(
        has_next_page: bool,
        has_previous_page: bool,
        start_cursor: Option<Cursor>,
        end_cursor: Option<Cursor>,
    ) -> Self {
        Self {
            has_next_page,
            has_previous_page,
            start_cursor,
            end_cursor,
        }
    }
}

/// Cursor-paginated connection
#[derive(Debug, Clone, Serialize)]
pub struct Connection<T> {
    /// Edges in this connection
    pub edges: Vec<Edge<T>>,
    /// Page information
    pub page_info: PageInfo,
    /// Total count (optional, can be expensive to compute)
    pub total_count: Option<u64>,
}

impl<T> Connection<T> {
    /// Create a new connection from edges
    pub fn new(
        mut edges: Vec<Edge<T>>,
        pagination: &CursorPagination,
        has_previous: bool,
        total_count: Option<u64>,
    ) -> Self {
        let has_next = edges.len() > pagination.limit as usize;

        // Remove the extra item we fetched
        if has_next {
            edges.pop();
        }

        let start_cursor = edges.first().map(|e| e.cursor.clone());
        let end_cursor = edges.last().map(|e| e.cursor.clone());

        let page_info = PageInfo::new(has_next, has_previous, start_cursor, end_cursor);

        Self {
            edges,
            page_info,
            total_count,
        }
    }

    /// Get the nodes (without cursor information)
    pub fn nodes(&self) -> Vec<&T> {
        self.edges.iter().map(|e| &e.node).collect()
    }

    /// Convert to nodes (consuming the connection)
    pub fn into_nodes(self) -> Vec<T> {
        self.edges.into_iter().map(|e| e.node).collect()
    }
}

/// Helper to build cursor from multiple fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorBuilder {
    fields: Vec<String>,
}

impl CursorBuilder {
    /// Create a new cursor builder
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }

    /// Add a field to the cursor
    pub fn add_field<T: ToString>(mut self, value: T) -> Self {
        self.fields.push(value.to_string());
        self
    }

    /// Build the cursor
    pub fn build(self) -> Cursor {
        let value = self.fields.join("|");
        Cursor::encode(&value)
    }

    /// Parse a cursor into fields
    pub fn parse(cursor: &Cursor) -> Result<Vec<String>, String> {
        let decoded = cursor.decode()?;
        Ok(decoded.split('|').map(|s| s.to_string()).collect())
    }
}

impl Default for CursorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_encode_decode() {
        let cursor = Cursor::encode("test-id-123");
        let decoded = cursor.decode().unwrap();
        assert_eq!(decoded, "test-id-123");
    }

    #[test]
    fn test_cursor_display() {
        let cursor = Cursor::new("abc123".to_string());
        assert_eq!(format!("{}", cursor), "abc123");
    }

    #[test]
    fn test_cursor_pagination_default() {
        let pagination = CursorPagination::default();
        assert_eq!(pagination.cursor, None);
        assert_eq!(pagination.limit, 20);
        assert_eq!(pagination.direction, Direction::Forward);
    }

    #[test]
    fn test_cursor_pagination_forward() {
        let pagination = CursorPagination::forward(10);
        assert_eq!(pagination.cursor, None);
        assert_eq!(pagination.limit, 10);
        assert_eq!(pagination.direction, Direction::Forward);
    }

    #[test]
    fn test_cursor_pagination_after() {
        let cursor = Cursor::new("abc".to_string());
        let pagination = CursorPagination::after(cursor.clone(), 15);
        assert_eq!(pagination.cursor, Some(cursor));
        assert_eq!(pagination.limit, 15);
        assert_eq!(pagination.direction, Direction::Forward);
    }

    #[test]
    fn test_cursor_pagination_before() {
        let cursor = Cursor::new("xyz".to_string());
        let pagination = CursorPagination::before(cursor.clone(), 10);
        assert_eq!(pagination.cursor, Some(cursor));
        assert_eq!(pagination.limit, 10);
        assert_eq!(pagination.direction, Direction::Backward);
    }

    #[test]
    fn test_cursor_pagination_sql_limit() {
        let pagination = CursorPagination::forward(20);
        assert_eq!(pagination.sql_limit(), 21); // Fetches one extra
    }

    #[test]
    fn test_edge_creation() {
        let cursor = Cursor::new("cursor1".to_string());
        let edge = Edge::new(cursor.clone(), "data");
        assert_eq!(edge.cursor, cursor);
        assert_eq!(edge.node, "data");
    }

    #[test]
    fn test_page_info_creation() {
        let cursor1 = Cursor::new("c1".to_string());
        let cursor2 = Cursor::new("c2".to_string());
        let page_info = PageInfo::new(true, false, Some(cursor1.clone()), Some(cursor2.clone()));

        assert!(page_info.has_next_page);
        assert!(!page_info.has_previous_page);
        assert_eq!(page_info.start_cursor, Some(cursor1));
        assert_eq!(page_info.end_cursor, Some(cursor2));
    }

    #[test]
    fn test_connection_creation() {
        let edges = vec![
            Edge::new(Cursor::new("c1".to_string()), 1),
            Edge::new(Cursor::new("c2".to_string()), 2),
            Edge::new(Cursor::new("c3".to_string()), 3),
        ];

        let pagination = CursorPagination::forward(2);
        let connection = Connection::new(edges, &pagination, false, Some(10));

        assert_eq!(connection.edges.len(), 2); // Extra removed
        assert!(connection.page_info.has_next_page);
        assert!(!connection.page_info.has_previous_page);
        assert_eq!(connection.total_count, Some(10));
    }

    #[test]
    fn test_connection_nodes() {
        let edges = vec![
            Edge::new(Cursor::new("c1".to_string()), "a"),
            Edge::new(Cursor::new("c2".to_string()), "b"),
        ];

        let pagination = CursorPagination::forward(5);
        let connection = Connection::new(edges, &pagination, false, None);

        let nodes = connection.nodes();
        assert_eq!(nodes, vec![&"a", &"b"]);
    }

    #[test]
    fn test_cursor_builder() {
        let cursor = CursorBuilder::new()
            .add_field("field1")
            .add_field(123)
            .add_field("field3")
            .build();

        let fields = CursorBuilder::parse(&cursor).unwrap();
        assert_eq!(fields, vec!["field1", "123", "field3"]);
    }

    #[test]
    fn test_cursor_builder_empty() {
        let cursor = CursorBuilder::new().build();
        let fields = CursorBuilder::parse(&cursor).unwrap();
        assert_eq!(fields, vec![""]);
    }
}
