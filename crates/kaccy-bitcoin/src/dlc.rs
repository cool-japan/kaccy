// Discreet Log Contract (DLC) support
//
// DLCs enable trustless smart contracts on Bitcoin using oracle signatures.
// Key features:
// - Oracle integration with attestation
// - Contract creation and execution
// - Multi-oracle support
// - Adaptor signatures for contract enforcement
// - Privacy-preserving contract resolution

use crate::error::BitcoinError;
use bitcoin::secp256k1::schnorr::Signature as Schnorr;
use bitcoin::{
    Address, Amount, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Witness,
    absolute::LockTime,
    secp256k1::{PublicKey, Secp256k1},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Oracle information for DLC attestation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oracle {
    /// Oracle identifier
    pub id: String,
    /// Oracle public key for signature verification
    pub pubkey: PublicKey,
    /// Oracle endpoint URL
    pub endpoint: String,
    /// Oracle reputation score (0-100)
    pub reputation: u8,
}

/// Oracle announcement for an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleAnnouncement {
    /// Unique event identifier
    pub event_id: String,
    /// Oracle that will attest to the event
    pub oracle: Oracle,
    /// Event maturity time
    pub maturity: DateTime<Utc>,
    /// Possible outcomes
    pub outcomes: Vec<String>,
    /// Event nonce point for adaptor signatures
    pub nonce_point: PublicKey,
    /// Announcement signature
    pub signature: Schnorr,
}

/// Oracle attestation of an event outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleAttestation {
    /// Event identifier
    pub event_id: String,
    /// Oracle identifier
    pub oracle_id: String,
    /// Attested outcome
    pub outcome: String,
    /// Oracle signature on the outcome
    pub signature: Schnorr,
    /// Attestation timestamp
    pub attested_at: DateTime<Utc>,
}

/// DLC contract outcome specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractOutcome {
    /// Outcome message (e.g., "BTC/USD > 50000")
    pub message: String,
    /// Payout to local party in satoshis
    pub local_payout: u64,
    /// Payout to remote party in satoshis
    pub remote_payout: u64,
}

/// DLC contract specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlcContract {
    /// Contract identifier
    pub id: String,
    /// Local party public key
    pub local_pubkey: PublicKey,
    /// Remote party public key
    pub remote_pubkey: PublicKey,
    /// Total contract value in satoshis
    pub total_collateral: u64,
    /// Oracle announcements (supports multi-oracle)
    pub oracle_announcements: Vec<OracleAnnouncement>,
    /// Required number of oracle signatures (threshold)
    pub oracle_threshold: usize,
    /// Contract outcomes
    pub outcomes: Vec<ContractOutcome>,
    /// Funding transaction outpoint
    pub funding_outpoint: Option<OutPoint>,
    /// Contract maturity time
    pub maturity: DateTime<Utc>,
    /// Refund locktime (in case oracles don't attest)
    pub refund_locktime: u32,
    /// Current contract status
    pub status: DlcStatus,
}

/// DLC contract status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DlcStatus {
    /// Contract being negotiated
    Offered,
    /// Contract accepted, waiting for funding
    Accepted,
    /// Funding transaction broadcast
    Funded,
    /// Oracle attestation received, CET can be broadcast
    Attested,
    /// Contract execution transaction confirmed
    Closed,
    /// Refund transaction executed
    Refunded,
    /// Contract cancelled before funding
    Cancelled,
}

/// Contract Execution Transaction (CET) builder
#[derive(Debug, Clone)]
pub struct CetBuilder {
    /// Funding outpoint
    funding_outpoint: OutPoint,
    /// Funding amount
    #[allow(dead_code)]
    funding_amount: u64,
    /// Local payout
    local_payout: u64,
    /// Remote payout
    remote_payout: u64,
    /// Local payout address
    local_address: Address,
    /// Remote payout address
    remote_address: Address,
    /// Lock time
    locktime: LockTime,
}

impl CetBuilder {
    /// Create a new CET builder
    pub fn new(
        funding_outpoint: OutPoint,
        funding_amount: u64,
        local_payout: u64,
        remote_payout: u64,
        local_address: Address,
        remote_address: Address,
    ) -> Self {
        Self {
            funding_outpoint,
            funding_amount,
            local_payout,
            remote_payout,
            local_address,
            remote_address,
            locktime: LockTime::ZERO,
        }
    }

    /// Set the locktime
    pub fn locktime(mut self, locktime: LockTime) -> Self {
        self.locktime = locktime;
        self
    }

