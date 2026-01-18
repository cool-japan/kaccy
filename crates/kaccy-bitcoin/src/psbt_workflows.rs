//! PSBT workflow manager for multi-signer coordination
//!
//! Provides tools for managing complex PSBT signing workflows with multiple signers.

use crate::error::BitcoinError;
use bitcoin::psbt::Psbt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// PSBT version
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsbtVersion {
    /// PSBT version 0 (BIP 174)
    V0,
    /// PSBT version 2 (BIP 370)
    V2,
}

/// Signer role in the workflow
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignerRole {
    /// Signer identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Required to sign
    pub required: bool,
}

/// Signing status for a signer
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigningStatus {
    /// Awaiting signature
    Pending,
    /// Signature provided
    Signed,
    /// Signature rejected
    Rejected,
    /// Timeout
    Timeout,
}

/// Signature record for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureRecord {
    /// Signer role
    pub signer: SignerRole,
    /// Signing status
    pub status: SigningStatus,
    /// When the signature was requested
    pub requested_at: chrono::DateTime<chrono::Utc>,
    /// When the signature was provided (if signed)
    pub signed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Additional notes
    pub notes: Option<String>,
}

/// PSBT workflow state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowState {
    /// Initial state - PSBT created
    Created,
    /// Collecting signatures
    Collecting,
    /// All required signatures collected
    Complete,
    /// Workflow cancelled
    Cancelled,
    /// Workflow expired
    Expired,
}

/// PSBT workflow for coordinating multi-signer transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtWorkflow {
    /// Workflow ID
    pub id: String,
    /// PSBT data (base64)
    pub psbt_base64: String,
    /// PSBT version
    pub version: PsbtVersion,
    /// Current workflow state
    pub state: WorkflowState,
    /// Signature records
    pub signatures: Vec<SignatureRecord>,
    /// Required signers
    pub required_signers: HashSet<String>,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Expiration timestamp
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl PsbtWorkflow {
    /// Create a new PSBT workflow
    pub fn new(
        id: String,
        psbt_base64: String,
        signers: Vec<SignerRole>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        let now = chrono::Utc::now();
        let required_signers: HashSet<String> = signers
            .iter()
            .filter(|s| s.required)
            .map(|s| s.id.clone())
            .collect();

        let signatures = signers
            .into_iter()
            .map(|signer| SignatureRecord {
                signer,
                status: SigningStatus::Pending,
                requested_at: now,
                signed_at: None,
                notes: None,
            })
            .collect();

        Self {
            id,
            psbt_base64,
            version: PsbtVersion::V0,
            state: WorkflowState::Created,
            signatures,
            required_signers,
            created_at: now,
            expires_at,
            metadata: HashMap::new(),
        }
    }

    /// Record a signature from a signer
    pub fn record_signature(
        &mut self,
        signer_id: &str,
        psbt_base64: String,
    ) -> Result<(), BitcoinError> {
        if self.state == WorkflowState::Complete || self.state == WorkflowState::Cancelled {
            return Err(BitcoinError::InvalidTransaction(
                "Workflow is already completed or cancelled".to_string(),
            ));
        }

        // Check if expired
        if let Some(expires_at) = self.expires_at {
            if chrono::Utc::now() > expires_at {
                self.state = WorkflowState::Expired;
                return Err(BitcoinError::InvalidTransaction(
                    "Workflow has expired".to_string(),
                ));
            }
        }

        // Find the signer
        let signature = self
            .signatures
            .iter_mut()
            .find(|s| s.signer.id == signer_id)
            .ok_or_else(|| BitcoinError::InvalidTransaction("Signer not found".to_string()))?;

        if signature.status == SigningStatus::Signed {
            return Err(BitcoinError::InvalidTransaction(
                "Signer has already signed".to_string(),
            ));
        }

        // Update signature record
        signature.status = SigningStatus::Signed;
        signature.signed_at = Some(chrono::Utc::now());

        // Update PSBT
        self.psbt_base64 = psbt_base64;

        // Update state
        self.state = WorkflowState::Collecting;

        // Check if all required signatures are collected
        if self.is_complete() {
            self.state = WorkflowState::Complete;
        }

        Ok(())
    }

    /// Check if all required signatures are collected
    pub fn is_complete(&self) -> bool {
        self.required_signers.iter().all(|id| {
            self.signatures
                .iter()
                .any(|s| s.signer.id == *id && s.status == SigningStatus::Signed)
        })
    }

    /// Get pending signers
    pub fn pending_signers(&self) -> Vec<&SignerRole> {
        self.signatures
            .iter()
            .filter(|s| s.status == SigningStatus::Pending)
            .map(|s| &s.signer)
            .collect()
    }

    /// Cancel the workflow
    pub fn cancel(&mut self) {
        self.state = WorkflowState::Cancelled;
    }

    /// Check if workflow is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Get the current PSBT
    pub fn get_psbt(&self) -> Result<Psbt, BitcoinError> {
        // Decode from base64
        use base64::Engine;
        let psbt_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.psbt_base64)
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Invalid base64: {}", e)))?;

        Psbt::deserialize(&psbt_bytes)
            .map_err(|e| BitcoinError::InvalidTransaction(format!("Invalid PSBT: {}", e)))
    }
}

