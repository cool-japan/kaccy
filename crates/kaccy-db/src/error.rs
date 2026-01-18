//! Database error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Duplicate: {0}")]
    Duplicate(String),

    #[error("Pool error: {0}")]
    Pool(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("{0}")]
    Other(String),
}

impl From<crate::pool::PoolError> for DbError {
    fn from(err: crate::pool::PoolError) -> Self {
        DbError::Pool(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DbError>;
