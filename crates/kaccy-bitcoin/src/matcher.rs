//! Transaction to order matching

use bitcoin::Amount;
use std::collections::HashMap;
use uuid::Uuid;

use crate::client::BitcoinClient;
use crate::error::Result;

/// A pending order awaiting payment
#[derive(Debug, Clone)]
pub struct OrderMatch {
    /// Order ID
    pub order_id: Uuid,
    /// Payment address
    pub address: String,
    /// Expected amount in satoshis
    pub expected_amount_sats: u64,
}

/// Result of matching a transaction to an order
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Order ID that was matched
    pub order_id: Uuid,
    /// Transaction ID
    pub txid: String,
    /// Received amount in satoshis
    pub received_amount_sats: u64,
    /// Expected amount in satoshis
    pub expected_amount_sats: u64,
    /// Number of confirmations
    pub confirmations: i32,
    /// Match status
    pub status: MatchStatus,
}

/// Status of a match
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStatus {
    /// Amount matches exactly
    Exact,
    /// Received less than expected
    Underpaid,
    /// Received more than expected
    Overpaid,
}

impl MatchResult {
    /// Check if payment is sufficient (exact or overpaid)
    pub fn is_sufficient(&self) -> bool {
        matches!(self.status, MatchStatus::Exact | MatchStatus::Overpaid)
    }

    /// Get the shortfall amount if underpaid
    pub fn shortfall(&self) -> Option<u64> {
        if self.status == MatchStatus::Underpaid {
            Some(
                self.expected_amount_sats
                    .saturating_sub(self.received_amount_sats),
            )
        } else {
            None
        }
    }

    /// Get the excess amount if overpaid
    pub fn excess(&self) -> Option<u64> {
        if self.status == MatchStatus::Overpaid {
            Some(
                self.received_amount_sats
                    .saturating_sub(self.expected_amount_sats),
            )
        } else {
            None
        }
    }
}

/// Matches incoming Bitcoin transactions to pending orders
pub struct TransactionMatcher {
    client: std::sync::Arc<BitcoinClient>,
    /// Minimum confirmations to consider a transaction
    min_confirmations: i32,
}

impl TransactionMatcher {
    /// Create a new transaction matcher
    pub fn new(client: std::sync::Arc<BitcoinClient>, min_confirmations: i32) -> Self {
        Self {
            client,
            min_confirmations,
        }
    }

    /// Match transactions to orders
    ///
    /// Returns a list of match results for orders that have received payments.
    pub fn match_transactions(
        &self,
        orders: &[OrderMatch],
        since_block: Option<&bitcoin::BlockHash>,
    ) -> Result<Vec<MatchResult>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        }

        // Build address -> order lookup
        let order_by_address: HashMap<&str, &OrderMatch> =
            orders.iter().map(|o| (o.address.as_str(), o)).collect();

        // Get recent transactions
        let result = self.client.list_since_block(since_block, None)?;

        let mut matches = Vec::new();

        // Match transactions to orders
        for tx in &result.transactions {
            // Only consider receive transactions
            if tx.category != "Receive" {
                continue;
            }

            // Check if we have an order for this address
            if let Some(address) = &tx.address {
                if let Some(order) = order_by_address.get(address.as_str()) {
                    // Determine amount (handle both positive and negative)
                    let received_sats = tx.amount.unsigned_abs();

                    let status = match received_sats.cmp(&order.expected_amount_sats) {
                        std::cmp::Ordering::Equal => MatchStatus::Exact,
                        std::cmp::Ordering::Less => MatchStatus::Underpaid,
                        std::cmp::Ordering::Greater => MatchStatus::Overpaid,
                    };

                    matches.push(MatchResult {
                        order_id: order.order_id,
                        txid: tx.txid.to_string(),
                        received_amount_sats: received_sats,
                        expected_amount_sats: order.expected_amount_sats,
                        confirmations: tx.confirmations,
                        status,
                    });
                }
            }
        }

        Ok(matches)
    }

    /// Check payment status for a single order
    pub fn check_order_payment(&self, order: &OrderMatch) -> Result<Option<MatchResult>> {
        let address: bitcoin::Address<bitcoin::address::NetworkUnchecked> =
            order
                .address
                .parse()
                .map_err(|e| crate::error::BitcoinError::InvalidAddress(format!("{:?}", e)))?;

        let checked_addr = address.assume_checked();

        // Get unconfirmed amount
        let unconfirmed = self
            .client
            .get_received_by_address(&checked_addr, Some(0))?;

        if unconfirmed == Amount::ZERO {
            return Ok(None);
        }

        // Get confirmed amount
        let confirmed = self
            .client
            .get_received_by_address(&checked_addr, Some(self.min_confirmations as u32))?;

        let received_sats = unconfirmed.to_sat();
        let confirmations = if confirmed >= unconfirmed {
            self.min_confirmations
        } else if confirmed > Amount::ZERO {
            // Some confirmations but not enough
            self.min_confirmations / 2
        } else {
            0
        };

        let status = match received_sats.cmp(&order.expected_amount_sats) {
            std::cmp::Ordering::Equal => MatchStatus::Exact,
            std::cmp::Ordering::Less => MatchStatus::Underpaid,
            std::cmp::Ordering::Greater => MatchStatus::Overpaid,
        };

        Ok(Some(MatchResult {
            order_id: order.order_id,
            txid: String::new(), // Would need additional lookup
            received_amount_sats: received_sats,
            expected_amount_sats: order.expected_amount_sats,
            confirmations,
            status,
        }))
    }

    /// Filter matches by confirmation count
    pub fn filter_confirmed<'a>(&self, matches: &'a [MatchResult]) -> Vec<&'a MatchResult> {
        matches
            .iter()
            .filter(|m| m.confirmations >= self.min_confirmations)
            .collect()
    }

    /// Group matches by status
    pub fn group_by_status(&self, matches: &[MatchResult]) -> MatchGroups {
        let mut groups = MatchGroups::default();

        for m in matches {
            match m.status {
                MatchStatus::Exact => groups.exact.push(m.clone()),
                MatchStatus::Underpaid => groups.underpaid.push(m.clone()),
                MatchStatus::Overpaid => groups.overpaid.push(m.clone()),
            }
        }

        groups
    }
}

/// Grouped match results by status
#[derive(Debug, Default)]
pub struct MatchGroups {
    /// Exact matches
    pub exact: Vec<MatchResult>,
    /// Underpaid matches
    pub underpaid: Vec<MatchResult>,
    /// Overpaid matches
    pub overpaid: Vec<MatchResult>,
}

impl MatchGroups {
    /// Check if there are any matches
    pub fn is_empty(&self) -> bool {
        self.exact.is_empty() && self.underpaid.is_empty() && self.overpaid.is_empty()
    }

    /// Get total count of matches
    pub fn total_count(&self) -> usize {
        self.exact.len() + self.underpaid.len() + self.overpaid.len()
    }

    /// Get all successful matches (exact and overpaid)
    pub fn successful(&self) -> impl Iterator<Item = &MatchResult> {
        self.exact.iter().chain(self.overpaid.iter())
    }
}
