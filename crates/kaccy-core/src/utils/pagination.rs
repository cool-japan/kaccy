//! Pagination utilities for list queries

use serde::{Deserialize, Serialize};

/// Pagination parameters for queries
#[derive(Debug, Clone, Deserialize)]
pub struct Pagination {
    /// Page number (1-indexed)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

impl Pagination {
    /// Create new pagination with page and per_page
    pub fn new(page: u32, per_page: u32) -> Self {
        Self { page, per_page }
    }

    /// Calculate SQL LIMIT
    pub fn limit(&self) -> u32 {
        self.per_page.min(100) // Cap at 100
    }

    /// Calculate SQL OFFSET
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.limit()
    }

    /// Get the page number
    pub fn page(&self) -> u32 {
        self.page
    }

    /// Get items per page
    pub fn per_page(&self) -> u32 {
        self.per_page
    }

    /// Validate pagination parameters
    pub fn validate(&self) -> Result<(), String> {
        if self.page == 0 {
            return Err("Page number must be at least 1".to_string());
        }
        if self.per_page == 0 {
            return Err("Per page must be at least 1".to_string());
        }
        if self.per_page > 100 {
            return Err("Per page cannot exceed 100".to_string());
        }
        Ok(())
    }
}

/// Paginated response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResponse<T> {
    /// The items for this page
    pub items: Vec<T>,
    /// Current page number
    pub page: u32,
    /// Items per page
    pub per_page: u32,
    /// Total number of items
    pub total: i64,
    /// Total number of pages
    pub total_pages: u32,
    /// Whether there is a next page
    pub has_next: bool,
    /// Whether there is a previous page
    pub has_prev: bool,
}

impl<T> PaginatedResponse<T> {
    /// Create a new paginated response
    pub fn new(items: Vec<T>, pagination: &Pagination, total: i64) -> Self {
        let total_pages = if total == 0 {
            0
        } else {
            ((total as f64) / (pagination.per_page as f64)).ceil() as u32
        };

        let has_next = pagination.page < total_pages;
        let has_prev = pagination.page > 1;

        Self {
            items,
            page: pagination.page,
            per_page: pagination.per_page,
            total,
            total_pages,
            has_next,
            has_prev,
        }
    }

    /// Get the next page number (if exists)
    pub fn next_page(&self) -> Option<u32> {
        if self.has_next {
            Some(self.page + 1)
        } else {
            None
        }
    }

    /// Get the previous page number (if exists)
    pub fn prev_page(&self) -> Option<u32> {
        if self.has_prev {
            Some(self.page - 1)
        } else {
            None
        }
    }

    /// Map items to a different type
    pub fn map<U, F>(self, f: F) -> PaginatedResponse<U>
    where
        F: FnMut(T) -> U,
    {
        PaginatedResponse {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            per_page: self.per_page,
            total: self.total,
            total_pages: self.total_pages,
            has_next: self.has_next,
            has_prev: self.has_prev,
        }
    }

    /// Check if response is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get number of items in this page
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Cursor-based pagination for efficient large dataset queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPagination {
    /// Cursor (typically an ID or timestamp)
    pub cursor: Option<String>,
    /// Number of items to fetch
    #[serde(default = "default_per_page")]
    pub limit: u32,
}

impl Default for CursorPagination {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: 20,
        }
    }
}

impl CursorPagination {
    pub fn new(cursor: Option<String>, limit: u32) -> Self {
        Self { cursor, limit }
    }

    /// Get the limit capped at maximum
    pub fn limit(&self) -> u32 {
        self.limit.min(100)
    }

    /// Validate cursor pagination
    pub fn validate(&self) -> Result<(), String> {
        if self.limit == 0 {
            return Err("Limit must be at least 1".to_string());
        }
        if self.limit > 100 {
            return Err("Limit cannot exceed 100".to_string());
        }
        Ok(())
    }
}

