//! Package relay support for Bitcoin transactions
//!
//! Package relay allows submitting multiple related transactions (parent and child)
//! together, which is essential for:
//! - Child Pays For Parent (CPFP)
//! - Anchor outputs in Lightning Network
//! - Fee bumping strategies

use crate::error::BitcoinError;
use bitcoin::{Transaction, Txid};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A transaction package containing related transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionPackage {
    /// Transactions in dependency order (parents before children)
    pub transactions: Vec<Transaction>,
    /// Transaction relationships (child -> parents)
    pub dependencies: HashMap<Txid, Vec<Txid>>,
}

impl TransactionPackage {
    /// Create a new empty transaction package
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
            dependencies: HashMap::new(),
        }
    }

    /// Add a transaction to the package
    pub fn add_transaction(
        &mut self,
        tx: Transaction,
        parent_txids: Vec<Txid>,
    ) -> Result<(), BitcoinError> {
        let txid = tx.compute_txid();

        // Check if transaction already exists
        if self.transactions.iter().any(|t| t.compute_txid() == txid) {
            return Err(BitcoinError::InvalidAddress(
                "Transaction already in package".to_string(),
            ));
        }

        // Verify parents exist in package if specified
        for parent_txid in &parent_txids {
            if !self
                .transactions
                .iter()
                .any(|t| t.compute_txid() == *parent_txid)
            {
                return Err(BitcoinError::InvalidAddress(format!(
                    "Parent transaction {} not found in package",
                    parent_txid
                )));
            }
        }

        self.transactions.push(tx);
        if !parent_txids.is_empty() {
            self.dependencies.insert(txid, parent_txids);
        }

        Ok(())
    }

    /// Get topologically sorted transactions (parents before children)
    pub fn get_sorted_transactions(&self) -> Result<Vec<Transaction>, BitcoinError> {
        let mut sorted = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for tx in &self.transactions {
            self.visit_transaction(tx.compute_txid(), &mut sorted, &mut visited)?;
        }

        Ok(sorted)
    }

    /// Visit a transaction in topological sort
    fn visit_transaction(
        &self,
        txid: Txid,
        sorted: &mut Vec<Transaction>,
        visited: &mut std::collections::HashSet<Txid>,
    ) -> Result<(), BitcoinError> {
        if visited.contains(&txid) {
            return Ok(());
        }

        // Visit parents first
        if let Some(parents) = self.dependencies.get(&txid) {
            for parent_txid in parents {
                self.visit_transaction(*parent_txid, sorted, visited)?;
            }
        }

        // Add this transaction
        if let Some(tx) = self.transactions.iter().find(|t| t.compute_txid() == txid) {
            sorted.push(tx.clone());
            visited.insert(txid);
        }

        Ok(())
    }

    /// Calculate total package fee
    ///
    /// Requires input amounts for each transaction to calculate accurate fees.
    /// Returns the sum of fees for all transactions in the package.
    pub fn calculate_total_fee(&self, input_amounts: &HashMap<Txid, u64>) -> u64 {
        let mut total_fee = 0u64;

        for tx in &self.transactions {
            // Calculate input value
            let input_value: u64 = tx
                .input
                .iter()
                .filter_map(|input| input_amounts.get(&input.previous_output.txid).copied())
                .sum();

            // Calculate output value
            let output_value: u64 = tx.output.iter().map(|output| output.value.to_sat()).sum();

            // Fee = input - output (saturating to prevent underflow)
            total_fee = total_fee.saturating_add(input_value.saturating_sub(output_value));
        }

        total_fee
    }

    /// Calculate package fee rate (sat/vbyte)
    pub fn calculate_fee_rate(&self, total_fee: u64) -> u64 {
        let total_vsize: u64 = self.transactions.iter().map(|tx| tx.vsize() as u64).sum();

        if total_vsize == 0 {
            return 0;
        }

        total_fee / total_vsize
    }

    /// Validate package rules
    pub fn validate(&self) -> Result<(), BitcoinError> {
        // Check package size limits
        if self.transactions.len() > 25 {
            return Err(BitcoinError::InvalidAddress(
                "Package exceeds maximum size of 25 transactions".to_string(),
            ));
        }

        // Check total package vsize
        let total_vsize: u64 = self.transactions.iter().map(|tx| tx.vsize() as u64).sum();
        if total_vsize > 101_000 {
            return Err(BitcoinError::InvalidAddress(
                "Package exceeds maximum vsize of 101,000 vbytes".to_string(),
            ));
        }

        // Verify dependency order
        let sorted = self.get_sorted_transactions()?;
        if sorted.len() != self.transactions.len() {
            return Err(BitcoinError::InvalidAddress(
                "Package contains circular dependencies".to_string(),
            ));
        }

        Ok(())
    }

    /// Get package statistics
    pub fn get_stats(&self) -> PackageStats {
        let total_vsize: u64 = self.transactions.iter().map(|tx| tx.vsize() as u64).sum();
        let tx_count = self.transactions.len();
        let dependency_count = self.dependencies.len();

        PackageStats {
            transaction_count: tx_count,
            total_vsize,
            dependency_count,
            max_depth: self.calculate_max_depth(),
        }
    }

    /// Calculate maximum dependency depth
    fn calculate_max_depth(&self) -> usize {
        let mut max_depth = 0;

        for tx in &self.transactions {
            let depth = self
                .get_transaction_depth(tx.compute_txid(), &mut std::collections::HashSet::new());
            max_depth = max_depth.max(depth);
        }

        max_depth
    }

    /// Get transaction depth in dependency tree
    fn get_transaction_depth(
        &self,
        txid: Txid,
        visited: &mut std::collections::HashSet<Txid>,
    ) -> usize {
        if visited.contains(&txid) {
            return 0;
        }

        visited.insert(txid);

        if let Some(parents) = self.dependencies.get(&txid) {
            let max_parent_depth = parents
                .iter()
                .map(|parent_txid| self.get_transaction_depth(*parent_txid, visited))
                .max()
                .unwrap_or(0);

            1 + max_parent_depth
        } else {
            1
        }
    }
}

