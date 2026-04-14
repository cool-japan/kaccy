//! Core error types

use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

/// Core error type for all kaccy-core operations
#[derive(Error, Debug)]
pub enum CoreError {
    /// A database operation failed
    #[error("Database error: {0}")]
    Database(String),

    /// Input validation failed
    #[error("Validation error: {0}")]
    Validation(String),

    /// Requested resource was not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Account balance is insufficient for the operation
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        /// Required amount
        required: Decimal,
        /// Currently available amount
        available: Decimal,
    },

    /// A token with this identifier already exists
    #[error("Token already exists: {0}")]
    TokenExists(String),

    /// Bonding curve parameters are invalid
    #[error("Invalid bonding curve parameters")]
    InvalidCurveParams,

    /// A circuit breaker has been triggered for the token
    #[error("Circuit breaker triggered")]
    CircuitBreakerTriggered,

    /// Trade slippage exceeded the configured tolerance
    #[error("Slippage exceeded: expected {expected}, actual {actual}")]
    SlippageExceeded {
        /// Expected slippage
        expected: Decimal,
        /// Actual slippage
        actual: Decimal,
    },

    /// Order has expired and can no longer be filled
    #[error("Order expired")]
    OrderExpired,

    /// Token trading is currently paused
    #[error("Token paused")]
    TokenPaused,

    /// User reputation is below the required threshold
    #[error("Reputation too low: required {required}, current {current}")]
    ReputationTooLow {
        /// Required reputation score
        required: Decimal,
        /// Current reputation score
        current: Decimal,
    },

    /// Token maximum supply has been reached
    #[error("Max supply reached")]
    MaxSupplyReached,

    /// Caller is not authorised to perform this action
    #[error("Unauthorized")]
    Unauthorized,

    /// Serialization or deserialization failed
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// A numeric calculation failed
    #[error("Calculation error: {0}")]
    Calculation(String),

    // Trading-specific errors
    /// The specified order was not found
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    /// Order quantity is invalid
    #[error("Invalid order quantity: {0}")]
    InvalidOrderQuantity(String),

    /// Order price is invalid
    #[error("Invalid price: {0}")]
    InvalidPrice(String),

    /// Order has already been filled
    #[error("Order already filled")]
    OrderAlreadyFilled,

    /// Order has already been cancelled
    #[error("Order already cancelled")]
    OrderAlreadyCancelled,

    /// Insufficient market liquidity for the trade
    #[error("Insufficient liquidity: {0}")]
    InsufficientLiquidity(String),

    /// Trade price impact exceeds the configured maximum
    #[error("Price impact too high: {impact}% exceeds maximum {max_impact}%")]
    PriceImpactTooHigh {
        /// Actual price impact
        impact: Decimal,
        /// Configured maximum allowed impact
        max_impact: Decimal,
    },

    /// Market is outside of its trading hours
    #[error("Market is closed")]
    MarketClosed,

    /// Trading for this token has been halted
    #[error("Trading halted for token {0}")]
    TradingHalted(Uuid),

    // Risk management errors
    /// Position size exceeds the configured limit
    #[error("Position limit exceeded: current {current}, maximum {maximum}")]
    PositionLimitExceeded {
        /// Current position size
        current: Decimal,
        /// Configured maximum
        maximum: Decimal,
    },

    /// Daily trading volume limit exceeded
    #[error("Daily trading limit exceeded: traded {traded}, limit {limit}")]
    DailyTradingLimitExceeded {
        /// Volume traded today
        traded: Decimal,
        /// Configured daily limit
        limit: Decimal,
    },

    /// Leverage ratio is too high
    #[error("Leverage ratio too high: {ratio} exceeds maximum {max_ratio}")]
    ExcessiveLeverage {
        /// Current leverage ratio
        ratio: Decimal,
        /// Configured maximum ratio
        max_ratio: Decimal,
    },

    /// Margin call: equity has fallen below maintenance margin
    #[error("Margin call: equity {equity} below maintenance margin {maintenance}")]
    MarginCall {
        /// Current equity
        equity: Decimal,
        /// Required maintenance margin
        maintenance: Decimal,
    },

    // Payment-specific errors
    /// The specified payment was not found
    #[error("Payment not found: {0}")]
    PaymentNotFound(Uuid),

    /// Payment has already been confirmed and cannot be reprocessed
    #[error("Payment already confirmed")]
    PaymentAlreadyConfirmed,

    /// Payment has expired
    #[error("Payment expired")]
    PaymentExpired,

    /// Not enough blockchain confirmations yet
    #[error("Insufficient confirmations: required {required}, current {current}")]
    InsufficientConfirmations {
        /// Required confirmation count
        required: u32,
        /// Current confirmation count
        current: u32,
    },

    // Reputation/KYC errors
    /// KYC verification is required before this action
    #[error("KYC verification required")]
    KycRequired,

    /// KYC verification is in progress
    #[error("KYC verification pending")]
    KycPending,

    /// KYC verification was rejected
    #[error("KYC verification rejected: {0}")]
    KycRejected(String),

    /// The specified commitment was not found
    #[error("Commitment not found: {0}")]
    CommitmentNotFound(Uuid),

    /// Commitment has already been verified
    #[error("Commitment already verified")]
    CommitmentAlreadyVerified,

    /// Commitment deadline has passed
    #[error("Commitment deadline passed")]
    CommitmentDeadlinePassed,

    // Rate limiting errors
    /// Rate limit exceeded; retry after the given number of seconds
    #[error("Rate limit exceeded: retry after {retry_after} seconds")]
    RateLimitExceeded {
        /// Seconds to wait before retrying
        retry_after: u64,
    },

    /// Too many requests from this user
    #[error("Too many requests from user {0}")]
    TooManyRequests(Uuid),

    // Concurrency errors
    /// A resource is currently locked by another operation
    #[error("Resource locked: {0}")]
    ResourceLocked(String),

    /// Optimistic locking failed because the resource was concurrently modified
    #[error("Optimistic lock failed: resource was modified")]
    OptimisticLockFailed,

    /// A database deadlock was detected
    #[error("Deadlock detected")]
    DeadlockDetected,

    // Configuration errors
    /// System configuration is invalid
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// A feature flag is not enabled
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    // State errors
    /// Entity is in an invalid state for this operation
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Entity already exists
    #[error("Already exists: {0}")]
    AlreadyExists(String),

    // Bridge-specific errors
    /// Provided amount is invalid
    #[error("Invalid amount")]
    InvalidAmount,

    /// Specified bridge route is invalid
    #[error("Invalid bridge route")]
    InvalidBridgeRoute,

    /// Bridge protocol is not supported
    #[error("Bridge not supported")]
    BridgeNotSupported,
}