/// PSBT workflow manager
pub struct PsbtWorkflowManager {
    workflows: HashMap<String, PsbtWorkflow>,
}

impl PsbtWorkflowManager {
    /// Create a new workflow manager
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }

    /// Create a new workflow
    pub fn create_workflow(
        &mut self,
        psbt_base64: String,
        signers: Vec<SignerRole>,
        expires_in_hours: Option<i64>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let expires_at =
            expires_in_hours.map(|hours| chrono::Utc::now() + chrono::Duration::hours(hours));

        let workflow = PsbtWorkflow::new(id.clone(), psbt_base64, signers, expires_at);
        self.workflows.insert(id.clone(), workflow);

        id
    }

    /// Get a workflow by ID
    pub fn get_workflow(&self, id: &str) -> Option<&PsbtWorkflow> {
        self.workflows.get(id)
    }

    /// Get a mutable workflow by ID
    pub fn get_workflow_mut(&mut self, id: &str) -> Option<&mut PsbtWorkflow> {
        self.workflows.get_mut(id)
    }

    /// Record a signature
    pub fn record_signature(
        &mut self,
        workflow_id: &str,
        signer_id: &str,
        psbt_base64: String,
    ) -> Result<(), BitcoinError> {
        let workflow = self
            .get_workflow_mut(workflow_id)
            .ok_or_else(|| BitcoinError::InvalidTransaction("Workflow not found".to_string()))?;

        workflow.record_signature(signer_id, psbt_base64)
    }

    /// Cancel a workflow
    pub fn cancel_workflow(&mut self, id: &str) -> Result<(), BitcoinError> {
        let workflow = self
            .get_workflow_mut(id)
            .ok_or_else(|| BitcoinError::InvalidTransaction("Workflow not found".to_string()))?;

        workflow.cancel();
        Ok(())
    }

    /// List all workflows
    pub fn list_workflows(&self) -> Vec<&PsbtWorkflow> {
        self.workflows.values().collect()
    }

    /// List workflows by state
    pub fn list_workflows_by_state(&self, state: WorkflowState) -> Vec<&PsbtWorkflow> {
        self.workflows
            .values()
            .filter(|w| w.state == state)
            .collect()
    }

    /// Clean up expired workflows
    pub fn cleanup_expired(&mut self) -> usize {
        let expired_ids: Vec<String> = self
            .workflows
            .iter()
            .filter(|(_, w)| w.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired_ids.len();

        for id in expired_ids {
            if let Some(workflow) = self.workflows.get_mut(&id) {
                workflow.state = WorkflowState::Expired;
            }
        }

        count
    }
}

impl Default for PsbtWorkflowManager {
    fn default() -> Self {
        Self::new()
    }
}

/// PSBT template for common transaction patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsbtTemplate {
    /// Template name
    pub name: String,
    /// Template description
    pub description: String,
    /// Number of inputs
    pub num_inputs: usize,
    /// Number of outputs
    pub num_outputs: usize,
    /// Required signers configuration
    pub signers: Vec<SignerRole>,
}

impl PsbtTemplate {
    /// Create a 2-of-3 multisig template
    pub fn multisig_2_of_3() -> Self {
        Self {
            name: "2-of-3 Multisig".to_string(),
            description: "Standard 2-of-3 multisig transaction".to_string(),
            num_inputs: 1,
            num_outputs: 2,
            signers: vec![
                SignerRole {
                    id: "signer_1".to_string(),
                    name: "Primary Signer".to_string(),
                    required: true,
                },
                SignerRole {
                    id: "signer_2".to_string(),
                    name: "Secondary Signer".to_string(),
                    required: true,
                },
                SignerRole {
                    id: "signer_3".to_string(),
                    name: "Backup Signer".to_string(),
                    required: false,
                },
            ],
        }
    }

