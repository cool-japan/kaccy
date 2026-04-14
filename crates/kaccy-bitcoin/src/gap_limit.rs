//! Gap limit tracking for BIP44 address discovery

use bitcoin::Address;
use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

use crate::client::BitcoinClient;
use crate::error::Result;

/// Gap limit configuration (BIP44)
#[derive(Debug, Clone)]
pub struct GapLimitConfig {
    /// Maximum number of consecutive unused addresses (default: 20 per BIP44)
    pub gap_limit: u32,
    /// Whether to scan both external and internal chains
    pub scan_change_addresses: bool,
    /// Minimum confirmations to consider an address "used"
    pub min_confirmations: u32,
}

impl Default for GapLimitConfig {
    fn default() -> Self {
        Self {
            gap_limit: 20, // BIP44 standard
            scan_change_addresses: true,
            min_confirmations: 0, // Even unconfirmed txs count as "used"
        }
    }
}

/// Address usage information
#[derive(Debug, Clone)]
pub struct AddressUsage {
    /// The Bitcoin address string
    pub address: String,
    /// Derivation index for this address
    pub index: u32,
    /// Whether this is an external (receiving) address
    pub is_external: bool,
    /// Whether any transactions have been seen for this address
    pub has_transactions: bool,
    /// Total satoshis received at this address
    pub received_amount: u64,
    /// Number of confirmations of the most recent transaction
    pub confirmations: u32,
}

/// Gap limit tracker
pub struct GapLimitTracker {
    config: GapLimitConfig,
    /// Highest used index for external chain
    highest_used_external: u32,
    /// Highest used index for internal (change) chain
    highest_used_internal: u32,
    /// Set of used addresses
    used_addresses: HashSet<String>,
    /// Address usage details
    address_usage: HashMap<String, AddressUsage>,
}

