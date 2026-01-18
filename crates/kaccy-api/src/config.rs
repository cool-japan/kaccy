//! Configuration management for kaccy-api
//!
//! This module provides structured configuration loading, validation, and management.
//! Configuration is loaded from environment variables with sensible defaults.

use std::env;
use thiserror::Error;

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    #[error("Invalid configuration value for {key}: {message}")]
    InvalidValue { key: String, message: String },

    #[error("Failed to parse {key}: {source}")]
    ParseError {
        key: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Security configuration
    pub security: SecurityConfig,

    /// Job processing configuration
    pub jobs: JobsConfig,

    /// Observability configuration
    pub observability: ObservabilityConfig,

    /// Optional integrations
    pub integrations: IntegrationsConfig,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to bind to
    pub port: u16,

    /// Environment (development, staging, production)
    pub environment: Environment,
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,

    /// Maximum number of connections in pool
    pub max_connections: u32,

    /// Minimum number of connections in pool
    pub min_connections: u32,

    /// Connection timeout in seconds
    pub connect_timeout: u64,
}

/// Security configuration
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// JWT secret for token signing
    pub jwt_secret: String,

    /// JWT expiration time in hours
    pub jwt_expiration_hours: i64,
}

/// Job processing configuration
#[derive(Debug, Clone)]
pub struct JobsConfig {
    /// Number of job workers
    pub worker_count: usize,

    /// Maximum queue size
    pub max_queue_size: usize,

    /// Enable job persistence to Redis
    pub enable_persistence: bool,
}

/// Observability configuration
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    /// Log level
    pub log_level: String,

    /// Enable OpenTelemetry
    pub enable_telemetry: bool,

    /// OpenTelemetry endpoint
    pub otel_endpoint: Option<String>,

    /// Enable Prometheus metrics
    pub enable_metrics: bool,
}

/// Optional integrations configuration
#[derive(Debug, Clone)]
pub struct IntegrationsConfig {
    /// Redis connection URL (optional)
    pub redis_url: Option<String>,

    /// Bitcoin RPC URL (optional)
    pub bitcoin_rpc_url: Option<String>,

    /// Bitcoin RPC username (optional)
    pub bitcoin_rpc_user: Option<String>,

    /// Bitcoin RPC password (optional)
    pub bitcoin_rpc_password: Option<String>,

    /// SMTP configuration (optional)
    pub smtp: Option<SmtpConfig>,
}

/// SMTP configuration
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// SMTP host
    pub host: String,

    /// SMTP port
    pub port: u16,

    /// SMTP username
    pub username: String,

    /// SMTP password
    pub password: String,

    /// Enable TLS
    pub enable_tls: bool,

    /// From email address
    pub from_email: String,
}

/// Application environment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl std::str::FromStr for Environment {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(ConfigError::InvalidValue {
                key: "ENVIRONMENT".to_string(),
                message: format!(
                    "Unknown environment: {}. Use development, staging, or production",
                    s
                ),
            }),
        }
    }
}

impl Environment {
    /// Check if this is production
    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    /// Check if this is development
    pub fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Staging => write!(f, "staging"),
            Self::Production => write!(f, "production"),
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if it exists (ignore errors)
        let _ = dotenvy::dotenv();