impl From<sqlx::Error> for CoreError {
    fn from(err: sqlx::Error) -> Self {
        CoreError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        CoreError::Serialization(err.to_string())
    }
}

impl CoreError {
    /// Check if error is retryable (e.g., for retry logic)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CoreError::Database(_)
                | CoreError::DeadlockDetected
                | CoreError::ResourceLocked(_)
                | CoreError::OptimisticLockFailed
        )
    }

    /// Check if error is a client error (4xx)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            CoreError::Validation(_)
                | CoreError::NotFound(_)
                | CoreError::Unauthorized
                | CoreError::OrderNotFound(_)
                | CoreError::InvalidOrderQuantity(_)
                | CoreError::InvalidPrice(_)
                | CoreError::OrderExpired
                | CoreError::OrderAlreadyFilled
                | CoreError::OrderAlreadyCancelled
                | CoreError::InsufficientBalance { .. }
                | CoreError::TokenExists(_)
                | CoreError::TokenPaused
                | CoreError::ReputationTooLow { .. }
                | CoreError::MaxSupplyReached
                | CoreError::SlippageExceeded { .. }
                | CoreError::InsufficientLiquidity { .. }
                | CoreError::PriceImpactTooHigh { .. }
                | CoreError::PositionLimitExceeded { .. }
                | CoreError::DailyTradingLimitExceeded { .. }
                | CoreError::ExcessiveLeverage { .. }
                | CoreError::PaymentNotFound(_)
                | CoreError::PaymentExpired
                | CoreError::InsufficientConfirmations { .. }
                | CoreError::KycRequired
                | CoreError::KycPending
                | CoreError::KycRejected(_)
                | CoreError::CommitmentNotFound(_)
                | CoreError::CommitmentDeadlinePassed
                | CoreError::RateLimitExceeded { .. }
                | CoreError::TooManyRequests(_)
                | CoreError::FeatureNotEnabled(_)
                | CoreError::InvalidState(_)
                | CoreError::InvalidAmount
                | CoreError::InvalidBridgeRoute
                | CoreError::BridgeNotSupported
        )
    }

    /// Check if error is a server error (5xx)
    pub fn is_server_error(&self) -> bool {
        matches!(
            self,
            CoreError::Database(_)
                | CoreError::Serialization(_)
                | CoreError::Calculation(_)
                | CoreError::DeadlockDetected
                | CoreError::ResourceLocked(_)
                | CoreError::OptimisticLockFailed
                | CoreError::Configuration(_)
        )
    }

    /// Get HTTP status code for this error
    pub fn status_code(&self) -> u16 {
        match self {
            CoreError::NotFound(_)
            | CoreError::OrderNotFound(_)
            | CoreError::PaymentNotFound(_)
            | CoreError::CommitmentNotFound(_) => 404,

            CoreError::Unauthorized => 401,

            CoreError::Validation(_)
            | CoreError::InvalidOrderQuantity(_)
            | CoreError::InvalidPrice(_)
            | CoreError::OrderExpired
            | CoreError::OrderAlreadyFilled
            | CoreError::OrderAlreadyCancelled
            | CoreError::InsufficientBalance { .. }
            | CoreError::TokenExists(_)
            | CoreError::InvalidCurveParams
            | CoreError::TokenPaused
            | CoreError::ReputationTooLow { .. }
            | CoreError::MaxSupplyReached
            | CoreError::SlippageExceeded { .. }
            | CoreError::InsufficientLiquidity(_)
            | CoreError::PriceImpactTooHigh { .. }
            | CoreError::MarketClosed
            | CoreError::TradingHalted(_)
            | CoreError::PositionLimitExceeded { .. }
            | CoreError::DailyTradingLimitExceeded { .. }
            | CoreError::ExcessiveLeverage { .. }
            | CoreError::MarginCall { .. }
            | CoreError::PaymentExpired
            | CoreError::InsufficientConfirmations { .. }
            | CoreError::KycRequired
            | CoreError::KycRejected(_)
            | CoreError::CommitmentDeadlinePassed
            | CoreError::FeatureNotEnabled(_)
            | CoreError::InvalidState(_)
            | CoreError::InvalidAmount
            | CoreError::InvalidBridgeRoute
            | CoreError::BridgeNotSupported => 400,

            CoreError::PaymentAlreadyConfirmed
            | CoreError::CommitmentAlreadyVerified
            | CoreError::KycPending
            | CoreError::AlreadyExists(_) => 409,

            CoreError::RateLimitExceeded { .. } | CoreError::TooManyRequests(_) => 429,

            CoreError::CircuitBreakerTriggered => 503,

            CoreError::Database(_)
            | CoreError::Serialization(_)
            | CoreError::Calculation(_)
            | CoreError::ResourceLocked(_)
            | CoreError::OptimisticLockFailed
            | CoreError::DeadlockDetected
            | CoreError::Configuration(_) => 500,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            CoreError::Database(_) => "database",
            CoreError::Validation(_)
            | CoreError::InvalidOrderQuantity(_)
            | CoreError::InvalidPrice(_) => "validation",
            CoreError::NotFound(_)
            | CoreError::OrderNotFound(_)
            | CoreError::PaymentNotFound(_)
            | CoreError::CommitmentNotFound(_) => "not_found",
            CoreError::Unauthorized => "authorization",
            CoreError::InsufficientBalance { .. } | CoreError::InsufficientLiquidity(_) => {
                "insufficient_funds"
            }
            CoreError::OrderExpired
            | CoreError::OrderAlreadyFilled
            | CoreError::OrderAlreadyCancelled => "order_state",
            CoreError::TokenExists(_) | CoreError::TokenPaused | CoreError::TradingHalted(_) => {
                "token_state"
            }
            CoreError::InvalidCurveParams => "bonding_curve",
            CoreError::CircuitBreakerTriggered => "circuit_breaker",
            CoreError::SlippageExceeded { .. } | CoreError::PriceImpactTooHigh { .. } => {
                "price_protection"
            }
            CoreError::ReputationTooLow { .. } => "reputation",
            CoreError::MaxSupplyReached => "supply_limit",
            CoreError::MarketClosed => "market_hours",
            CoreError::PositionLimitExceeded { .. }
            | CoreError::DailyTradingLimitExceeded { .. }
            | CoreError::ExcessiveLeverage { .. }
            | CoreError::MarginCall { .. } => "risk_management",
            CoreError::PaymentAlreadyConfirmed
            | CoreError::PaymentExpired
            | CoreError::InsufficientConfirmations { .. } => "payment",
            CoreError::KycRequired | CoreError::KycPending | CoreError::KycRejected(_) => "kyc",
            CoreError::CommitmentAlreadyVerified | CoreError::CommitmentDeadlinePassed => {
                "commitment"
            }
            CoreError::RateLimitExceeded { .. } | CoreError::TooManyRequests(_) => "rate_limiting",
            CoreError::ResourceLocked(_)
            | CoreError::OptimisticLockFailed
            | CoreError::DeadlockDetected => "concurrency",
            CoreError::Configuration(_) | CoreError::FeatureNotEnabled(_) => "configuration",
            CoreError::Serialization(_) => "serialization",
            CoreError::Calculation(_) => "calculation",
            CoreError::InvalidState(_) => "state",
            CoreError::AlreadyExists(_) => "duplicate",
            CoreError::InvalidAmount
            | CoreError::InvalidBridgeRoute
            | CoreError::BridgeNotSupported => "bridge",
        }
    }
}