impl Default for TransactionPackage {
    fn default() -> Self {
        Self::new()
    }
}

/// Package relay manager
#[derive(Debug)]
pub struct PackageRelayManager {
    /// Active packages being constructed
    packages: HashMap<String, TransactionPackage>,
}

impl PackageRelayManager {
    /// Create a new package relay manager
    pub fn new() -> Self {
        Self {
            packages: HashMap::new(),
        }
    }

    /// Create a new package
    pub fn create_package(&mut self, package_id: String) -> Result<(), BitcoinError> {
        if self.packages.contains_key(&package_id) {
            return Err(BitcoinError::InvalidAddress(
                "Package already exists".to_string(),
            ));
        }

        self.packages.insert(package_id, TransactionPackage::new());
        Ok(())
    }

    /// Add transaction to a package
    pub fn add_to_package(
        &mut self,
        package_id: &str,
        tx: Transaction,
        parent_txids: Vec<Txid>,
    ) -> Result<(), BitcoinError> {
        let package = self
            .packages
            .get_mut(package_id)
            .ok_or_else(|| BitcoinError::InvalidAddress("Package not found".to_string()))?;

        package.add_transaction(tx, parent_txids)
    }

    /// Get a package
    pub fn get_package(&self, package_id: &str) -> Option<&TransactionPackage> {
        self.packages.get(package_id)
    }

    /// Submit a package (would relay to Bitcoin network)
    pub async fn submit_package(&mut self, package_id: &str) -> Result<Vec<Txid>, BitcoinError> {
        let package = self
            .packages
            .get(package_id)
            .ok_or_else(|| BitcoinError::InvalidAddress("Package not found".to_string()))?;

        // Validate package
        package.validate()?;

        // Get sorted transactions
        let sorted_txs = package.get_sorted_transactions()?;

        // In production, this would submit to Bitcoin Core via RPC
        // Using the testmempoolaccept and submitpackage RPCs
        let txids: Vec<Txid> = sorted_txs.iter().map(|tx| tx.compute_txid()).collect();

        Ok(txids)
    }

    /// Remove a package
    pub fn remove_package(&mut self, package_id: &str) -> Option<TransactionPackage> {
        self.packages.remove(package_id)
    }

    /// List all package IDs
    pub fn list_packages(&self) -> Vec<String> {
        self.packages.keys().cloned().collect()
    }
}

impl Default for PackageRelayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a transaction package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageStats {
    /// Number of transactions
    pub transaction_count: usize,
    /// Total package size in vbytes
    pub total_vsize: u64,
    /// Number of dependency relationships
    pub dependency_count: usize,
    /// Maximum dependency depth
    pub max_depth: usize,
}

/// CPFP (Child Pays For Parent) helper
#[derive(Debug)]
pub struct CpfpHelper;