impl GapLimitTracker {
    /// Create a new gap limit tracker
    pub fn new(config: GapLimitConfig) -> Self {
        Self {
            config,
            highest_used_external: 0,
            highest_used_internal: 0,
            used_addresses: HashSet::new(),
            address_usage: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(GapLimitConfig::default())
    }

    /// Check if an address has been used
    pub async fn is_address_used(&self, client: &BitcoinClient, address: &Address) -> Result<bool> {
        let received =
            client.get_received_by_address(address, Some(self.config.min_confirmations))?;
        Ok(received.to_sat() > 0)
    }

    /// Scan for used addresses starting from a given index
    pub async fn scan_addresses<F>(
        &mut self,
        client: &BitcoinClient,
        is_external: bool,
        start_index: u32,
        address_generator: F,
    ) -> Result<Vec<AddressUsage>>
    where
        F: Fn(u32) -> Result<Address>,
    {
        let mut found_addresses = Vec::new();
        let mut consecutive_unused = 0;
        let mut current_index = start_index;

        loop {
            let address = address_generator(current_index)?;

            // Check if address has received funds
            let received =
                client.get_received_by_address(&address, Some(self.config.min_confirmations))?;
            let received_sats = received.to_sat();
            let has_transactions = received_sats > 0;

            if has_transactions {
                // Found a used address
                let usage = AddressUsage {
                    address: address.to_string(),
                    index: current_index,
                    is_external,
                    has_transactions,
                    received_amount: received_sats,
                    confirmations: self.config.min_confirmations,
                };

                found_addresses.push(usage.clone());
                self.address_usage.insert(address.to_string(), usage);
                self.used_addresses.insert(address.to_string());

                // Update highest used index
                if is_external {
                    self.highest_used_external = self.highest_used_external.max(current_index);
                } else {
                    self.highest_used_internal = self.highest_used_internal.max(current_index);
                }

                consecutive_unused = 0;
                debug!(
                    address = %address,
                    index = current_index,
                    is_external = is_external,
                    received_sats = received_sats,
                    "Found used address"
                );
            } else {
                consecutive_unused += 1;
                debug!(
                    address = %address,
                    index = current_index,
                    consecutive_unused = consecutive_unused,
                    "Address unused"
                );
            }

            // Check if we've hit the gap limit
            if consecutive_unused >= self.config.gap_limit {
                debug!(
                    consecutive_unused = consecutive_unused,
                    gap_limit = self.config.gap_limit,
                    "Reached gap limit, stopping scan"
                );
                break;
            }

            current_index += 1;

            // Safety check to prevent infinite loops
            if current_index > 100_000 {
                warn!("Scan reached index 100,000, stopping for safety");
                break;
            }
        }

        Ok(found_addresses)
    }

    /// Get the next safe index to use for address generation
    pub fn get_next_safe_index(&self, is_external: bool) -> u32 {
        if is_external {
            self.highest_used_external + 1
        } else {
            self.highest_used_internal + 1
        }
    }

    /// Check if gap limit is being respected for a given index
    pub fn validate_gap_limit(&self, index: u32, is_external: bool) -> bool {
        let highest_used = if is_external {
            self.highest_used_external
        } else {
            self.highest_used_internal
        };

        index <= highest_used + self.config.gap_limit
    }

    /// Get address usage statistics
    pub fn get_usage_stats(&self) -> GapLimitStats {
        let external_used = self
            .address_usage
            .values()
            .filter(|u| u.is_external && u.has_transactions)
            .count() as u32;

        let internal_used = self
            .address_usage
            .values()
            .filter(|u| !u.is_external && u.has_transactions)
            .count() as u32;

        let total_received = self.address_usage.values().map(|u| u.received_amount).sum();

        GapLimitStats {
            highest_used_external: self.highest_used_external,
            highest_used_internal: self.highest_used_internal,
            total_addresses_scanned: self.address_usage.len() as u32,
            external_addresses_used: external_used,
            internal_addresses_used: internal_used,
            total_received_sats: total_received,
            gap_limit: self.config.gap_limit,
        }
    }

    /// Check if an address is marked as used
    pub fn is_used(&self, address: &str) -> bool {
        self.used_addresses.contains(address)
    }

    /// Get address usage details
    pub fn get_address_usage(&self, address: &str) -> Option<&AddressUsage> {
        self.address_usage.get(address)
    }

    /// Mark an address as used manually (e.g., when generating a new address for an order)
    pub fn mark_as_used(&mut self, address: String, index: u32, is_external: bool) {
        let usage = AddressUsage {
            address: address.clone(),
            index,
            is_external,
            has_transactions: true, // Mark as used
            received_amount: 0,
            confirmations: 0,
        };

        self.address_usage.insert(address.clone(), usage);
        self.used_addresses.insert(address);

        if is_external {
            self.highest_used_external = self.highest_used_external.max(index);
        } else {
            self.highest_used_internal = self.highest_used_internal.max(index);
        }
    }

    /// Reset the tracker (useful for re-scanning)
    pub fn reset(&mut self) {
        self.highest_used_external = 0;
        self.highest_used_internal = 0;
        self.used_addresses.clear();
        self.address_usage.clear();
    }
}

/// Gap limit statistics
#[derive(Debug, Clone)]
pub struct GapLimitStats {
    /// Highest derivation index used on the external chain
    pub highest_used_external: u32,
    /// Highest derivation index used on the internal (change) chain
    pub highest_used_internal: u32,
    /// Total number of addresses scanned
    pub total_addresses_scanned: u32,
    /// Number of external addresses with activity
    pub external_addresses_used: u32,
    /// Number of internal addresses with activity
    pub internal_addresses_used: u32,
    /// Total satoshis received across all scanned addresses
    pub total_received_sats: u64,
    /// Configured gap limit value
    pub gap_limit: u32,
}

impl GapLimitStats {
    /// Check if we're approaching the gap limit
    pub fn is_near_gap_limit(&self, current_index: u32, is_external: bool) -> bool {
        let highest_used = if is_external {
            self.highest_used_external
        } else {
            self.highest_used_internal
        };

        let gap = current_index.saturating_sub(highest_used);
        gap >= self.gap_limit * 3 / 4 // Warn at 75% of gap limit
    }
}

/// Address discovery manager combining gap limit with HD wallet
pub struct AddressDiscovery {
    tracker: GapLimitTracker,
}

impl AddressDiscovery {
    /// Create a new address discovery manager
    pub fn new(config: GapLimitConfig) -> Self {
        Self {
            tracker: GapLimitTracker::new(config),
        }
    }

    /// Create with defaults
    pub fn with_defaults() -> Self {
        Self {
            tracker: GapLimitTracker::with_defaults(),
        }
    }

    /// Discover all used addresses for a wallet
    pub async fn discover_addresses<F>(
        &mut self,
        client: &BitcoinClient,
        external_generator: F,
        internal_generator: Option<F>,
    ) -> Result<DiscoveryResult>
    where
        F: Fn(u32) -> Result<Address>,
    {
        // Scan external chain (receiving addresses)
        let external_addresses = self
            .tracker
            .scan_addresses(client, true, 0, &external_generator)
            .await?;

        // Scan internal chain (change addresses) if requested
        let internal_addresses = if self.tracker.config.scan_change_addresses {
            if let Some(internal_gen) = internal_generator {
                self.tracker
                    .scan_addresses(client, false, 0, internal_gen)
                    .await?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let stats = self.tracker.get_usage_stats();

        Ok(DiscoveryResult {
            external_addresses,
            internal_addresses,
            stats,
        })
    }

    /// Get the tracker for manual operations
    pub fn tracker(&self) -> &GapLimitTracker {
        &self.tracker
    }

    /// Get mutable tracker
    pub fn tracker_mut(&mut self) -> &mut GapLimitTracker {
        &mut self.tracker
    }
}

/// Result of address discovery
#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    /// Discovered external (receiving) addresses with activity
    pub external_addresses: Vec<AddressUsage>,
    /// Discovered internal (change) addresses with activity
    pub internal_addresses: Vec<AddressUsage>,
    /// Summary statistics for the discovery scan
    pub stats: GapLimitStats,
}

impl DiscoveryResult {
    /// Get all discovered addresses
    pub fn all_addresses(&self) -> Vec<&AddressUsage> {
        self.external_addresses
            .iter()
            .chain(self.internal_addresses.iter())
            .collect()
    }

