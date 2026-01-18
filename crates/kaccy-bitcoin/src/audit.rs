//! Audit logging for compliance and security
//!
//! This module provides immutable audit trail functionality for tracking
//! all sensitive operations, transactions, and administrative actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{BitcoinError, Result};

/// Audit event severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// Informational event
    Info,
    /// Warning event
    Warning,
    /// Critical event (requires attention)
    Critical,
    /// Security-related event
    Security,
}

/// Types of auditable events
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Transaction created
    TransactionCreated,
    /// Transaction signed
    TransactionSigned,
    /// Transaction broadcasted
    TransactionBroadcasted,
    /// Transaction confirmed
    TransactionConfirmed,
    /// Withdrawal requested
    WithdrawalRequested,
    /// Withdrawal approved
    WithdrawalApproved,
    /// Withdrawal rejected
    WithdrawalRejected,
    /// Withdrawal completed
    WithdrawalCompleted,
    /// Address generated
    AddressGenerated,
    /// Key accessed
    KeyAccessed,
    /// Configuration changed
    ConfigurationChanged,
    /// Admin action
    AdminAction,
    /// Security alert
    SecurityAlert,
    /// Limit exceeded
    LimitExceeded,
    /// Authentication event
    Authentication,
    /// Authorization event
    Authorization,
    /// PSBT created
    PsbtCreated,
    /// PSBT signed
    PsbtSigned,
    /// Multi-sig operation
    MultisigOperation,
    /// Hardware wallet operation
    HardwareWalletOperation,
    /// Custom event
    Custom(String),
}

/// Audit event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: Uuid,
    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: AuditEventType,
    /// Severity level
    pub severity: AuditSeverity,
    /// User or system that triggered the event
    pub actor: String,
    /// Resource affected (e.g., transaction ID, address)
    pub resource: Option<String>,
    /// Action description
    pub action: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// IP address (if applicable)
    pub ip_address: Option<String>,
    /// Session ID (if applicable)
    pub session_id: Option<String>,
    /// Result of the action (success/failure)
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(
        event_type: AuditEventType,
        severity: AuditSeverity,
        actor: String,
        action: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            event_type,
            severity,
            actor,
            resource: None,
            action,
            metadata: HashMap::new(),
            ip_address: None,
            session_id: None,
            success: true,
            error: None,
        }
    }

    /// Set the resource identifier
    pub fn with_resource(mut self, resource: String) -> Self {
        self.resource = Some(resource);
        self
    }

    /// Add metadata field
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Set IP address
    pub fn with_ip_address(mut self, ip: String) -> Self {
        self.ip_address = Some(ip);
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session: String) -> Self {
        self.session_id = Some(session);
        self
    }

    /// Mark as failed with error message
    pub fn with_error(mut self, error: String) -> Self {
        self.success = false;
        self.error = Some(error);
        self
    }
}

/// Audit log storage backend trait
#[allow(dead_code)]
#[async_trait::async_trait]
pub trait AuditStorage: Send + Sync {
    /// Store an audit event
    async fn store(&self, event: &AuditEvent) -> Result<()>;

    /// Query audit events by criteria
    async fn query(&self, criteria: &AuditQueryCriteria) -> Result<Vec<AuditEvent>>;

    /// Get event count
    async fn count(&self, criteria: &AuditQueryCriteria) -> Result<usize>;
}

/// Query criteria for audit events
#[derive(Debug, Clone, Default)]
pub struct AuditQueryCriteria {
    /// Filter by event type
    pub event_type: Option<AuditEventType>,
    /// Filter by severity
    pub severity: Option<AuditSeverity>,
    /// Filter by actor
    pub actor: Option<String>,
    /// Filter by resource
    pub resource: Option<String>,
    /// Filter by time range (start)
    pub time_from: Option<DateTime<Utc>>,
    /// Filter by time range (end)
    pub time_to: Option<DateTime<Utc>>,
    /// Filter by success/failure
    pub success: Option<bool>,
    /// Limit number of results
    pub limit: Option<usize>,
    /// Skip first N results
    pub offset: Option<usize>,
}

/// In-memory audit storage (for testing and development)
#[derive(Clone)]
pub struct InMemoryAuditStorage {
    events: Arc<RwLock<Vec<AuditEvent>>>,
}

impl InMemoryAuditStorage {
    /// Create a new in-memory storage
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get all events (for testing)
    pub async fn all_events(&self) -> Vec<AuditEvent> {
        self.events.read().await.clone()
    }

    /// Clear all events (for testing)
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }
}

