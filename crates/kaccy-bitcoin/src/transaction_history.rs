//! Transaction history with pagination, filtering, and export

use bitcoin::Txid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

use crate::error::Result;

/// Transaction type for categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionType {
    Received,
    Sent,
    SelfTransfer,
    Unknown,
}

/// Transaction record for history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalTransaction {
    pub txid: Txid,
    pub tx_type: TransactionType,
    pub amount_sats: i64,
    pub fee_sats: Option<u64>,
    pub confirmations: u32,
    pub block_height: Option<u64>,
    pub block_time: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
    pub addresses: Vec<String>,
    pub label: Option<String>,
    pub notes: Option<String>,
}

/// Filter for querying transaction history
#[derive(Debug, Clone, Default)]
pub struct TransactionFilter {
    /// Filter by transaction type
    pub tx_type: Option<TransactionType>,
    /// Filter by minimum amount (in satoshis)
    pub min_amount: Option<i64>,
    /// Filter by maximum amount (in satoshis)
    pub max_amount: Option<i64>,
    /// Filter by start date
    pub start_date: Option<DateTime<Utc>>,
    /// Filter by end date
    pub end_date: Option<DateTime<Utc>>,
    /// Filter by minimum confirmations
    pub min_confirmations: Option<u32>,
    /// Filter by address
    pub address: Option<String>,
    /// Filter by label
    pub label: Option<String>,
    /// Search query (searches in labels, notes, addresses)
    pub search_query: Option<String>,
}

impl TransactionFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by transaction type
    pub fn with_type(mut self, tx_type: TransactionType) -> Self {
        self.tx_type = Some(tx_type);
        self
    }

    /// Filter by amount range
    pub fn with_amount_range(mut self, min: Option<i64>, max: Option<i64>) -> Self {
        self.min_amount = min;
        self.max_amount = max;
        self
    }

    /// Filter by date range
    pub fn with_date_range(
        mut self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Self {
        self.start_date = start;
        self.end_date = end;
        self
    }

    /// Filter by minimum confirmations
    pub fn with_min_confirmations(mut self, min_conf: u32) -> Self {
        self.min_confirmations = Some(min_conf);
        self
    }

    /// Filter by address
    pub fn with_address(mut self, address: String) -> Self {
        self.address = Some(address);
        self
    }

    /// Filter by search query
    pub fn with_search(mut self, query: String) -> Self {
        self.search_query = Some(query);
        self
    }

    /// Check if a transaction matches this filter
    pub fn matches(&self, tx: &HistoricalTransaction) -> bool {
        // Type filter
        if let Some(tx_type) = self.tx_type {
            if tx.tx_type != tx_type {
                return false;
            }
        }

        // Amount filters
        if let Some(min) = self.min_amount {
            if tx.amount_sats.abs() < min.abs() {
                return false;
            }
        }

        if let Some(max) = self.max_amount {
            if tx.amount_sats.abs() > max.abs() {
                return false;
            }
        }

        // Date filters
        if let Some(start) = self.start_date {
            if tx.timestamp < start {
                return false;
            }
        }

        if let Some(end) = self.end_date {
            if tx.timestamp > end {
                return false;
            }
        }

        // Confirmations filter
        if let Some(min_conf) = self.min_confirmations {
            if tx.confirmations < min_conf {
                return false;
            }
        }

        // Address filter
        if let Some(ref addr) = self.address {
            if !tx.addresses.contains(addr) {
                return false;
            }
        }

        // Label filter
        if let Some(ref label) = self.label {
            if tx.label.as_ref() != Some(label) {
                return false;
            }
        }

        // Search query (searches in labels, notes, addresses, txid)
        if let Some(ref query) = self.search_query {
            let query_lower = query.to_lowercase();
            let matches = tx
                .label
                .as_ref()
                .is_some_and(|l| l.to_lowercase().contains(&query_lower))
                || tx
                    .notes
                    .as_ref()
                    .is_some_and(|n| n.to_lowercase().contains(&query_lower))
                || tx
                    .addresses
                    .iter()
                    .any(|a| a.to_lowercase().contains(&query_lower))
                || tx.txid.to_string().contains(&query_lower);

            if !matches {
                return false;
            }
        }

        true
    }
}