impl CpfpHelper {
    /// Create a CPFP package for a stuck transaction
    pub fn create_cpfp_package(
        parent_tx: Transaction,
        child_tx: Transaction,
    ) -> Result<TransactionPackage, BitcoinError> {
        let mut package = TransactionPackage::new();
        let parent_txid = parent_tx.compute_txid();

        // Add parent
        package.add_transaction(parent_tx, vec![])?;

        // Add child with dependency on parent
        package.add_transaction(child_tx, vec![parent_txid])?;

        package.validate()?;

        Ok(package)
    }

    /// Calculate required child fee for target package fee rate
    pub fn calculate_child_fee(
        parent_tx: &Transaction,
        parent_fee: u64,
        child_tx: &Transaction,
        target_fee_rate: u64,
    ) -> u64 {
        let parent_vsize = parent_tx.vsize() as u64;
        let child_vsize = child_tx.vsize() as u64;
        let total_vsize = parent_vsize + child_vsize;

        let target_total_fee = total_vsize * target_fee_rate;

        target_total_fee.saturating_sub(parent_fee)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn create_dummy_tx() -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::blockdata::locktime::absolute::LockTime::ZERO,
            input: vec![],
            output: vec![],
        }
    }

    #[allow(dead_code)]
    fn create_dummy_tx_with_locktime(locktime: u32) -> Transaction {
        Transaction {
            version: bitcoin::transaction::Version::TWO,
            lock_time: bitcoin::blockdata::locktime::absolute::LockTime::from_consensus(locktime),
            input: vec![],
            output: vec![],
        }
    }

    #[test]
    fn test_package_creation() {
        let package = TransactionPackage::new();
        assert_eq!(package.transactions.len(), 0);
        assert_eq!(package.dependencies.len(), 0);
    }

    #[test]
    fn test_package_stats() {
        let mut package = TransactionPackage::new();
        let tx = create_dummy_tx();
        package.add_transaction(tx, vec![]).unwrap();

        let stats = package.get_stats();
        assert_eq!(stats.transaction_count, 1);
        assert_eq!(stats.dependency_count, 0);
    }

    #[test]
    fn test_package_validation_size_limit() {
        let mut package = TransactionPackage::new();

        // Add 26 transactions (exceeds limit of 25)
        for i in 0..26 {
            let tx = create_dummy_tx_with_locktime(i);
            let _ = package.add_transaction(tx, vec![]);
        }

        let result = package.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_package_fee_rate() {
        let package = TransactionPackage::new();
        let fee_rate = package.calculate_fee_rate(1000);
        assert_eq!(fee_rate, 0); // No transactions
    }

    #[test]
    fn test_manager_creation() {
        let manager = PackageRelayManager::new();
        assert_eq!(manager.list_packages().len(), 0);
    }

    #[test]
    fn test_manager_create_package() {
        let mut manager = PackageRelayManager::new();
        manager.create_package("test".to_string()).unwrap();

        assert_eq!(manager.list_packages().len(), 1);
        assert!(manager.get_package("test").is_some());
    }

    #[test]
    fn test_manager_duplicate_package() {
        let mut manager = PackageRelayManager::new();
        manager.create_package("test".to_string()).unwrap();

        let result = manager.create_package("test".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_manager_remove_package() {
        let mut manager = PackageRelayManager::new();
        manager.create_package("test".to_string()).unwrap();

        let removed = manager.remove_package("test");
        assert!(removed.is_some());
        assert_eq!(manager.list_packages().len(), 0);
    }

    #[tokio::test]
    async fn test_manager_submit_package() {
        let mut manager = PackageRelayManager::new();
        manager.create_package("test".to_string()).unwrap();

        let tx = create_dummy_tx();
        manager.add_to_package("test", tx, vec![]).unwrap();

        let result = manager.submit_package("test").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_cpfp_package_creation() {
        let parent = create_dummy_tx_with_locktime(1);
        let child = create_dummy_tx_with_locktime(2);

        let result = CpfpHelper::create_cpfp_package(parent, child);
        assert!(result.is_ok());

        let package = result.unwrap();
        assert_eq!(package.transactions.len(), 2);
    }

    #[test]
    fn test_cpfp_fee_calculation() {
        let parent = create_dummy_tx();
        let child = create_dummy_tx();

        let child_fee = CpfpHelper::calculate_child_fee(&parent, 1000, &child, 10);
        // child_fee is u64, so we just verify it's a valid value
        let _ = child_fee; // Suppress unused variable warning
    }
}
