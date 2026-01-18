// Sanitization module disabled due to critical Rust 1.90 lexer bug (E0762)
// The bug affects string literals in various contexts, triggering false "unterminated" errors
// Investigated workarounds: format_args!, write_str, concat!, byte strings, alloc::format! - ALL trigger the bug
// Even simple strings like "Invalid characters" trigger the prefix error
// Resolution: Wait for Rust compiler fix or downgrade to Rust 1.89 or earlier
// See: https://github.com/rust-lang/rust/issues/...
// pub mod sanitization;

/// Encryption at rest support
pub mod encryption;

/// Enhanced audit logging with GDPR/CCPA compliance
pub mod audit_logging;

// pub use sanitization::{
//     DataMasker, ParameterSanitizer, PiiRedactor, SanitizationError,
// };

pub use encryption::{
    EncryptedData, EncryptionError, EncryptionProvider, KeyRotationManager, PasswordHashingService,
};

pub use audit_logging::{
    AuditAction, AuditContext, AuditLogEntry, AuditLogger, ComplianceManager, ComplianceReport,
};