/// Pagination options
#[derive(Debug, Clone)]
pub struct PaginationOptions {
    /// Page number (0-indexed)
    pub page: usize,
    /// Items per page
    pub page_size: usize,
    /// Sort by field
    pub sort_by: SortField,
    /// Sort order
    pub sort_order: SortOrder,
}

impl Default for PaginationOptions {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: 50,
            sort_by: SortField::Timestamp,
            sort_order: SortOrder::Descending,
        }
    }
}

/// Fields to sort by
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Timestamp,
    Amount,
    Confirmations,
    BlockHeight,
}

/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Paginated result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult {
    pub transactions: Vec<HistoricalTransaction>,
    pub total_count: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

/// Transaction history manager
pub struct TransactionHistory {
    transactions: Vec<HistoricalTransaction>,
    labels: HashMap<Txid, String>,
    notes: HashMap<Txid, String>,
}

impl TransactionHistory {
    /// Create a new transaction history manager
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
            labels: HashMap::new(),
            notes: HashMap::new(),
        }
    }

    /// Add a transaction to history
    pub fn add_transaction(&mut self, mut tx: HistoricalTransaction) {
        // Apply labels and notes if available
        if let Some(label) = self.labels.get(&tx.txid) {
            tx.label = Some(label.clone());
        }
        if let Some(notes) = self.notes.get(&tx.txid) {
            tx.notes = Some(notes.clone());
        }

        self.transactions.push(tx);
        debug!(
            count = self.transactions.len(),
            "Transaction added to history"
        );
    }

    /// Set label for a transaction
    pub fn set_label(&mut self, txid: Txid, label: String) {
        self.labels.insert(txid, label.clone());

        // Update existing transaction if present
        if let Some(tx) = self.transactions.iter_mut().find(|t| t.txid == txid) {
            tx.label = Some(label);
        }
    }

    /// Set notes for a transaction
    pub fn set_notes(&mut self, txid: Txid, notes: String) {
        self.notes.insert(txid, notes.clone());

        // Update existing transaction if present
        if let Some(tx) = self.transactions.iter_mut().find(|t| t.txid == txid) {
            tx.notes = Some(notes);
        }
    }

    /// Get transaction by txid
    pub fn get_transaction(&self, txid: &Txid) -> Option<&HistoricalTransaction> {
        self.transactions.iter().find(|t| &t.txid == txid)
    }

    /// Query transactions with filter and pagination
    pub fn query(
        &self,
        filter: Option<&TransactionFilter>,
        pagination: Option<&PaginationOptions>,
    ) -> PaginatedResult {
        // Apply filter
        let mut filtered: Vec<HistoricalTransaction> = if let Some(f) = filter {
            self.transactions
                .iter()
                .filter(|tx| f.matches(tx))
                .cloned()
                .collect()
        } else {
            self.transactions.clone()
        };

        // Sort
        let pagination = pagination.cloned().unwrap_or_default();
        match (pagination.sort_by, pagination.sort_order) {
            (SortField::Timestamp, SortOrder::Ascending) => filtered.sort_by_key(|tx| tx.timestamp),
            (SortField::Timestamp, SortOrder::Descending) => {
                filtered.sort_by_key(|tx| std::cmp::Reverse(tx.timestamp))
            }
            (SortField::Amount, SortOrder::Ascending) => filtered.sort_by_key(|tx| tx.amount_sats),
            (SortField::Amount, SortOrder::Descending) => {
                filtered.sort_by_key(|tx| std::cmp::Reverse(tx.amount_sats))
            }
            (SortField::Confirmations, SortOrder::Ascending) => {
                filtered.sort_by_key(|tx| tx.confirmations)
            }
            (SortField::Confirmations, SortOrder::Descending) => {
                filtered.sort_by_key(|tx| std::cmp::Reverse(tx.confirmations))
            }
            (SortField::BlockHeight, SortOrder::Ascending) => {
                filtered.sort_by_key(|tx| tx.block_height)
            }
            (SortField::BlockHeight, SortOrder::Descending) => {
                filtered.sort_by_key(|tx| std::cmp::Reverse(tx.block_height))
            }
        }

        // Paginate
        let total_count = filtered.len();
        let total_pages = total_count.div_ceil(pagination.page_size);

        let start = pagination.page * pagination.page_size;
        let end = (start + pagination.page_size).min(total_count);

        let transactions = if start < total_count {
            filtered[start..end].to_vec()
        } else {
            Vec::new()
        };

        PaginatedResult {
            transactions,
            total_count,
            page: pagination.page,
            page_size: pagination.page_size,
            total_pages,
        }
    }

    /// Get summary statistics
    pub fn get_summary(&self) -> TransactionSummary {
        let total_received = self
            .transactions
            .iter()
            .filter(|t| t.tx_type == TransactionType::Received)
            .map(|t| t.amount_sats.unsigned_abs())
            .sum();

        let total_sent = self
            .transactions
            .iter()
            .filter(|t| t.tx_type == TransactionType::Sent)
            .map(|t| t.amount_sats.unsigned_abs())
            .sum();

        let total_fees = self.transactions.iter().filter_map(|t| t.fee_sats).sum();

        TransactionSummary {
            total_transactions: self.transactions.len(),
            total_received_sats: total_received,
            total_sent_sats: total_sent,
            total_fees_sats: total_fees,
            confirmed_transactions: self
                .transactions
                .iter()
                .filter(|t| t.confirmations > 0)
                .count(),
            pending_transactions: self
                .transactions
                .iter()
                .filter(|t| t.confirmations == 0)
                .count(),
        }
    }

    /// Export to CSV format
    pub fn export_csv(&self, filter: Option<&TransactionFilter>) -> String {
        let mut csv = String::from(
            "Txid,Type,Amount (sats),Fee (sats),Confirmations,Block Height,Timestamp,Label,Notes\n",
        );

        let transactions = if let Some(f) = filter {
            self.transactions
                .iter()
                .filter(|tx| f.matches(tx))
                .collect::<Vec<_>>()
        } else {
            self.transactions.iter().collect::<Vec<_>>()
        };

        for tx in transactions {
            csv.push_str(&format!(
                "{},{:?},{},{},{},{},{},{},{}\n",
                tx.txid,
                tx.tx_type,
                tx.amount_sats,
                tx.fee_sats.map_or("".to_string(), |f| f.to_string()),
                tx.confirmations,
                tx.block_height.map_or("".to_string(), |h| h.to_string()),
                tx.timestamp.to_rfc3339(),
                tx.label.as_deref().unwrap_or(""),
                tx.notes.as_deref().unwrap_or("")
            ));
        }

        csv
    }

    /// Export to JSON format
    pub fn export_json(&self, filter: Option<&TransactionFilter>) -> Result<String> {
        let transactions = if let Some(f) = filter {
            self.transactions
                .iter()
                .filter(|tx| f.matches(tx))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            self.transactions.clone()
        };

        serde_json::to_string_pretty(&transactions).map_err(|e| {
            crate::error::BitcoinError::Validation(format!("JSON serialization failed: {}", e))
        })
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.transactions.clear();
        debug!("Transaction history cleared");
    }

    /// Detect address reuse
    pub fn detect_address_reuse(&self) -> Vec<AddressReuseReport> {
        let mut address_usage: HashMap<String, Vec<Txid>> = HashMap::new();

        for tx in &self.transactions {
            for addr in &tx.addresses {
                address_usage.entry(addr.clone()).or_default().push(tx.txid);
            }
        }

        address_usage
            .into_iter()
            .filter(|(_, txids)| txids.len() > 1)
            .map(|(address, txids)| AddressReuseReport {
                address,
                usage_count: txids.len(),
                transactions: txids,
            })
            .collect()
    }
}