    /// Create a single-sig with co-signer template
    pub fn single_with_cosigner() -> Self {
        Self {
            name: "Single + Co-signer".to_string(),
            description: "Primary signer with optional co-signer approval".to_string(),
            num_inputs: 1,
            num_outputs: 2,
            signers: vec![
                SignerRole {
                    id: "primary".to_string(),
                    name: "Primary Signer".to_string(),
                    required: true,
                },
                SignerRole {
                    id: "cosigner".to_string(),
                    name: "Co-signer".to_string(),
                    required: false,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psbt_workflow_creation() {
        let signers = vec![
            SignerRole {
                id: "signer1".to_string(),
                name: "Signer 1".to_string(),
                required: true,
            },
            SignerRole {
                id: "signer2".to_string(),
                name: "Signer 2".to_string(),
                required: true,
            },
        ];

        let workflow = PsbtWorkflow::new(
            "test_workflow".to_string(),
            "test_psbt".to_string(),
            signers,
            None,
        );

        assert_eq!(workflow.state, WorkflowState::Created);
        assert_eq!(workflow.signatures.len(), 2);
        assert!(!workflow.is_complete());
    }

    #[test]
    fn test_workflow_signature_recording() {
        let signers = vec![SignerRole {
            id: "signer1".to_string(),
            name: "Signer 1".to_string(),
            required: true,
        }];

        let mut workflow = PsbtWorkflow::new(
            "test_workflow".to_string(),
            "test_psbt".to_string(),
            signers,
            None,
        );

        let result = workflow.record_signature("signer1", "signed_psbt".to_string());
        assert!(result.is_ok());
        assert_eq!(workflow.state, WorkflowState::Complete);
        assert!(workflow.is_complete());
    }

    #[test]
    fn test_workflow_manager() {
        let mut manager = PsbtWorkflowManager::new();

        let signers = vec![SignerRole {
            id: "signer1".to_string(),
            name: "Signer 1".to_string(),
            required: true,
        }];

        let workflow_id = manager.create_workflow("test_psbt".to_string(), signers, Some(24));

        assert!(manager.get_workflow(&workflow_id).is_some());

        let result = manager.record_signature(&workflow_id, "signer1", "signed_psbt".to_string());
        assert!(result.is_ok());

        let workflow = manager.get_workflow(&workflow_id).unwrap();
        assert!(workflow.is_complete());
    }

    #[test]
    fn test_psbt_template_multisig() {
        let template = PsbtTemplate::multisig_2_of_3();
        assert_eq!(template.signers.len(), 3);
        assert_eq!(template.signers.iter().filter(|s| s.required).count(), 2);
    }

    #[test]
    fn test_psbt_template_single_cosigner() {
        let template = PsbtTemplate::single_with_cosigner();
        assert_eq!(template.signers.len(), 2);
        assert_eq!(template.signers.iter().filter(|s| s.required).count(), 1);
    }

    #[test]
    fn test_workflow_expiration() {
        let signers = vec![SignerRole {
            id: "signer1".to_string(),
            name: "Signer 1".to_string(),
            required: true,
        }];

        let expired_time = chrono::Utc::now() - chrono::Duration::hours(1);
        let workflow = PsbtWorkflow::new(
            "test_workflow".to_string(),
            "test_psbt".to_string(),
            signers,
            Some(expired_time),
        );

        assert!(workflow.is_expired());
    }

    #[test]
    fn test_pending_signers() {
        let signers = vec![
            SignerRole {
                id: "signer1".to_string(),
                name: "Signer 1".to_string(),
                required: true,
            },
            SignerRole {
                id: "signer2".to_string(),
                name: "Signer 2".to_string(),
                required: true,
            },
        ];

        let mut workflow = PsbtWorkflow::new(
            "test_workflow".to_string(),
            "test_psbt".to_string(),
            signers,
            None,
        );

        let pending = workflow.pending_signers();
        assert_eq!(pending.len(), 2);

        workflow
            .record_signature("signer1", "signed_psbt".to_string())
            .ok();

        let pending = workflow.pending_signers();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "signer2");
    }
}
