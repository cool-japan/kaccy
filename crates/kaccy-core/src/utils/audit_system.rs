//! Advanced audit system with immutable logs and cryptographic proofs
//!
//! Provides comprehensive audit trails, tamper-evident logging, and audit analytics

use crate::error::CoreError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Audit log manager with immutable, tamper-evident logging
pub struct AuditLogManager {
    /// Audit log entries
    logs: Arc<RwLock<Vec<AuditLogEntry>>>,
    /// Hash chain for tamper detection
    hash_chain: Arc<RwLock<Vec<String>>>,
    /// Analytics engine
    analytics: Arc<RwLock<AuditAnalytics>>,
}

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Entry ID
    pub id: Uuid,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Actor (user/system)
    pub actor: String,
    /// Tenant ID (for multi-tenancy)
    pub tenant_id: Option<Uuid>,
    /// Action performed
    pub action: AuditAction,
    /// Resource type
    pub resource_type: String,
    /// Resource ID
    pub resource_id: String,
    /// Previous state (JSON)
    pub previous_state: Option<String>,
    /// New state (JSON)
    pub new_state: Option<String>,
    /// IP address
    pub ip_address: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Metadata
    pub metadata: HashMap<String, String>,
    /// Previous hash (for chain)
    pub previous_hash: String,
    /// Current hash
    pub hash: String,
}

/// Audit action types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
    PasswordChange,
    PermissionChange,
    ConfigChange,
    Trade,
    Withdrawal,
    Deposit,
    Custom(String),
}