/// Cursor-based paginated response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPaginatedResponse<T> {
    /// The items for this page
    pub items: Vec<T>,
    /// Cursor for the next page
    pub next_cursor: Option<String>,
    /// Whether there are more items
    pub has_more: bool,
}

impl<T> CursorPaginatedResponse<T> {
    pub fn new(items: Vec<T>, next_cursor: Option<String>) -> Self {
        let has_more = next_cursor.is_some();
        Self {
            items,
            next_cursor,
            has_more,
        }
    }

    /// Check if there are more items
    pub fn has_more(&self) -> bool {
        self.has_more
    }

    /// Get the next cursor
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }

    /// Map items to a different type
    pub fn map<U, F>(self, f: F) -> CursorPaginatedResponse<U>
    where
        F: FnMut(T) -> U,
    {
        CursorPaginatedResponse {
            items: self.items.into_iter().map(f).collect(),
            next_cursor: self.next_cursor,
            has_more: self.has_more,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_offset() {
        let pagination = Pagination::new(1, 20);
        assert_eq!(pagination.offset(), 0);
        assert_eq!(pagination.limit(), 20);

        let pagination = Pagination::new(2, 20);
        assert_eq!(pagination.offset(), 20);

        let pagination = Pagination::new(3, 10);
        assert_eq!(pagination.offset(), 20);
    }

    #[test]
    fn test_pagination_validation() {
        let valid = Pagination::new(1, 20);
        assert!(valid.validate().is_ok());

        let invalid_page = Pagination::new(0, 20);
        assert!(invalid_page.validate().is_err());

        let invalid_per_page = Pagination::new(1, 0);
        assert!(invalid_per_page.validate().is_err());

        let too_large = Pagination::new(1, 200);
        assert!(too_large.validate().is_err());
    }

    #[test]
    fn test_paginated_response() {
        let items = vec![1, 2, 3, 4, 5];
        let pagination = Pagination::new(2, 5);
        let total = 15;

        let response = PaginatedResponse::new(items, &pagination, total);

        assert_eq!(response.page, 2);
        assert_eq!(response.per_page, 5);
        assert_eq!(response.total, 15);
        assert_eq!(response.total_pages, 3);
        assert!(response.has_next);
        assert!(response.has_prev);
        assert_eq!(response.next_page(), Some(3));
        assert_eq!(response.prev_page(), Some(1));
    }

    #[test]
    fn test_paginated_response_first_page() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(1, 3);
        let total = 10;

        let response = PaginatedResponse::new(items, &pagination, total);

        assert!(response.has_next);
        assert!(!response.has_prev);
        assert_eq!(response.next_page(), Some(2));
        assert_eq!(response.prev_page(), None);
    }

    #[test]
    fn test_paginated_response_last_page() {
        let items = vec![10];
        let pagination = Pagination::new(4, 3);
        let total = 10;

        let response = PaginatedResponse::new(items, &pagination, total);

        assert!(!response.has_next);
        assert!(response.has_prev);
        assert_eq!(response.next_page(), None);
        assert_eq!(response.prev_page(), Some(3));
    }

    #[test]
    fn test_cursor_pagination() {
        let cursor = CursorPagination::new(Some("abc123".to_string()), 20);
        assert_eq!(cursor.cursor, Some("abc123".to_string()));
        assert_eq!(cursor.limit(), 20);

        assert!(cursor.validate().is_ok());
    }

    #[test]
    fn test_cursor_response() {
        let items = vec![1, 2, 3];
        let response = CursorPaginatedResponse::new(items, Some("next_cursor".to_string()));

        assert!(response.has_more());
        assert_eq!(response.next_cursor(), Some("next_cursor"));
    }

    #[test]
    fn test_map_paginated_response() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(1, 10);
        let response = PaginatedResponse::new(items, &pagination, 3);

        let mapped = response.map(|x| x * 2);
        assert_eq!(mapped.items, vec![2, 4, 6]);
        assert_eq!(mapped.total, 3);
    }
}
