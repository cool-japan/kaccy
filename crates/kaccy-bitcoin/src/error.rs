//! Bitcoin error types

use bitcoin::Network;
use thiserror::Error;

/// Top-level error type for all kaccy-bitcoin operations.
#[derive(Error, Debug)]
pub enum BitcoinError {
    /// Bitcoin Core RPC communication error
    #[error("RPC error: {0}")]
    Rpc(#[from] bitcoincore_rpc::Error),

    /// Address generation failed
    #[error("Address generation error: {0}")]
    AddressGeneration(String),

    /// Transaction not found in the blockchain
    #[error("Transaction not found: {0}")]
    TransactionNotFound(String),

    /// Transaction does not have enough confirmations
    #[error("Insufficient confirmations: {current}/{required}")]
    InsufficientConfirmations {
        /// Number of confirmations the transaction currently has.
        current: u32,
        /// Minimum number of confirmations required.
        required: u32,
    },

    /// Invalid Bitcoin address format
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// Network mismatch between address and configured network
    #[error(
        "Network mismatch: address is for {address_network:?} but client is configured for {configured_network:?}"
    )]
    NetworkMismatch {
        /// The network encoded in the address (e.g. Testnet).
        address_network: Network,
        /// The network the client is configured for (e.g. Mainnet).
        configured_network: Network,
    },

    /// Connection to Bitcoin Core failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Connection timeout
    #[error("Connection timeout after {timeout_secs} seconds")]
    ConnectionTimeout {
        /// Number of seconds elapsed before the timeout was triggered.
        timeout_secs: u64,
    },

    /// Connection pool exhausted
    #[error("Connection pool exhausted: all connections are in use")]
    ConnectionPoolExhausted,

    /// Wallet operation failed
    #[error("Wallet error: {0}")]
    Wallet(String),

    /// Payment amount mismatch
    #[error("Payment amount mismatch: expected {expected} sats, received {received} sats")]
    PaymentMismatch {
        /// Expected payment amount in satoshis.
        expected: u64,
        /// Actual received amount in satoshis.
        received: u64,
    },

    /// Underpayment detected
    #[error(
        "Underpayment: expected {expected} sats, received {received} sats (short by {shortfall} sats)"
    )]
    Underpayment {
        /// Expected payment amount in satoshis.
        expected: u64,
        /// Actual received amount in satoshis.
        received: u64,
        /// Shortfall amount in satoshis (`expected - received`).
        shortfall: u64,
    },

    /// Overpayment detected
    #[error(
        "Overpayment: expected {expected} sats, received {received} sats (excess {excess} sats)"
    )]
    Overpayment {
        /// Expected payment amount in satoshis.
        expected: u64,
        /// Actual received amount in satoshis.
        received: u64,
        /// Excess amount in satoshis (`received - expected`).
        excess: u64,
    },

    /// Transaction was replaced (RBF)
    #[error("Transaction replaced: original {original_txid}, replacement {replacement_txid}")]
    TransactionReplaced {
        /// Txid of the original transaction that was replaced.
        original_txid: String,
        /// Txid of the RBF replacement transaction.
        replacement_txid: String,
    },

    /// Fee estimation failed
    #[error("Fee estimation failed for target {target_blocks} blocks: {reason}")]
    FeeEstimationFailed {
        /// Confirmation target (number of blocks) used for the fee estimate.
        target_blocks: u32,
        /// Human-readable description of why the estimate failed.
        reason: String,
    },

    /// UTXO not found
    #[error("UTXO not found: {txid}:{vout}")]
    UtxoNotFound {
        /// Transaction ID of the missing UTXO.
        txid: String,
        /// Output index within the transaction.
        vout: u32,
    },

    /// Transaction broadcast failed
    #[error("Transaction broadcast failed: {0}")]
    BroadcastFailed(String),

    /// Block reorganization detected
    #[error(
        "Block reorganization detected at height {height}: expected {expected_hash}, got {actual_hash}"
    )]
    Reorganization {
        /// Block height at which the reorganization was detected.
        height: u64,
        /// Block hash that was expected (previously known tip or ancestor).
        expected_hash: String,
        /// Block hash that was actually observed on the new chain.
        actual_hash: String,
    },

    /// Order not found for payment
    #[error("No order found for payment address: {address}")]
    OrderNotFound {
        /// Bitcoin address for which no matching order was found.
        address: String,
    },

    /// Payment expired
    #[error(
        "Payment expired: order was created at {created_at} and expired after {expiry_hours} hours"
    )]
    PaymentExpired {
        /// ISO-8601 timestamp when the order was created.
        created_at: String,
        /// Configured expiry window in hours.
        expiry_hours: u32,
    },

    /// Invalid transaction format
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),

    /// Mempool rejection
    #[error("Transaction rejected by mempool: {reason}")]
    MempoolRejection {
        /// Rejection reason returned by the node (e.g. "mempool min fee not met").
        reason: String,
    },

    /// Invalid extended public key (xpub/zpub)
    #[error("Invalid xpub: {0}")]
    InvalidXpub(String),

    /// HD wallet derivation failed
    #[error("Derivation failed: {0}")]
    DerivationFailed(String),

    /// Address not found in wallet
    #[error("Address not found in wallet: {0}")]
    AddressNotInWallet(String),

    /// Transaction limit exceeded
    #[error("Transaction limit exceeded: {0}")]
    LimitExceeded(String),

    /// Insufficient funds
    #[error("Insufficient funds: {0}")]
    InsufficientFunds(String),

    /// Generic RPC error (for string-based errors)
    #[error("RPC error: {0}")]
    RpcError(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// PSBT error
    #[error("PSBT error: {0}")]
    Psbt(String),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

impl From<bitcoin::psbt::Error> for BitcoinError {
    fn from(err: bitcoin::psbt::Error) -> Self {
        BitcoinError::Psbt(err.to_string())
    }
}

impl From<bitcoin::address::ParseError> for BitcoinError {
    fn from(err: bitcoin::address::ParseError) -> Self {
        BitcoinError::InvalidAddress(err.to_string())
    }
}

impl From<bitcoin::consensus::encode::Error> for BitcoinError {
    fn from(err: bitcoin::consensus::encode::Error) -> Self {
        BitcoinError::InvalidTransaction(err.to_string())
    }
}

impl BitcoinError {
    /// Check if this error is recoverable (can be retried)
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            BitcoinError::ConnectionFailed(_)
                | BitcoinError::ConnectionTimeout { .. }
                | BitcoinError::InsufficientConfirmations { .. }
        )
    }

    /// Check if this error indicates a payment issue
    pub fn is_payment_error(&self) -> bool {
        matches!(
            self,
            BitcoinError::PaymentMismatch { .. }
                | BitcoinError::Underpayment { .. }
                | BitcoinError::Overpayment { .. }
                | BitcoinError::PaymentExpired { .. }
        )
    }

    /// Create an underpayment error
    pub fn underpayment(expected: u64, received: u64) -> Self {
        BitcoinError::Underpayment {
            expected,
            received,
            shortfall: expected.saturating_sub(received),
        }
    }

    /// Create an overpayment error
    pub fn overpayment(expected: u64, received: u64) -> Self {
        BitcoinError::Overpayment {
            expected,
            received,
            excess: received.saturating_sub(expected),
        }
    }
}

/// Convenience `Result` alias that uses [`BitcoinError`] as the error type.
pub type Result<T> = std::result::Result<T, BitcoinError>;