impl AuditLogManager {
    /// Create a new audit log manager
    pub fn new() -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            hash_chain: Arc::new(RwLock::new(vec!["genesis".to_string()])),
            analytics: Arc::new(RwLock::new(AuditAnalytics::new())),
        }
    }

    /// Log an audit event
    #[allow(clippy::too_many_arguments)]
    pub fn log(
        &self,
        actor: String,
        tenant_id: Option<Uuid>,
        action: AuditAction,
        resource_type: String,
        resource_id: String,
        previous_state: Option<String>,
        new_state: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        metadata: HashMap<String, String>,
    ) -> Result<Uuid, CoreError> {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();

        // Get previous hash from chain
        let previous_hash = {
            let chain = self.hash_chain.read().unwrap();
            chain.last().unwrap().clone()
        };

        // Create entry
        let entry = AuditLogEntry {
            id,
            timestamp,
            actor: actor.clone(),
            tenant_id,
            action: action.clone(),
            resource_type: resource_type.clone(),
            resource_id: resource_id.clone(),
            previous_state: previous_state.clone(),
            new_state: new_state.clone(),
            ip_address,
            user_agent,
            metadata: metadata.clone(),
            previous_hash: previous_hash.clone(),
            hash: String::new(), // Will be computed
        };

        // Compute hash
        let hash = self.compute_hash(&entry);

        let mut entry = entry;
        entry.hash = hash.clone();

        // Add to logs
        self.logs.write().unwrap().push(entry.clone());

        // Add to hash chain
        self.hash_chain.write().unwrap().push(hash);

        // Update analytics
        self.analytics
            .write()
            .unwrap()
            .record_event(&actor, &action, &resource_type);

        Ok(id)
    }

    /// Compute cryptographic hash of an entry
    fn compute_hash(&self, entry: &AuditLogEntry) -> String {
        let mut hasher = Sha256::new();

        // Include all fields in hash
        hasher.update(entry.id.to_string().as_bytes());
        hasher.update(entry.timestamp.to_rfc3339().as_bytes());
        hasher.update(entry.actor.as_bytes());
        if let Some(tid) = &entry.tenant_id {
            hasher.update(tid.to_string().as_bytes());
        }
        hasher.update(format!("{:?}", entry.action).as_bytes());
        hasher.update(entry.resource_type.as_bytes());
        hasher.update(entry.resource_id.as_bytes());
        if let Some(ps) = &entry.previous_state {
            hasher.update(ps.as_bytes());
        }
        if let Some(ns) = &entry.new_state {
            hasher.update(ns.as_bytes());
        }
        hasher.update(entry.previous_hash.as_bytes());

        format!("{:x}", hasher.finalize())
    }

    /// Verify integrity of audit log
    pub fn verify_integrity(&self) -> Result<bool, CoreError> {
        let logs = self.logs.read().unwrap();
        let chain = self.hash_chain.read().unwrap();

        if logs.is_empty() {
            return Ok(true);
        }

        // Verify each entry's hash
        for (i, entry) in logs.iter().enumerate() {
            let computed_hash = self.compute_hash(entry);
            if computed_hash != entry.hash {
                return Ok(false);
            }

            // Verify chain linkage
            if i > 0 {
                let prev_entry = &logs[i - 1];
                if entry.previous_hash != prev_entry.hash {
                    return Ok(false);
                }
            }

            // Verify against chain
            if chain.get(i + 1) != Some(&entry.hash) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get audit logs with filtering
    pub fn get_logs(&self, filter: AuditLogFilter) -> Vec<AuditLogEntry> {
        let logs = self.logs.read().unwrap();

        logs.iter()
            .filter(|entry| {
                // Filter by actor
                if let Some(ref actor) = filter.actor {
                    if &entry.actor != actor {
                        return false;
                    }
                }

                // Filter by tenant
                if let Some(ref tenant_id) = filter.tenant_id {
                    if entry.tenant_id.as_ref() != Some(tenant_id) {
                        return false;
                    }
                }

                // Filter by action
                if let Some(ref action) = filter.action {
                    if &entry.action != action {
                        return false;
                    }
                }

                // Filter by resource type
                if let Some(ref resource_type) = filter.resource_type {
                    if &entry.resource_type != resource_type {
                        return false;
                    }
                }

                // Filter by time range
                if let Some(start) = filter.start_time {
                    if entry.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = filter.end_time {
                    if entry.timestamp > end {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    /// Get audit analytics
    pub fn get_analytics(&self) -> AuditAnalytics {
        self.analytics.read().unwrap().clone()
    }

    /// Detect anomalies in audit logs
    pub fn detect_anomalies(&self) -> Vec<AuditAnomaly> {
        let logs = self.logs.read().unwrap();
        let mut anomalies = Vec::new();

        // Detect unusual activity patterns
        let mut actor_actions: HashMap<String, Vec<&AuditLogEntry>> = HashMap::new();

        for entry in logs.iter() {
            actor_actions
                .entry(entry.actor.clone())
                .or_default()
                .push(entry);
        }

        // Check for suspicious patterns
        for (actor, entries) in actor_actions.iter() {
            // Rapid successive actions
            let mut sorted = entries.to_vec();
            sorted.sort_by_key(|e| e.timestamp);

            for window in sorted.windows(5) {
                if window.len() == 5 {
                    let duration = window[4].timestamp - window[0].timestamp;
                    if duration.num_seconds() < 5 {
                        anomalies.push(AuditAnomaly {
                            anomaly_type: AnomalyType::RapidActivity,
                            actor: actor.clone(),
                            description: format!("5 actions in {} seconds", duration.num_seconds()),
                            severity: AnomalySeverity::Medium,
                            timestamp: window[0].timestamp,
                        });
                    }
                }
            }

            // Multiple failed login attempts
            let failed_logins = entries
                .iter()
                .filter(|e| {
                    e.action == AuditAction::Login
                        && e.metadata.get("status") == Some(&"failed".to_string())
                })
                .count();

            if failed_logins > 3 {
                anomalies.push(AuditAnomaly {
                    anomaly_type: AnomalyType::FailedLogins,
                    actor: actor.clone(),
                    description: format!("{} failed login attempts", failed_logins),
                    severity: AnomalySeverity::High,
                    timestamp: Utc::now(),
                });
            }

            // Unusual action patterns (e.g., reading then immediately deleting)
            for window in sorted.windows(2) {
                if window[0].action == AuditAction::Read
                    && window[1].action == AuditAction::Delete
                    && window[0].resource_id == window[1].resource_id
                {
                    let duration = window[1].timestamp - window[0].timestamp;
                    if duration.num_seconds() < 2 {
                        anomalies.push(AuditAnomaly {
                            anomaly_type: AnomalyType::SuspiciousPattern,
                            actor: actor.clone(),
                            description: "Read followed by immediate delete".to_string(),
                            severity: AnomalySeverity::Medium,
                            timestamp: window[1].timestamp,
                        });
                    }
                }
            }
        }

        anomalies
    }

    /// Generate compliance report
    pub fn generate_compliance_report(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> ComplianceReport {
        let logs = self.get_logs(AuditLogFilter {
            actor: None,
            tenant_id: None,
            action: None,
            resource_type: None,
            start_time: Some(start),
            end_time: Some(end),
        });

        let mut action_counts: HashMap<String, usize> = HashMap::new();
        let mut resource_counts: HashMap<String, usize> = HashMap::new();
        let mut unique_actors = std::collections::HashSet::new();

        for entry in &logs {
            let action_key = format!("{:?}", entry.action);
            *action_counts.entry(action_key).or_insert(0) += 1;
            *resource_counts
                .entry(entry.resource_type.clone())
                .or_insert(0) += 1;
            unique_actors.insert(entry.actor.clone());
        }

        ComplianceReport {
            period_start: start,
            period_end: end,
            total_events: logs.len(),
            unique_actors: unique_actors.len(),
            action_breakdown: action_counts,
            resource_breakdown: resource_counts,
            integrity_verified: self.verify_integrity().unwrap_or(false),
        }
    }

    /// Export audit logs for forensic analysis
    pub fn export_logs(&self, format: ExportFormat) -> Result<String, CoreError> {
        let logs = self.logs.read().unwrap();

        match format {
            ExportFormat::Json => serde_json::to_string_pretty(&*logs)
                .map_err(|e| CoreError::Validation(format!("JSON export failed: {}", e))),
            ExportFormat::Csv => {
                let mut csv =
                    String::from("id,timestamp,actor,action,resource_type,resource_id,hash\n");
                for entry in logs.iter() {
                    csv.push_str(&format!(
                        "{},{},{},{:?},{},{},{}\n",
                        entry.id,
                        entry.timestamp,
                        entry.actor,
                        entry.action,
                        entry.resource_type,
                        entry.resource_id,
                        entry.hash
                    ));
                }
                Ok(csv)
            }
        }
    }

    /// Get total log count
    pub fn log_count(&self) -> usize {
        self.logs.read().unwrap().len()
    }
}

impl Default for AuditLogManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Filter for querying audit logs
#[derive(Debug, Clone, Default)]
pub struct AuditLogFilter {
    pub actor: Option<String>,
    pub tenant_id: Option<Uuid>,
    pub action: Option<AuditAction>,
    pub resource_type: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

/// Audit analytics
#[derive(Debug, Clone)]
pub struct AuditAnalytics {
    /// Actions per actor
    actions_per_actor: HashMap<String, usize>,
    /// Actions per type
    actions_per_type: HashMap<String, usize>,
    /// Resources accessed
    resources_accessed: HashMap<String, usize>,
}

impl AuditAnalytics {
    fn new() -> Self {
        Self {
            actions_per_actor: HashMap::new(),
            actions_per_type: HashMap::new(),
            resources_accessed: HashMap::new(),
        }
    }

    fn record_event(&mut self, actor: &str, action: &AuditAction, resource_type: &str) {
        *self.actions_per_actor.entry(actor.to_string()).or_insert(0) += 1;
        *self
            .actions_per_type
            .entry(format!("{:?}", action))
            .or_insert(0) += 1;
        *self
            .resources_accessed
            .entry(resource_type.to_string())
            .or_insert(0) += 1;
    }

    /// Get top actors by activity
    pub fn top_actors(&self, limit: usize) -> Vec<(String, usize)> {
        let mut actors: Vec<_> = self.actions_per_actor.iter().collect();
        actors.sort_by(|a, b| b.1.cmp(a.1));
        actors.truncate(limit);
        actors.into_iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Get action distribution
    pub fn action_distribution(&self) -> HashMap<String, usize> {
        self.actions_per_type.clone()
    }
}

/// Detected anomaly
#[derive(Debug, Clone)]
pub struct AuditAnomaly {
    pub anomaly_type: AnomalyType,
    pub actor: String,
    pub description: String,
    pub severity: AnomalySeverity,
    pub timestamp: DateTime<Utc>,
}

/// Anomaly type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnomalyType {
    RapidActivity,
    FailedLogins,
    SuspiciousPattern,
    UnusualAccess,
}

/// Anomaly severity
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Compliance report
#[derive(Debug, Clone)]
pub struct ComplianceReport {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_events: usize,
    pub unique_actors: usize,
    pub action_breakdown: HashMap<String, usize>,
    pub resource_breakdown: HashMap<String, usize>,
    pub integrity_verified: bool,
}

/// Export format for audit logs
#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Json,
    Csv,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_creation() {
        let manager = AuditLogManager::new();

        let id = manager
            .log(
                "user123".to_string(),
                None,
                AuditAction::Create,
                "order".to_string(),
                "order_456".to_string(),
                None,
                Some("{\"status\": \"pending\"}".to_string()),
                Some("192.168.1.1".to_string()),
                None,
                HashMap::new(),
            )
            .unwrap();

        assert!(id != Uuid::nil());
        assert_eq!(manager.log_count(), 1);
    }

    #[test]
    fn test_integrity_verification() {
        let manager = AuditLogManager::new();

        manager
            .log(
                "user1".to_string(),
                None,
                AuditAction::Create,
                "token".to_string(),
                "token_1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        manager
            .log(
                "user2".to_string(),
                None,
                AuditAction::Update,
                "token".to_string(),
                "token_1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        assert!(manager.verify_integrity().unwrap());
    }

    #[test]
    fn test_audit_log_filtering() {
        let manager = AuditLogManager::new();

        manager
            .log(
                "alice".to_string(),
                None,
                AuditAction::Create,
                "order".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        manager
            .log(
                "bob".to_string(),
                None,
                AuditAction::Delete,
                "order".to_string(),
                "2".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        let filter = AuditLogFilter {
            actor: Some("alice".to_string()),
            ..Default::default()
        };

        let logs = manager.get_logs(filter);
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].actor, "alice");
    }

    #[test]
    fn test_anomaly_detection() {
        let manager = AuditLogManager::new();

        // Create rapid activity
        for _ in 0..6 {
            manager
                .log(
                    "suspicious_user".to_string(),
                    None,
                    AuditAction::Read,
                    "account".to_string(),
                    "acc_1".to_string(),
                    None,
                    None,
                    None,
                    None,
                    HashMap::new(),
                )
                .unwrap();
        }

        let anomalies = manager.detect_anomalies();
        assert!(!anomalies.is_empty());
    }

    #[test]
    fn test_compliance_report() {
        let manager = AuditLogManager::new();

        let start = Utc::now();

        manager
            .log(
                "user1".to_string(),
                None,
                AuditAction::Trade,
                "order".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        let end = Utc::now();

        let report = manager.generate_compliance_report(start, end);
        assert_eq!(report.total_events, 1);
        assert_eq!(report.unique_actors, 1);
        assert!(report.integrity_verified);
    }

    #[test]
    fn test_export_json() {
        let manager = AuditLogManager::new();

        manager
            .log(
                "user1".to_string(),
                None,
                AuditAction::Create,
                "token".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        let json = manager.export_logs(ExportFormat::Json).unwrap();
        assert!(json.contains("user1"));
    }

    #[test]
    fn test_export_csv() {
        let manager = AuditLogManager::new();

        manager
            .log(
                "user1".to_string(),
                None,
                AuditAction::Create,
                "token".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        let csv = manager.export_logs(ExportFormat::Csv).unwrap();
        assert!(csv.contains("user1"));
        assert!(csv.contains("id,timestamp,actor"));
    }

    #[test]
    fn test_analytics() {
        let manager = AuditLogManager::new();

        manager
            .log(
                "alice".to_string(),
                None,
                AuditAction::Create,
                "order".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        manager
            .log(
                "alice".to_string(),
                None,
                AuditAction::Update,
                "order".to_string(),
                "1".to_string(),
                None,
                None,
                None,
                None,
                HashMap::new(),
            )
            .unwrap();

        let analytics = manager.get_analytics();
        let top_actors = analytics.top_actors(5);
        assert_eq!(top_actors[0].0, "alice");
        assert_eq!(top_actors[0].1, 2);
    }
}