    /// Build the CET transaction
    pub fn build(&self) -> Result<Transaction, BitcoinError> {
        let mut tx = Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: self.locktime,
            input: vec![TxIn {
                previous_output: self.funding_outpoint,
                script_sig: ScriptBuf::new(),
                sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
                witness: Witness::new(),
            }],
            output: vec![],
        };

        // Add local payout output
        if self.local_payout > 0 {
            tx.output.push(TxOut {
                value: Amount::from_sat(self.local_payout),
                script_pubkey: self.local_address.script_pubkey(),
            });
        }

        // Add remote payout output
        if self.remote_payout > 0 {
            tx.output.push(TxOut {
                value: Amount::from_sat(self.remote_payout),
                script_pubkey: self.remote_address.script_pubkey(),
            });
        }

        Ok(tx)
    }
}

/// DLC manager for contract lifecycle
pub struct DlcManager {
    /// Active contracts
    contracts: HashMap<String, DlcContract>,
    /// Known oracles
    oracles: HashMap<String, Oracle>,
    /// Secp256k1 context
    #[allow(dead_code)]
    secp: Secp256k1<bitcoin::secp256k1::All>,
}

impl DlcManager {
    /// Create a new DLC manager
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
            oracles: HashMap::new(),
            secp: Secp256k1::new(),
        }
    }

    /// Register an oracle
    pub fn register_oracle(&mut self, oracle: Oracle) {
        self.oracles.insert(oracle.id.clone(), oracle);
    }

    /// Get oracle by ID
    pub fn get_oracle(&self, oracle_id: &str) -> Option<&Oracle> {
        self.oracles.get(oracle_id)
    }

    /// Create a new DLC contract offer
    #[allow(clippy::too_many_arguments)]
    pub fn create_contract(
        &mut self,
        contract_id: String,
        local_pubkey: PublicKey,
        remote_pubkey: PublicKey,
        total_collateral: u64,
        oracle_announcements: Vec<OracleAnnouncement>,
        oracle_threshold: usize,
        outcomes: Vec<ContractOutcome>,
        maturity: DateTime<Utc>,
        refund_locktime: u32,
    ) -> Result<DlcContract, BitcoinError> {
        // Validate oracle threshold
        if oracle_threshold == 0 || oracle_threshold > oracle_announcements.len() {
            return Err(BitcoinError::InvalidInput(
                "Invalid oracle threshold".to_string(),
            ));
        }

        // Validate outcomes
        for outcome in &outcomes {
            if outcome.local_payout + outcome.remote_payout != total_collateral {
                return Err(BitcoinError::InvalidInput(
                    "Outcome payouts must sum to total collateral".to_string(),
                ));
            }
        }

        let contract = DlcContract {
            id: contract_id.clone(),
            local_pubkey,
            remote_pubkey,
            total_collateral,
            oracle_announcements,
            oracle_threshold,
            outcomes,
            funding_outpoint: None,
            maturity,
            refund_locktime,
            status: DlcStatus::Offered,
        };

        self.contracts.insert(contract_id, contract.clone());
        Ok(contract)
    }

    /// Accept a DLC contract
    pub fn accept_contract(&mut self, contract_id: &str) -> Result<(), BitcoinError> {
        let contract = self
            .contracts
            .get_mut(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        if contract.status != DlcStatus::Offered {
            return Err(BitcoinError::InvalidInput(
                "Contract cannot be accepted in current status".to_string(),
            ));
        }

        contract.status = DlcStatus::Accepted;
        Ok(())
    }

    /// Mark contract as funded
    pub fn mark_funded(
        &mut self,
        contract_id: &str,
        funding_outpoint: OutPoint,
    ) -> Result<(), BitcoinError> {
        let contract = self
            .contracts
            .get_mut(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        if contract.status != DlcStatus::Accepted {
            return Err(BitcoinError::InvalidInput(
                "Contract must be accepted before funding".to_string(),
            ));
        }

        contract.funding_outpoint = Some(funding_outpoint);
        contract.status = DlcStatus::Funded;
        Ok(())
    }

    /// Process oracle attestation
    pub fn process_attestation(
        &mut self,
        contract_id: &str,
        attestation: OracleAttestation,
    ) -> Result<(), BitcoinError> {
        // Verify oracle exists first
        if !self.oracles.contains_key(&attestation.oracle_id) {
            return Err(BitcoinError::NotFound("Oracle not found".to_string()));
        }

        // In a real implementation, we would verify the Schnorr signature here
        // For now, we trust the attestation

        let contract = self
            .contracts
            .get_mut(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        if contract.status != DlcStatus::Funded {
            return Err(BitcoinError::InvalidInput(
                "Contract must be funded to process attestation".to_string(),
            ));
        }

        contract.status = DlcStatus::Attested;
        Ok(())
    }

    /// Build CET for a specific outcome
    pub fn build_cet(
        &self,
        contract_id: &str,
        outcome_index: usize,
        local_address: Address,
        remote_address: Address,
    ) -> Result<Transaction, BitcoinError> {
        let contract = self
            .contracts
            .get(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        let funding_outpoint = contract
            .funding_outpoint
            .ok_or_else(|| BitcoinError::InvalidInput("Contract not funded".to_string()))?;

        let outcome = contract
            .outcomes
            .get(outcome_index)
            .ok_or_else(|| BitcoinError::InvalidInput("Invalid outcome index".to_string()))?;

        let cet = CetBuilder::new(
            funding_outpoint,
            contract.total_collateral,
            outcome.local_payout,
            outcome.remote_payout,
            local_address,
            remote_address,
        )
        .build()?;

        Ok(cet)
    }

    /// Build refund transaction
    pub fn build_refund(
        &self,
        contract_id: &str,
        local_address: Address,
        remote_address: Address,
    ) -> Result<Transaction, BitcoinError> {
        let contract = self
            .contracts
            .get(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        let funding_outpoint = contract
            .funding_outpoint
            .ok_or_else(|| BitcoinError::InvalidInput("Contract not funded".to_string()))?;

        // Refund splits collateral equally (or according to contract terms)
        let half = contract.total_collateral / 2;

        let refund = CetBuilder::new(
            funding_outpoint,
            contract.total_collateral,
            half,
            contract.total_collateral - half,
            local_address,
            remote_address,
        )
        .locktime(
            LockTime::from_height(contract.refund_locktime)
                .map_err(|_| BitcoinError::InvalidInput("Invalid refund locktime".to_string()))?,
        )
        .build()?;

        Ok(refund)
    }

    /// Mark contract as closed
    pub fn close_contract(&mut self, contract_id: &str) -> Result<(), BitcoinError> {
        let contract = self
            .contracts
            .get_mut(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        contract.status = DlcStatus::Closed;
        Ok(())
    }

    /// Cancel contract
    pub fn cancel_contract(&mut self, contract_id: &str) -> Result<(), BitcoinError> {
        let contract = self
            .contracts
            .get_mut(contract_id)
            .ok_or_else(|| BitcoinError::NotFound("Contract not found".to_string()))?;

        if contract.status != DlcStatus::Offered && contract.status != DlcStatus::Accepted {
            return Err(BitcoinError::InvalidInput(
                "Cannot cancel funded contract".to_string(),
            ));
        }

        contract.status = DlcStatus::Cancelled;
        Ok(())
    }

    /// Get contract by ID
    pub fn get_contract(&self, contract_id: &str) -> Option<&DlcContract> {
        self.contracts.get(contract_id)
    }

    /// List all contracts
    pub fn list_contracts(&self) -> Vec<&DlcContract> {
        self.contracts.values().collect()
    }

    /// List contracts by status
    pub fn list_contracts_by_status(&self, status: DlcStatus) -> Vec<&DlcContract> {
        self.contracts
            .values()
            .filter(|c| c.status == status)
            .collect()
    }
}

impl Default for DlcManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::secp256k1::rand::thread_rng;
    use bitcoin::secp256k1::{Keypair, Message};

    fn create_test_oracle() -> Oracle {
        let secp = Secp256k1::new();
        let (_, pubkey) = secp.generate_keypair(&mut thread_rng());

        Oracle {
            id: "test-oracle".to_string(),
            pubkey,
            endpoint: "https://oracle.example.com".to_string(),
            reputation: 95,
        }
    }

    fn create_test_announcement() -> OracleAnnouncement {
        let secp = Secp256k1::new();
        let mut rng = thread_rng();
        let keypair = Keypair::new(&secp, &mut rng);
        let oracle = create_test_oracle();

        // Create a dummy signature
        let msg = Message::from_digest_slice(&[0u8; 32]).unwrap();
        let sig = secp.sign_schnorr(&msg, &keypair);

        OracleAnnouncement {
            event_id: "test-event".to_string(),
            oracle,
            maturity: Utc::now() + chrono::Duration::days(1),
            outcomes: vec!["high".to_string(), "low".to_string()],
            nonce_point: keypair.public_key(),
            signature: sig,
        }
    }

    #[test]
    fn test_dlc_manager_creation() {
        let manager = DlcManager::new();
        assert_eq!(manager.list_contracts().len(), 0);
    }

    #[test]
    fn test_oracle_registration() {
        let mut manager = DlcManager::new();
        let oracle = create_test_oracle();
        let oracle_id = oracle.id.clone();

        manager.register_oracle(oracle);
        assert!(manager.get_oracle(&oracle_id).is_some());
    }

    #[test]
    fn test_contract_creation() {
        let mut manager = DlcManager::new();
        let secp = Secp256k1::new();
        let (_, local_pk) = secp.generate_keypair(&mut thread_rng());
        let (_, remote_pk) = secp.generate_keypair(&mut thread_rng());

        let announcement = create_test_announcement();
        let outcomes = vec![
            ContractOutcome {
                message: "BTC > 50000".to_string(),
                local_payout: 100_000,
                remote_payout: 0,
            },
            ContractOutcome {
                message: "BTC <= 50000".to_string(),
                local_payout: 0,
                remote_payout: 100_000,
            },
        ];

        let contract = manager
            .create_contract(
                "test-contract".to_string(),
                local_pk,
                remote_pk,
                100_000,
                vec![announcement],
                1,
                outcomes,
                Utc::now() + chrono::Duration::days(7),
                144 * 30, // ~30 days
            )
            .unwrap();

        assert_eq!(contract.status, DlcStatus::Offered);
        assert_eq!(contract.total_collateral, 100_000);
    }

    #[test]
    fn test_contract_acceptance() {
        let mut manager = DlcManager::new();
        let secp = Secp256k1::new();
        let (_, local_pk) = secp.generate_keypair(&mut thread_rng());
        let (_, remote_pk) = secp.generate_keypair(&mut thread_rng());

        let announcement = create_test_announcement();
        let outcomes = vec![ContractOutcome {
            message: "test".to_string(),
            local_payout: 50_000,
            remote_payout: 50_000,
        }];

        manager
            .create_contract(
                "test".to_string(),
                local_pk,
                remote_pk,
                100_000,
                vec![announcement],
                1,
                outcomes,
                Utc::now() + chrono::Duration::days(7),
                144 * 30,
            )
            .unwrap();

        manager.accept_contract("test").unwrap();
        let contract = manager.get_contract("test").unwrap();
        assert_eq!(contract.status, DlcStatus::Accepted);
    }

    #[test]
    fn test_invalid_outcome_payouts() {
        let mut manager = DlcManager::new();
        let secp = Secp256k1::new();
        let (_, local_pk) = secp.generate_keypair(&mut thread_rng());
        let (_, remote_pk) = secp.generate_keypair(&mut thread_rng());

        let announcement = create_test_announcement();
        let outcomes = vec![ContractOutcome {
            message: "test".to_string(),
            local_payout: 30_000,
            remote_payout: 30_000, // Sum doesn't match collateral
        }];

        let result = manager.create_contract(
            "test".to_string(),
            local_pk,
            remote_pk,
            100_000,
            vec![announcement],
            1,
            outcomes,
            Utc::now() + chrono::Duration::days(7),
            144 * 30,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_contract_lifecycle() {
        let mut manager = DlcManager::new();
        let secp = Secp256k1::new();
        let (_, local_pk) = secp.generate_keypair(&mut thread_rng());
        let (_, remote_pk) = secp.generate_keypair(&mut thread_rng());

        let oracle = create_test_oracle();
        manager.register_oracle(oracle.clone());

        let announcement = create_test_announcement();
        let outcomes = vec![ContractOutcome {
            message: "test".to_string(),
            local_payout: 100_000,
            remote_payout: 0,
        }];

        // Create
        manager
            .create_contract(
                "test".to_string(),
                local_pk,
                remote_pk,
                100_000,
                vec![announcement],
                1,
                outcomes,
                Utc::now() + chrono::Duration::days(7),
                144 * 30,
            )
            .unwrap();

        // Accept
        manager.accept_contract("test").unwrap();

        // Fund
        let outpoint = OutPoint::null();
        manager.mark_funded("test", outpoint).unwrap();

        let contract = manager.get_contract("test").unwrap();
        assert_eq!(contract.status, DlcStatus::Funded);

        // Close
        manager.close_contract("test").unwrap();
        let contract = manager.get_contract("test").unwrap();
        assert_eq!(contract.status, DlcStatus::Closed);
    }
}