    /// Get total amount received across all addresses
    pub fn total_received(&self) -> u64 {
        self.stats.total_received_sats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_limit_config_defaults() {
        let config = GapLimitConfig::default();
        assert_eq!(config.gap_limit, 20);
        assert!(config.scan_change_addresses);
        assert_eq!(config.min_confirmations, 0);
    }

    #[test]
    fn test_gap_limit_tracker_initialization() {
        let tracker = GapLimitTracker::with_defaults();
        assert_eq!(tracker.highest_used_external, 0);
        assert_eq!(tracker.highest_used_internal, 0);
        assert!(tracker.used_addresses.is_empty());
    }

    #[test]
    fn test_mark_as_used() {
        let mut tracker = GapLimitTracker::with_defaults();

        tracker.mark_as_used("bc1q...".to_string(), 5, true);

        assert_eq!(tracker.highest_used_external, 5);
        assert!(tracker.is_used("bc1q..."));
        assert_eq!(tracker.get_next_safe_index(true), 6);
    }

    #[test]
    fn test_validate_gap_limit() {
        let mut tracker = GapLimitTracker::with_defaults();
        tracker.mark_as_used("bc1q1...".to_string(), 5, true);

        // Within gap limit (5 + 20 = 25)
        assert!(tracker.validate_gap_limit(25, true));

        // Beyond gap limit
        assert!(!tracker.validate_gap_limit(26, true));
    }

    #[test]
    fn test_gap_limit_stats() {
        let mut tracker = GapLimitTracker::with_defaults();

        tracker.mark_as_used("bc1q1...".to_string(), 0, true);
        tracker.mark_as_used("bc1q2...".to_string(), 1, true);
        tracker.mark_as_used("bc1q3...".to_string(), 0, false);

        let stats = tracker.get_usage_stats();

        assert_eq!(stats.highest_used_external, 1);
        assert_eq!(stats.highest_used_internal, 0);
        assert_eq!(stats.external_addresses_used, 2);
        assert_eq!(stats.internal_addresses_used, 1);
    }

    #[test]
    fn test_is_near_gap_limit() {
        let stats = GapLimitStats {
            highest_used_external: 10,
            highest_used_internal: 5,
            total_addresses_scanned: 50,
            external_addresses_used: 11,
            internal_addresses_used: 6,
            total_received_sats: 1_000_000,
            gap_limit: 20,
        };

        // 10 + 15 = 25, gap of 15 which is 75% of 20
        assert!(stats.is_near_gap_limit(25, true));

        // 10 + 10 = 20, gap of 10 which is 50% of 20
        assert!(!stats.is_near_gap_limit(20, true));
    }

    #[test]
    fn test_address_discovery_initialization() {
        let discovery = AddressDiscovery::with_defaults();
        assert_eq!(discovery.tracker().highest_used_external, 0);
    }

    #[test]
    fn test_discovery_result_all_addresses() {
        let external = vec![AddressUsage {
            address: "bc1q1...".to_string(),
            index: 0,
            is_external: true,
            has_transactions: true,
            received_amount: 100000,
            confirmations: 6,
        }];

        let internal = vec![AddressUsage {
            address: "bc1q2...".to_string(),
            index: 0,
            is_external: false,
            has_transactions: true,
            received_amount: 50000,
            confirmations: 3,
        }];

        let stats = GapLimitStats {
            highest_used_external: 0,
            highest_used_internal: 0,
            total_addresses_scanned: 2,
            external_addresses_used: 1,
            internal_addresses_used: 1,
            total_received_sats: 150000,
            gap_limit: 20,
        };

        let result = DiscoveryResult {
            external_addresses: external,
            internal_addresses: internal,
            stats,
        };

        assert_eq!(result.all_addresses().len(), 2);
        assert_eq!(result.total_received(), 150000);
    }

    #[test]
    fn test_tracker_reset() {
        let mut tracker = GapLimitTracker::with_defaults();

        tracker.mark_as_used("bc1q...".to_string(), 5, true);
        assert_eq!(tracker.highest_used_external, 5);

        tracker.reset();
        assert_eq!(tracker.highest_used_external, 0);
        assert!(tracker.used_addresses.is_empty());
    }
}