/// Convenience type alias for results that may produce a [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_error_retryable() {
        let retryable_error = CoreError::Database("Connection failed".to_string());
        assert!(retryable_error.is_retryable());

        let non_retryable_error = CoreError::Validation("Invalid input".to_string());
        assert!(!non_retryable_error.is_retryable());
    }

    #[test]
    fn test_error_categorization() {
        let client_error = CoreError::Validation("Invalid input".to_string());
        assert!(client_error.is_client_error());
        assert!(!client_error.is_server_error());

        let server_error = CoreError::Database("Connection failed".to_string());
        assert!(!server_error.is_client_error());
        assert!(server_error.is_server_error());
    }

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            CoreError::NotFound("Resource".to_string()).status_code(),
            404
        );
        assert_eq!(CoreError::Unauthorized.status_code(), 401);
        assert_eq!(
            CoreError::Validation("Invalid".to_string()).status_code(),
            400
        );
        assert_eq!(CoreError::Database("Error".to_string()).status_code(), 500);
        assert_eq!(
            CoreError::RateLimitExceeded { retry_after: 60 }.status_code(),
            429
        );
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(
            CoreError::Database("Error".to_string()).category(),
            "database"
        );
        assert_eq!(
            CoreError::Validation("Error".to_string()).category(),
            "validation"
        );
        assert_eq!(
            CoreError::InsufficientBalance {
                required: dec!(100),
                available: dec!(50)
            }
            .category(),
            "insufficient_funds"
        );
        assert_eq!(
            CoreError::CircuitBreakerTriggered.category(),
            "circuit_breaker"
        );
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_err.is_err());

        let core_err: CoreError = json_err.unwrap_err().into();
        matches!(core_err, CoreError::Serialization(_));
    }
}