impl Default for InMemoryAuditStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuditStorage for InMemoryAuditStorage {
    async fn store(&self, event: &AuditEvent) -> Result<()> {
        self.events.write().await.push(event.clone());
        Ok(())
    }

    async fn query(&self, criteria: &AuditQueryCriteria) -> Result<Vec<AuditEvent>> {
        let events = self.events.read().await;
        let mut filtered: Vec<AuditEvent> = events
            .iter()
            .filter(|e| {
                if let Some(ref event_type) = criteria.event_type {
                    if &e.event_type != event_type {
                        return false;
                    }
                }
                if let Some(ref severity) = criteria.severity {
                    if &e.severity != severity {
                        return false;
                    }
                }
                if let Some(ref actor) = criteria.actor {
                    if &e.actor != actor {
                        return false;
                    }
                }
                if let Some(ref resource) = criteria.resource {
                    if e.resource.as_ref() != Some(resource) {
                        return false;
                    }
                }
                if let Some(time_from) = criteria.time_from {
                    if e.timestamp < time_from {
                        return false;
                    }
                }
                if let Some(time_to) = criteria.time_to {
                    if e.timestamp > time_to {
                        return false;
                    }
                }
                if let Some(success) = criteria.success {
                    if e.success != success {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        // Apply offset
        if let Some(offset) = criteria.offset {
            filtered = filtered.into_iter().skip(offset).collect();
        }

        // Apply limit
        if let Some(limit) = criteria.limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn count(&self, criteria: &AuditQueryCriteria) -> Result<usize> {
        let events = self.query(criteria).await?;
        Ok(events.len())
    }
}

/// File-based audit storage (append-only log file)
pub struct FileAuditStorage {
    file_path: PathBuf,
}

impl FileAuditStorage {
    /// Create a new file-based storage
    pub fn new(file_path: PathBuf) -> Result<Self> {
        // Ensure the directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                BitcoinError::Validation(format!("Failed to create audit log directory: {}", e))
            })?;
        }

        Ok(Self { file_path })
    }
}

#[async_trait::async_trait]
impl AuditStorage for FileAuditStorage {
    async fn store(&self, event: &AuditEvent) -> Result<()> {
        let json = serde_json::to_string(event).map_err(|e| {
            BitcoinError::Validation(format!("Failed to serialize audit event: {}", e))
        })?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| {
                BitcoinError::Validation(format!("Failed to open audit log file: {}", e))
            })?;

        writeln!(file, "{}", json).map_err(|e| {
            BitcoinError::Validation(format!("Failed to write to audit log: {}", e))
        })?;

        Ok(())
    }

    async fn query(&self, criteria: &AuditQueryCriteria) -> Result<Vec<AuditEvent>> {
        use std::io::{BufRead, BufReader};

        let file = std::fs::File::open(&self.file_path).map_err(|e| {
            BitcoinError::Validation(format!("Failed to open audit log file: {}", e))
        })?;

        let reader = BufReader::new(file);
        let mut filtered = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| {
                BitcoinError::Validation(format!("Failed to read audit log line: {}", e))
            })?;

            let event: AuditEvent = serde_json::from_str(&line).map_err(|e| {
                BitcoinError::Validation(format!("Failed to parse audit event: {}", e))
            })?;

            // Apply filters
            if let Some(ref event_type) = criteria.event_type {
                if &event.event_type != event_type {
                    continue;
                }
            }
            if let Some(ref severity) = criteria.severity {
                if &event.severity != severity {
                    continue;
                }
            }
            if let Some(ref actor) = criteria.actor {
                if &event.actor != actor {
                    continue;
                }
            }
            if let Some(ref resource) = criteria.resource {
                if event.resource.as_ref() != Some(resource) {
                    continue;
                }
            }
            if let Some(time_from) = criteria.time_from {
                if event.timestamp < time_from {
                    continue;
                }
            }
            if let Some(time_to) = criteria.time_to {
                if event.timestamp > time_to {
                    continue;
                }
            }
            if let Some(success) = criteria.success {
                if event.success != success {
                    continue;
                }
            }

            filtered.push(event);
        }

        // Apply offset
        if let Some(offset) = criteria.offset {
            filtered = filtered.into_iter().skip(offset).collect();
        }

        // Apply limit
        if let Some(limit) = criteria.limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    async fn count(&self, criteria: &AuditQueryCriteria) -> Result<usize> {
        let events = self.query(criteria).await?;
        Ok(events.len())
    }
}

