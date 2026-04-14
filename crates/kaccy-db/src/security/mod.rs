/// Input sanitization and PII redaction
pub mod sanitization;

/// Encryption at rest support
pub mod encryption;

/// Enhanced audit logging with GDPR/CCPA compliance
pub mod audit_logging;

pub use sanitization::{DataMasker, ParameterSanitizer, PiiRedactor, SanitizationError};

pub use encryption::{
    EncryptedData, EncryptionError, EncryptionProvider, KeyRotationManager, PasswordHashingService,
};

pub use audit_logging::{
    AuditAction, AuditContext, AuditLogEntry, AuditLogger, ComplianceManager, ComplianceReport,
};