impl Default for TransactionHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Transaction summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub total_transactions: usize,
    pub total_received_sats: u64,
    pub total_sent_sats: u64,
    pub total_fees_sats: u64,
    pub confirmed_transactions: usize,
    pub pending_transactions: usize,
}

impl TransactionSummary {
    /// Get net balance change
    pub fn net_balance_sats(&self) -> i64 {
        self.total_received_sats as i64 - self.total_sent_sats as i64 - self.total_fees_sats as i64
    }

    /// Get balance in BTC
    pub fn net_balance_btc(&self) -> f64 {
        self.net_balance_sats() as f64 / 100_000_000.0
    }
}

/// Address reuse detection report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressReuseReport {
    pub address: String,
    pub usage_count: usize,
    pub transactions: Vec<Txid>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;

    fn create_test_tx(amount: i64, tx_type: TransactionType) -> HistoricalTransaction {
        HistoricalTransaction {
            txid: Txid::all_zeros(),
            tx_type,
            amount_sats: amount,
            fee_sats: Some(1000),
            confirmations: 6,
            block_height: Some(700000),
            block_time: Some(Utc::now()),
            timestamp: Utc::now(),
            addresses: vec!["bc1q...".to_string()],
            label: None,
            notes: None,
        }
    }

    #[test]
    fn test_transaction_filter_new() {
        let filter = TransactionFilter::new();
        assert!(filter.tx_type.is_none());
        assert!(filter.min_amount.is_none());
    }

    #[test]
    fn test_transaction_filter_builder() {
        let filter = TransactionFilter::new()
            .with_type(TransactionType::Received)
            .with_min_confirmations(6);

        assert_eq!(filter.tx_type, Some(TransactionType::Received));
        assert_eq!(filter.min_confirmations, Some(6));
    }

    #[test]
    fn test_transaction_history_add() {
        let mut history = TransactionHistory::new();
        let tx = create_test_tx(100000, TransactionType::Received);

        history.add_transaction(tx);
        assert_eq!(history.transactions.len(), 1);
    }

    #[test]
    fn test_transaction_history_labels() {
        let mut history = TransactionHistory::new();
        let txid = Txid::all_zeros();

        history.set_label(txid, "Test Payment".to_string());
        assert_eq!(history.labels.get(&txid).unwrap(), "Test Payment");

        let tx = create_test_tx(100000, TransactionType::Received);
        history.add_transaction(tx);

        let stored = history.get_transaction(&txid).unwrap();
        assert_eq!(stored.label, Some("Test Payment".to_string()));
    }

    #[test]
    fn test_pagination_defaults() {
        let opts = PaginationOptions::default();
        assert_eq!(opts.page, 0);
        assert_eq!(opts.page_size, 50);
    }

    #[test]
    fn test_transaction_query() {
        let mut history = TransactionHistory::new();

        for i in 0..100 {
            let mut tx = create_test_tx(100000 * i, TransactionType::Received);
            tx.txid = Txid::from_byte_array([i as u8; 32]);
            history.add_transaction(tx);
        }

        let pagination = PaginationOptions {
            page: 0,
            page_size: 10,
            ..Default::default()
        };

        let result = history.query(None, Some(&pagination));
        assert_eq!(result.transactions.len(), 10);
        assert_eq!(result.total_count, 100);
        assert_eq!(result.total_pages, 10);
    }

    #[test]
    fn test_transaction_filter_matches() {
        let filter = TransactionFilter::new().with_type(TransactionType::Received);

        let tx1 = create_test_tx(100000, TransactionType::Received);
        let tx2 = create_test_tx(-100000, TransactionType::Sent);

        assert!(filter.matches(&tx1));
        assert!(!filter.matches(&tx2));
    }

    #[test]
    fn test_transaction_summary() {
        let mut history = TransactionHistory::new();

        history.add_transaction(create_test_tx(100000, TransactionType::Received));
        history.add_transaction(create_test_tx(-50000, TransactionType::Sent));

        let summary = history.get_summary();
        assert_eq!(summary.total_transactions, 2);
        assert_eq!(summary.total_received_sats, 100000);
        assert_eq!(summary.total_sent_sats, 50000);
    }

    #[test]
    fn test_export_csv() {
        let mut history = TransactionHistory::new();
        history.add_transaction(create_test_tx(100000, TransactionType::Received));

        let csv = history.export_csv(None);
        assert!(csv.contains("Txid"));
        assert!(csv.contains("100000"));
    }

    #[test]
    fn test_address_reuse_detection() {
        let mut history = TransactionHistory::new();

        let mut tx1 = create_test_tx(100000, TransactionType::Received);
        tx1.txid = Txid::from_byte_array([1; 32]);
        tx1.addresses = vec!["bc1qtest".to_string()];

        let mut tx2 = create_test_tx(50000, TransactionType::Received);
        tx2.txid = Txid::from_byte_array([2; 32]);
        tx2.addresses = vec!["bc1qtest".to_string()];

        history.add_transaction(tx1);
        history.add_transaction(tx2);

        let reuse = history.detect_address_reuse();
        assert_eq!(reuse.len(), 1);
        assert_eq!(reuse[0].usage_count, 2);
    }
}
