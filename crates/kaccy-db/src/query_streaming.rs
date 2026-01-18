//! Query result streaming for memory-efficient iteration over large result sets.
//!
//! This module provides:
//! - Async stream for large result sets
//! - Backpressure handling
//! - Memory-efficient iteration

use futures::{Stream, StreamExt as FuturesStreamExt};
use sqlx::{FromRow, PgPool};

use crate::error::Result;

/// Configuration for query streaming
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Batch size for fetching rows
    pub fetch_size: usize,
    /// Maximum number of concurrent fetches
    pub max_concurrent_fetches: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            fetch_size: 100,
            max_concurrent_fetches: 2,
        }
    }
}

/// Query builder for streaming results
///
/// This is a lightweight wrapper that helps construct streaming queries.
/// It doesn't implement Stream itself to avoid lifetime issues.
pub struct QueryStream<T> {
    pool: PgPool,
    query: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> QueryStream<T>
where
    T: Send + 'static,
{
    /// Create a new query stream builder
    ///
    /// The pool is cloned (cheap operation as PgPool uses Arc internally)
    pub fn new(pool: &PgPool, query: impl Into<String>) -> Self {
        Self {
            pool: pool.clone(),
            query: query.into(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Execute the query and collect all results into a Vec
    ///
    /// This is the recommended method for most use cases.
    pub async fn collect_vec(self) -> Result<Vec<T>>
    where
        T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Unpin,
    {
        use futures::TryStreamExt;

        let results: Vec<T> = sqlx::query_as(&self.query)
            .fetch(&self.pool)
            .try_collect()
            .await?;

        Ok(results)
    }

    /// Execute the query and process results one by one with a processor function
    ///
    /// This is useful for processing large result sets without loading everything into memory.
    pub async fn for_each<F, Fut>(self, mut processor: F) -> Result<u64>
    where
        T: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Unpin,
        F: FnMut(T) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut stream = sqlx::query_as::<_, T>(&self.query).fetch(&self.pool);
        let mut count = 0u64;

        while let Some(row) = FuturesStreamExt::next(&mut stream).await {
            let row = row?;
            processor(row).await?;
            count += 1;
        }

        Ok(count)
    }
}

/// Buffered stream processor for handling backpressure
pub struct BufferedStreamProcessor<T> {
    buffer_size: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> BufferedStreamProcessor<T> {
    /// Create a new buffered stream processor
    pub fn new(buffer_size: usize) -> Self {
        Self {
            buffer_size,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Process a stream with backpressure handling
    pub async fn process<S, F, Fut>(&self, mut stream: S, mut processor: F) -> Result<u64>
    where
        S: Stream<Item = Result<T>> + Unpin,
        F: FnMut(T) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
        T: Send,
    {
        let mut processed = 0u64;
        let mut buffer = Vec::with_capacity(self.buffer_size);

        while let Some(item) = FuturesStreamExt::next(&mut stream).await {
            let item = item?;
            buffer.push(item);

            if buffer.len() >= self.buffer_size {
                for item in buffer.drain(..) {
                    processor(item).await?;
                    processed += 1;
                }
            }
        }

        // Process remaining items
        for item in buffer {
            processor(item).await?;
            processed += 1;
        }

        Ok(processed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert_eq!(config.fetch_size, 100);
        assert_eq!(config.max_concurrent_fetches, 2);
    }

    #[test]
    fn test_buffered_stream_processor_creation() {
        let processor = BufferedStreamProcessor::<i32>::new(50);
        assert_eq!(processor.buffer_size, 50);
    }
}