/// Audit logger for tracking operations
pub struct AuditLogger<S: AuditStorage> {
    storage: Arc<S>,
    enabled: bool,
}

impl<S: AuditStorage> AuditLogger<S> {
    /// Create a new audit logger
    pub fn new(storage: S) -> Self {
        Self {
            storage: Arc::new(storage),
            enabled: true,
        }
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        tracing::info!(
            event_id = %event.id,
            event_type = ?event.event_type,
            severity = ?event.severity,
            actor = %event.actor,
            "Audit event logged"
        );

        self.storage.store(&event).await
    }

    /// Query audit events
    pub async fn query(&self, criteria: &AuditQueryCriteria) -> Result<Vec<AuditEvent>> {
        self.storage.query(criteria).await
    }

    /// Get event count
    pub async fn count(&self, criteria: &AuditQueryCriteria) -> Result<usize> {
        self.storage.count(criteria).await
    }

    /// Enable or disable logging
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Builder for creating audit events with fluent API
pub struct AuditEventBuilder {
    event: AuditEvent,
}

impl AuditEventBuilder {
    /// Start building a new audit event
    pub fn new(event_type: AuditEventType, actor: String, action: String) -> Self {
        Self {
            event: AuditEvent::new(event_type, AuditSeverity::Info, actor, action),
        }
    }

    /// Set severity
    pub fn severity(mut self, severity: AuditSeverity) -> Self {
        self.event.severity = severity;
        self
    }

    /// Set resource
    pub fn resource(mut self, resource: String) -> Self {
        self.event.resource = Some(resource);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: String, value: String) -> Self {
        self.event.metadata.insert(key, value);
        self
    }

    /// Set IP address
    pub fn ip_address(mut self, ip: String) -> Self {
        self.event.ip_address = Some(ip);
        self
    }

    /// Set session ID
    pub fn session_id(mut self, session: String) -> Self {
        self.event.session_id = Some(session);
        self
    }

    /// Mark as failed
    pub fn failed(mut self, error: String) -> Self {
        self.event.success = false;
        self.event.error = Some(error);
        self
    }

    /// Build the audit event
    pub fn build(self) -> AuditEvent {
        self.event
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            AuditEventType::TransactionCreated,
            AuditSeverity::Info,
            "system".to_string(),
            "Created new transaction".to_string(),
        );

        assert_eq!(event.event_type, AuditEventType::TransactionCreated);
        assert_eq!(event.severity, AuditSeverity::Info);
        assert_eq!(event.actor, "system");
        assert!(event.success);
        assert!(event.error.is_none());
    }

    #[test]
    fn test_audit_event_builder() {
        let event = AuditEventBuilder::new(
            AuditEventType::WithdrawalRequested,
            "user123".to_string(),
            "Requested withdrawal of 1 BTC".to_string(),
        )
        .severity(AuditSeverity::Critical)
        .resource("tx_abc123".to_string())
        .metadata("amount".to_string(), "100000000".to_string())
        .build();

        assert_eq!(event.severity, AuditSeverity::Critical);
        assert_eq!(event.resource, Some("tx_abc123".to_string()));
        assert_eq!(event.metadata.get("amount"), Some(&"100000000".to_string()));
    }

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryAuditStorage::new();
        let event = AuditEvent::new(
            AuditEventType::TransactionCreated,
            AuditSeverity::Info,
            "test".to_string(),
            "test action".to_string(),
        );

        storage.store(&event).await.unwrap();

        let events = storage.all_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].actor, "test");
    }

    #[tokio::test]
    async fn test_audit_query() {
        let storage = InMemoryAuditStorage::new();

        // Store multiple events
        for i in 0..5 {
            let event = AuditEvent::new(
                AuditEventType::TransactionCreated,
                AuditSeverity::Info,
                format!("user{}", i),
                "test".to_string(),
            );
            storage.store(&event).await.unwrap();
        }

        // Query with actor filter
        let criteria = AuditQueryCriteria {
            actor: Some("user2".to_string()),
            ..Default::default()
        };

        let results = storage.query(&criteria).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].actor, "user2");
    }

    #[tokio::test]
    async fn test_audit_logger() {
        let storage = InMemoryAuditStorage::new();
        let logger = AuditLogger::new(storage.clone());

        let event = AuditEvent::new(
            AuditEventType::SecurityAlert,
            AuditSeverity::Security,
            "system".to_string(),
            "Suspicious activity detected".to_string(),
        );

        logger.log(event).await.unwrap();

        let events = storage.all_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].severity, AuditSeverity::Security);
    }
}
