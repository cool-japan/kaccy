//! Generic repository trait for standardized CRUD operations.
//!
//! This module provides:
//! - Base trait for CRUD operations
//! - Automatic caching integration
//! - Generic pagination support

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::RedisCache;
use crate::error::Result;

/// Pagination parameters
#[derive(Debug, Clone)]
pub struct Pagination {
    /// Page number (0-indexed)
    pub page: u64,
    /// Number of items per page
    pub page_size: u64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: 20,
        }
    }
}

impl Pagination {
    /// Create a new pagination
    pub fn new(page: u64, page_size: u64) -> Self {
        Self { page, page_size }
    }

    /// Get the LIMIT value
    pub fn limit(&self) -> i64 {
        self.page_size as i64
    }

    /// Get the OFFSET value
    pub fn offset(&self) -> i64 {
        (self.page * self.page_size) as i64
    }
}

/// Paginated result
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    /// Items in this page
    pub items: Vec<T>,
    /// Total number of items
    pub total: u64,
    /// Current page number
    pub page: u64,
    /// Page size
    pub page_size: u64,
    /// Total number of pages
    pub total_pages: u64,
}

impl<T> Page<T> {
    /// Create a new page
    pub fn new(items: Vec<T>, total: u64, pagination: &Pagination) -> Self {
        let total_pages = if pagination.page_size > 0 {
            total.div_ceil(pagination.page_size)
        } else {
            0
        };

        Self {
            items,
            total,
            page: pagination.page,
            page_size: pagination.page_size,
            total_pages,
        }
    }

    /// Check if there is a next page
    pub fn has_next(&self) -> bool {
        self.page + 1 < self.total_pages
    }

    /// Check if there is a previous page
    pub fn has_prev(&self) -> bool {
        self.page > 0
    }
}

/// Generic repository trait for CRUD operations
#[async_trait]
pub trait Repository<T>: Send + Sync
where
    T: Send + Sync + Clone + Serialize + DeserializeOwned,
{
    /// Get the entity type name for caching
    fn entity_name(&self) -> &str;

    /// Find an entity by ID
    async fn find_by_id(&self, id: Uuid) -> Result<Option<T>>;

    /// Find all entities with pagination
    async fn find_all(&self, pagination: Pagination) -> Result<Page<T>>;

    /// Create a new entity
    async fn create(&self, entity: &T) -> Result<T>;

    /// Update an existing entity
    async fn update(&self, id: Uuid, entity: &T) -> Result<T>;

    /// Delete an entity by ID
    async fn delete(&self, id: Uuid) -> Result<bool>;

    /// Count total entities
    async fn count(&self) -> Result<u64>;

    /// Check if an entity exists
    async fn exists(&self, id: Uuid) -> Result<bool> {
        Ok(self.find_by_id(id).await?.is_some())
    }
}

/// Generic repository implementation with caching
#[allow(dead_code)]
pub struct CachedGenericRepository<T>
where
    T: Send + Sync + Clone + Serialize + DeserializeOwned,
{
    pool: Arc<PgPool>,
    cache: Option<Arc<RedisCache>>,
    entity_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> CachedGenericRepository<T>
where
    T: Send + Sync + Clone + Serialize + DeserializeOwned,
{
    /// Create a new cached generic repository
    pub fn new(pool: Arc<PgPool>, cache: Option<Arc<RedisCache>>, entity_name: String) -> Self {
        Self {
            pool,
            cache,
            entity_name,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get cache key for an entity
    #[allow(dead_code)]
    fn cache_key(&self, id: Uuid) -> String {
        format!("{}:{}", self.entity_name, id)
    }

    /// Get from cache
    #[allow(dead_code)]
    async fn get_from_cache(&self, id: Uuid) -> Result<Option<T>> {
        if let Some(cache) = &self.cache {
            cache.get(&self.cache_key(id)).await
        } else {
            Ok(None)
        }
    }

    /// Set in cache
    #[allow(dead_code)]
    async fn set_in_cache(&self, id: Uuid, entity: &T, ttl_secs: u64) -> Result<()> {
        if let Some(cache) = &self.cache {
            cache.set(&self.cache_key(id), entity, ttl_secs).await?;
        }
        Ok(())
    }

    /// Invalidate cache
    #[allow(dead_code)]
    async fn invalidate_cache(&self, id: Uuid) -> Result<()> {
        if let Some(cache) = &self.cache {
            cache.delete(&self.cache_key(id)).await?;
        }
        Ok(())
    }

    /// Get pool reference
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_default() {
        let pagination = Pagination::default();
        assert_eq!(pagination.page, 0);
        assert_eq!(pagination.page_size, 20);
        assert_eq!(pagination.limit(), 20);
        assert_eq!(pagination.offset(), 0);
    }

    #[test]
    fn test_pagination_offset() {
        let pagination = Pagination::new(2, 10);
        assert_eq!(pagination.limit(), 10);
        assert_eq!(pagination.offset(), 20);
    }

    #[test]
    fn test_page_creation() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(0, 10);
        let page = Page::new(items, 25, &pagination);

        assert_eq!(page.items.len(), 3);
        assert_eq!(page.total, 25);
        assert_eq!(page.page, 0);
        assert_eq!(page.page_size, 10);
        assert_eq!(page.total_pages, 3);
    }

    #[test]
    fn test_page_has_next() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(0, 10);
        let page = Page::new(items, 25, &pagination);

        assert!(page.has_next());
        assert!(!page.has_prev());
    }

    #[test]
    fn test_page_has_prev() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(1, 10);
        let page = Page::new(items, 25, &pagination);

        assert!(page.has_next());
        assert!(page.has_prev());
    }

    #[test]
    fn test_page_last_page() {
        let items = vec![1, 2, 3];
        let pagination = Pagination::new(2, 10);
        let page = Page::new(items, 25, &pagination);

        assert!(!page.has_next());
        assert!(page.has_prev());
    }
}