        Ok(Self {
            server: ServerConfig::from_env()?,
            database: DatabaseConfig::from_env()?,
            security: SecurityConfig::from_env()?,
            jobs: JobsConfig::from_env()?,
            observability: ObservabilityConfig::from_env()?,
            integrations: IntegrationsConfig::from_env()?,
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate database URL format
        if !self.database.url.starts_with("postgres://")
            && !self.database.url.starts_with("postgresql://")
        {
            return Err(ConfigError::InvalidValue {
                key: "DATABASE_URL".to_string(),
                message: "Must be a valid PostgreSQL connection string".to_string(),
            });
        }

        // Validate JWT secret length in production
        if self.server.environment.is_production() && self.security.jwt_secret.len() < 32 {
            return Err(ConfigError::InvalidValue {
                key: "JWT_SECRET".to_string(),
                message: "JWT secret must be at least 32 characters in production".to_string(),
            });
        }

        // Validate worker count
        if self.jobs.worker_count == 0 {
            return Err(ConfigError::InvalidValue {
                key: "JOB_WORKERS".to_string(),
                message: "Worker count must be greater than 0".to_string(),
            });
        }

        Ok(())
    }
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let environment = env::var("ENVIRONMENT")
            .or_else(|_| env::var("ENV"))
            .unwrap_or_else(|_| "development".to_string());

        let environment: Environment = environment.parse()?;

        let host = env::var("HOST").unwrap_or_else(|_| {
            if environment.is_production() {
                "0.0.0.0".to_string()
            } else {
                "127.0.0.1".to_string()
            }
        });

        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        Ok(Self {
            host,
            port,
            environment,
        })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let url = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::MissingRequired("DATABASE_URL".to_string()))?;

        let max_connections = env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let min_connections = env::var("DB_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let connect_timeout = env::var("DB_CONNECT_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Ok(Self {
            url,
            max_connections,
            min_connections,
            connect_timeout,
        })
    }
}

impl SecurityConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = env::var("JWT_SECRET")
            .map_err(|_| ConfigError::MissingRequired("JWT_SECRET".to_string()))?;

        let jwt_expiration_hours = env::var("JWT_EXPIRATION_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24);

        Ok(Self {
            jwt_secret,
            jwt_expiration_hours,
        })
    }
}

impl JobsConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let worker_count = env::var("JOB_WORKERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);

        let max_queue_size = env::var("JOB_MAX_QUEUE_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000);

        let enable_persistence = env::var("JOB_ENABLE_PERSISTENCE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        Ok(Self {
            worker_count,
            max_queue_size,
            enable_persistence,
        })
    }
}

impl ObservabilityConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let log_level = env::var("LOG_LEVEL")
            .or_else(|_| env::var("RUST_LOG"))
            .unwrap_or_else(|_| "info".to_string());

        let otel_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
        let enable_telemetry = otel_endpoint.is_some();

        let enable_metrics = env::var("ENABLE_METRICS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        Ok(Self {
            log_level,
            enable_telemetry,
            otel_endpoint,
            enable_metrics,
        })
    }
}

impl IntegrationsConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let redis_url = env::var("REDIS_URL").ok();
        let bitcoin_rpc_url = env::var("BITCOIN_RPC_URL").ok();
        let bitcoin_rpc_user = env::var("BITCOIN_RPC_USER").ok();
        let bitcoin_rpc_password = env::var("BITCOIN_RPC_PASSWORD").ok();

        let smtp = if env::var("SMTP_HOST").is_ok() {
            Some(SmtpConfig::from_env()?)
        } else {
            None
        };

        Ok(Self {
            redis_url,
            bitcoin_rpc_url,
            bitcoin_rpc_user,
            bitcoin_rpc_password,
            smtp,
        })
    }
}

impl SmtpConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("SMTP_HOST").map_err(|_| {
            ConfigError::MissingRequired("SMTP_HOST (when configuring SMTP)".to_string())
        })?;

        let port = env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);

        let username = env::var("SMTP_USERNAME")
            .map_err(|_| ConfigError::MissingRequired("SMTP_USERNAME".to_string()))?;

        let password = env::var("SMTP_PASSWORD")
            .map_err(|_| ConfigError::MissingRequired("SMTP_PASSWORD".to_string()))?;

        let enable_tls = env::var("SMTP_ENABLE_TLS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        let from_email =
            env::var("SMTP_FROM_EMAIL").unwrap_or_else(|_| format!("noreply@{}", host));

        Ok(Self {
            host,
            port,
            username,
            password,
            enable_tls,
            from_email,
        })
    }
}
