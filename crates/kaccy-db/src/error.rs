//! Database error types

use thiserror::Error;

/// Top-level database error type for the kaccy-db crate.
#[derive(Error, Debug)]
pub enum DbError {
    /// Underlying SQLx database error.
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// Requested record was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// A unique constraint violation occurred.
    #[error("Duplicate: {0}")]
    Duplicate(String),

    /// Connection pool error.
    #[error("Pool error: {0}")]
    Pool(String),

    /// Database connection error.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Cache (Redis) error.
    #[error("Cache error: {0}")]
    Cache(String),

    /// Query construction or execution error.
    #[error("Query error: {0}")]
    Query(String),

    /// Input validation error.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Miscellaneous error not covered by other variants.
    #[error("{0}")]
    Other(String),
}

impl From<crate::pool::PoolError> for DbError {
    fn from(err: crate::pool::PoolError) -> Self {
        DbError::Pool(err.to_string())
    }
}

/// Convenience type alias for `Result<T, DbError>`.
pub type Result<T> = std::result::Result<T, DbError>;
