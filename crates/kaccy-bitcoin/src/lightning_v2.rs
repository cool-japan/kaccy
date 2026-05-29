//! Lightning Network v2 - Advanced Features
//!
//! This module provides advanced Lightning Network capabilities including:
//! - Dual-funded channels
//! - Channel splicing
//! - Zero-confirmation channels
//! - Multi-path payments (MPP)
//! - BOLT 12 Offers
//! - Advanced routing strategies

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{BitcoinError, Result};
#[allow(unused_imports)]
use crate::lightning::{ChannelPoint, LightningProvider, NodeInfo, PaymentStatus};

/// Extended Lightning provider with v2 features
#[async_trait]
pub trait LightningV2Provider: LightningProvider {
    /// Open a dual-funded channel
    async fn open_dual_funded_channel(
        &self,
        request: DualFundedChannelRequest,
    ) -> Result<ChannelPoint>;

    /// Splice funds into/out of a channel
    async fn splice_channel(&self, request: SpliceRequest) -> Result<SpliceResult>;

    /// Accept a zero-conf channel
    async fn accept_zero_conf_channel(&self, channel_point: &ChannelPoint) -> Result<()>;

    /// Pay using multi-path payment
    async fn pay_multipath(&self, request: MultiPathPaymentRequest) -> Result<MultiPathPayment>;

    /// Create a BOLT 12 offer
    async fn create_offer(&self, request: OfferRequest) -> Result<Offer>;

    /// Pay a BOLT 12 offer
    async fn pay_offer(&self, offer_string: &str, amount_msat: Option<u64>)
    -> Result<OfferPayment>;

    /// Get routing suggestions for a payment
    async fn get_route_suggestions(
        &self,
        destination: &str,
        amount_msat: u64,
    ) -> Result<Vec<RouteSuggestion>>;

    /// Optimize liquidity across channels
    async fn optimize_liquidity(&self) -> Result<LiquidityOptimization>;
}

/// Dual-funded channel request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualFundedChannelRequest {
    /// Node public key to connect to
    pub node_pubkey: String,
    /// Our funding amount in satoshis
    pub our_funding_sats: u64,
    /// Expected funding from counterparty in satoshis
    pub their_funding_sats: u64,
    /// Fee rate for funding transaction
    pub fee_rate_sat_per_vbyte: u64,
    /// Whether to make the channel private
    pub private: bool,
    /// Locktime for funding transaction
    pub locktime: Option<u32>,
}

impl DualFundedChannelRequest {
    /// Create a new dual-funded channel request
    pub fn new(
        node_pubkey: impl Into<String>,
        our_funding_sats: u64,
        their_funding_sats: u64,
    ) -> Self {
        Self {
            node_pubkey: node_pubkey.into(),
            our_funding_sats,
            their_funding_sats,
            fee_rate_sat_per_vbyte: 1,
            private: false,
            locktime: None,
        }
    }
}

/// Channel splice request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpliceRequest {
    /// Channel to splice
    pub channel_point: ChannelPoint,
    /// Amount to add (positive) or remove (negative) in satoshis
    pub amount_sats: i64,
    /// Fee rate for splice transaction
    pub fee_rate_sat_per_vbyte: u64,
}

/// Splice operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpliceResult {
    /// New channel point after splice
    pub new_channel_point: ChannelPoint,
    /// Splice transaction ID
    pub splice_txid: String,
    /// New channel capacity
    pub new_capacity_sats: u64,
    /// Whether splice is adding or removing funds
    pub splice_type: SpliceType,
}

/// Type of splice operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpliceType {
    /// Adding funds to channel (splice-in)
    SpliceIn,
    /// Removing funds from channel (splice-out)
    SpliceOut,
}

/// Multi-path payment request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPathPaymentRequest {
    /// Destination node public key
    pub destination: String,
    /// Total amount to pay in millisatoshis
    pub amount_msat: u64,
    /// Payment hash
    pub payment_hash: String,
    /// Payment secret
    pub payment_secret: Option<String>,
    /// Maximum fee in millisatoshis
    pub max_fee_msat: u64,
    /// Maximum number of paths to use
    pub max_paths: u32,
    /// Timeout for payment
    pub timeout_secs: u32,
}

impl MultiPathPaymentRequest {
    /// Create a new multi-path payment request
    pub fn new(
        destination: impl Into<String>,
        amount_msat: u64,
        payment_hash: impl Into<String>,
    ) -> Self {
        Self {
            destination: destination.into(),
            amount_msat,
            payment_hash: payment_hash.into(),
            payment_secret: None,
            max_fee_msat: amount_msat / 100, // 1% default
            max_paths: 4,
            timeout_secs: 60,
        }
    }
}

/// Multi-path payment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiPathPayment {
    /// Payment hash
    pub payment_hash: String,
    /// Payment preimage
    pub payment_preimage: String,
    /// Total amount sent
    pub amount_msat: u64,
    /// Total fees paid
    pub fee_msat: u64,
    /// Individual payment paths
    pub paths: Vec<PaymentPath>,
    /// Payment status
    pub status: PaymentStatus,
    /// Time taken for payment
    pub duration_ms: u64,
}

/// Individual payment path in multi-path payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentPath {
    /// Amount sent on this path
    pub amount_msat: u64,
    /// Fee for this path
    pub fee_msat: u64,
    /// Number of hops
    pub num_hops: u32,
    /// Success status of this path
    pub succeeded: bool,
}

/// BOLT 12 Offer request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferRequest {
    /// Description of what's being offered
    pub description: String,
    /// Amount in millisatoshis (None for "any amount" offers)
    pub amount_msat: Option<u64>,
    /// Issuer identity
    pub issuer: Option<String>,
    /// Supported features
    pub features: Vec<u64>,
    /// Absolute expiry timestamp
    pub absolute_expiry: Option<u64>,
    /// Paths to the offer issuer
    pub paths: Vec<BlindedPath>,
    /// Quantity limits
    pub quantity_max: Option<u64>,
}

impl OfferRequest {
    /// Create a new offer request with the given description
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            amount_msat: None,
            issuer: None,
            features: vec![],
            absolute_expiry: None,
            paths: vec![],
            quantity_max: None,
        }
    }

    /// Set a fixed amount for this offer in millisatoshis
    pub fn with_amount(mut self, amount_msat: u64) -> Self {
        self.amount_msat = Some(amount_msat);
        self
    }

    /// Set the issuer identifier for this offer
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(issuer.into());
        self
    }

    /// Set the offer expiry duration in seconds from now
    pub fn with_expiry(mut self, expiry_secs: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();
        self.absolute_expiry = Some(now + expiry_secs);
        self
    }
}

/// BOLT 12 Offer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    /// Offer ID
    pub offer_id: String,
    /// Encoded offer string (starts with "lno1")
    pub offer_string: String,
    /// Description
    pub description: String,
    /// Amount (if fixed)
    pub amount_msat: Option<u64>,
    /// Issuer
    pub issuer: Option<String>,
    /// Expiry timestamp
    pub absolute_expiry: Option<u64>,
    /// Whether the offer is active
    pub active: bool,
}

impl Offer {
    /// Check if offer is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expiry) = self.absolute_expiry {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("SystemTime should be after UNIX_EPOCH")
                .as_secs();
            now > expiry
        } else {
            false
        }
    }
}

/// BOLT 12 Offer payment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferPayment {
    /// Payment hash
    pub payment_hash: String,
    /// Payment preimage
    pub payment_preimage: Option<String>,
    /// Amount paid
    pub amount_msat: u64,
    /// Fee paid
    pub fee_msat: u64,
    /// Payment status
    pub status: PaymentStatus,
}

/// Blinded path for privacy-preserving routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedPath {
    /// Introduction node
    pub introduction_node: String,
    /// Blinding point
    pub blinding_point: String,
    /// Blinded hops
    pub blinded_hops: Vec<BlindedHop>,
}

/// Blinded hop in a path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedHop {
    /// Blinded node ID
    pub blinded_node_id: String,
    /// Encrypted data
    pub encrypted_data: Vec<u8>,
}

/// Route suggestion for payments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteSuggestion {
    /// Route hops
    pub hops: Vec<RouteHop>,
    /// Total fees
    pub total_fee_msat: u64,
    /// Success probability (0.0 - 1.0)
    pub success_probability: f64,
    /// Estimated time lock delta
    pub timelock_delta: u32,
    /// Route score (higher is better)
    pub score: f64,
}

/// Route hop with detailed information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    /// Node public key
    pub pubkey: String,
    /// Short channel ID
    pub short_channel_id: String,
    /// Amount to forward
    pub amount_msat: u64,
    /// Fee
    pub fee_msat: u64,
    /// CLTV expiry delta
    pub cltv_expiry_delta: u32,
    /// Channel capacity
    pub capacity_sats: Option<u64>,
}

/// Liquidity optimization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityOptimization {
    /// Recommended rebalancing operations
    pub rebalance_operations: Vec<RebalanceOp>,
    /// Channels that need more inbound liquidity
    pub needs_inbound: Vec<String>,
    /// Channels that need more outbound liquidity
    pub needs_outbound: Vec<String>,
    /// Overall liquidity score (0-100)
    pub liquidity_score: u32,
}

/// Rebalancing operation recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceOp {
    /// Source channel
    pub from_channel: String,
    /// Destination channel
    pub to_channel: String,
    /// Amount to rebalance
    pub amount_sats: u64,
    /// Expected cost
    pub estimated_fee_msat: u64,
}

/// Advanced routing strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingStrategy {
    /// Minimize fees
    LowFee,
    /// Maximize success probability
    HighReliability,
    /// Minimize time locks
    FastSettle,
    /// Balance between fee, reliability, and speed
    Balanced,
    /// Maximize privacy (more hops)
    MaxPrivacy,
}

/// Liquidity manager for automatic channel management
pub struct LiquidityManager {
    provider: Box<dyn LightningV2Provider>,
    config: LiquidityConfig,
}

impl LiquidityManager {
    /// Create a new liquidity manager with the given provider and configuration
    pub fn new(provider: Box<dyn LightningV2Provider>, config: LiquidityConfig) -> Self {
        Self { provider, config }
    }

    /// Automatically optimize liquidity across all channels
    pub async fn auto_optimize(&self) -> Result<LiquidityOptimization> {
        self.provider.optimize_liquidity().await
    }

    /// Check if we need to open more channels
    pub async fn needs_more_channels(&self) -> Result<bool> {
        let info = self.provider.get_info().await?;
        Ok(info.num_active_channels < self.config.min_channels)
    }

    /// Execute recommended rebalancing operations
    pub async fn execute_rebalancing(&self, ops: Vec<RebalanceOp>) -> Result<Vec<RebalanceResult>> {
        let mut results = Vec::new();

        for op in ops {
            let result = self.execute_single_rebalance(&op).await;
            results.push(RebalanceResult {
                operation: op.clone(),
                success: result.is_ok(),
                error: result.err().map(|e| e.to_string()),
            });
        }

        Ok(results)
    }

    #[allow(dead_code)]
    async fn execute_single_rebalance(&self, op: &RebalanceOp) -> Result<()> {
        // Create a circular payment to rebalance
        // This would involve creating an invoice on the destination channel
        // and paying it through the source channel
        // For now, this is a placeholder
        let _ = op;
        Err(BitcoinError::Wallet(
            "Rebalancing not yet implemented".to_string(),
        ))
    }
}

/// Liquidity management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityConfig {
    /// Minimum number of channels to maintain
    pub min_channels: u32,
    /// Target inbound capacity per channel (satoshis)
    pub target_inbound_sats: u64,
    /// Target outbound capacity per channel (satoshis)
    pub target_outbound_sats: u64,
    /// Maximum fee for rebalancing (satoshis)
    pub max_rebalance_fee_sats: u64,
    /// Rebalancing interval
    pub rebalance_interval: Duration,
}

impl Default for LiquidityConfig {
    fn default() -> Self {
        Self {
            min_channels: 3,
            target_inbound_sats: 500_000,
            target_outbound_sats: 500_000,
            max_rebalance_fee_sats: 1000,
            rebalance_interval: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Result of a rebalancing operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebalanceResult {
    /// The operation that was attempted
    pub operation: RebalanceOp,
    /// Whether it succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Multi-path payment router
pub struct MultiPathRouter {
    provider: Box<dyn LightningV2Provider>,
    strategy: RoutingStrategy,
}

impl MultiPathRouter {
    /// Create a new multi-path router with the given provider and routing strategy
    pub fn new(provider: Box<dyn LightningV2Provider>, strategy: RoutingStrategy) -> Self {
        Self { provider, strategy }
    }

    /// Find optimal routes for a multi-path payment
    pub async fn find_routes(
        &self,
        destination: &str,
        amount_msat: u64,
        max_paths: u32,
    ) -> Result<Vec<RouteSuggestion>> {
        let suggestions = self
            .provider
            .get_route_suggestions(destination, amount_msat)
            .await?;

        // Filter and sort based on strategy
        let mut filtered = self.filter_by_strategy(suggestions);
        filtered.truncate(max_paths as usize);

        Ok(filtered)
    }

    fn filter_by_strategy(&self, mut routes: Vec<RouteSuggestion>) -> Vec<RouteSuggestion> {
        routes.sort_by(|a, b| {
            let score_a = self.calculate_route_score(a);
            let score_b = self.calculate_route_score(b);
            score_b
                .partial_cmp(&score_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        routes
    }

    fn calculate_route_score(&self, route: &RouteSuggestion) -> f64 {
        match self.strategy {
            RoutingStrategy::LowFee => 1.0 / (route.total_fee_msat as f64 + 1.0),
            RoutingStrategy::HighReliability => route.success_probability,
            RoutingStrategy::FastSettle => 1.0 / (route.timelock_delta as f64 + 1.0),
            RoutingStrategy::Balanced => {
                route.success_probability * 0.5
                    + (1.0 / (route.total_fee_msat as f64 + 1.0)) * 0.3
                    + (1.0 / (route.timelock_delta as f64 + 1.0)) * 0.2
            }
            RoutingStrategy::MaxPrivacy => route.hops.len() as f64,
        }
    }

    /// Execute a multi-path payment
    pub async fn pay(&self, request: MultiPathPaymentRequest) -> Result<MultiPathPayment> {
        self.provider.pay_multipath(request).await
    }
}

/// Zero-conf channel manager
pub struct ZeroConfManager {
    provider: Box<dyn LightningV2Provider>,
    trusted_peers: Vec<String>,
}

impl ZeroConfManager {
    /// Create a new zero-conf manager with no trusted peers
    pub fn new(provider: Box<dyn LightningV2Provider>) -> Self {
        Self {
            provider,
            trusted_peers: Vec::new(),
        }
    }

    /// Add a trusted peer for zero-conf channels
    pub fn add_trusted_peer(&mut self, pubkey: impl Into<String>) {
        self.trusted_peers.push(pubkey.into());
    }

    /// Check if a peer is trusted for zero-conf
    pub fn is_trusted(&self, pubkey: &str) -> bool {
        self.trusted_peers.contains(&pubkey.to_string())
    }

    /// Accept a zero-conf channel from a trusted peer
    pub async fn accept_channel(
        &self,
        channel_point: &ChannelPoint,
        peer_pubkey: &str,
    ) -> Result<()> {
        if !self.is_trusted(peer_pubkey) {
            return Err(BitcoinError::Wallet(
                "Peer not trusted for zero-conf channels".to_string(),
            ));
        }

        self.provider.accept_zero_conf_channel(channel_point).await
    }
}

/// BOLT 12 offer manager
pub struct OfferManager {
    provider: Box<dyn LightningV2Provider>,
    active_offers: HashMap<String, Offer>,
}

impl OfferManager {
    /// Create a new offer manager with no active offers
    pub fn new(provider: Box<dyn LightningV2Provider>) -> Self {
        Self {
            provider,
            active_offers: HashMap::new(),
        }
    }

    /// Create a new offer
    pub async fn create_offer(&mut self, request: OfferRequest) -> Result<Offer> {
        let offer = self.provider.create_offer(request).await?;
        self.active_offers
            .insert(offer.offer_id.clone(), offer.clone());
        Ok(offer)
    }

    /// Get an active offer
    pub fn get_offer(&self, offer_id: &str) -> Option<&Offer> {
        self.active_offers.get(offer_id)
    }

    /// Deactivate an offer
    pub fn deactivate_offer(&mut self, offer_id: &str) -> Result<()> {
        if let Some(offer) = self.active_offers.get_mut(offer_id) {
            offer.active = false;
            Ok(())
        } else {
            Err(BitcoinError::Wallet("Offer not found".to_string()))
        }
    }

    /// Pay an offer
    pub async fn pay_offer(
        &self,
        offer_string: &str,
        amount_msat: Option<u64>,
    ) -> Result<OfferPayment> {
        self.provider.pay_offer(offer_string, amount_msat).await
    }

    /// Clean up expired offers
    pub fn cleanup_expired(&mut self) -> usize {
        let initial_count = self.active_offers.len();
        self.active_offers.retain(|_, offer| !offer.is_expired());
        initial_count - self.active_offers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dual_funded_channel_request() {
        let request = DualFundedChannelRequest::new("03abc...".to_string(), 1_000_000, 500_000);

        assert_eq!(request.our_funding_sats, 1_000_000);
        assert_eq!(request.their_funding_sats, 500_000);
    }

    #[test]
    fn test_multipath_payment_request() {
        let request =
            MultiPathPaymentRequest::new("03def...".to_string(), 100_000_000, "payment_hash_123");

        assert_eq!(request.amount_msat, 100_000_000);
        assert_eq!(request.max_paths, 4);
    }

    #[test]
    fn test_offer_builder() {
        let offer_request = OfferRequest::new("Test product")
            .with_amount(50_000_000)
            .with_issuer("Test Store")
            .with_expiry(86400);

        assert_eq!(offer_request.amount_msat, Some(50_000_000));
        assert_eq!(offer_request.issuer, Some("Test Store".to_string()));
        assert!(offer_request.absolute_expiry.is_some());
    }

    #[test]
    fn test_routing_strategy_scoring() {
        let route = RouteSuggestion {
            hops: vec![],
            total_fee_msat: 1000,
            success_probability: 0.95,
            timelock_delta: 40,
            score: 0.0,
        };

        let router = MultiPathRouter {
            provider: Box::new(MockProvider),
            strategy: RoutingStrategy::HighReliability,
        };

        let score = router.calculate_route_score(&route);
        assert!((score - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_liquidity_config_default() {
        let config = LiquidityConfig::default();
        assert_eq!(config.min_channels, 3);
        assert_eq!(config.target_inbound_sats, 500_000);
    }

    #[test]
    fn test_splice_type() {
        assert_eq!(SpliceType::SpliceIn, SpliceType::SpliceIn);
        assert_ne!(SpliceType::SpliceIn, SpliceType::SpliceOut);
    }

    #[test]
    fn test_offer_expiry() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        let expired_offer = Offer {
            offer_id: "offer1".to_string(),
            offer_string: "lno1...".to_string(),
            description: "Test".to_string(),
            amount_msat: Some(1000),
            issuer: None,
            absolute_expiry: Some(now - 3600), // Expired 1 hour ago
            active: true,
        };

        assert!(expired_offer.is_expired());

        let active_offer = Offer {
            offer_id: "offer2".to_string(),
            offer_string: "lno1...".to_string(),
            description: "Test".to_string(),
            amount_msat: Some(1000),
            issuer: None,
            absolute_expiry: Some(now + 3600), // Expires in 1 hour
            active: true,
        };

        assert!(!active_offer.is_expired());
    }

    // Mock provider for testing
    struct MockProvider;

    #[async_trait]
    impl LightningProvider for MockProvider {
        async fn get_info(&self) -> Result<NodeInfo> {
            Ok(NodeInfo {
                pubkey: "mock_node_pubkey".into(),
                alias: "MockNode".into(),
                num_active_channels: 0,
                num_pending_channels: 0,
                num_peers: 0,
                block_height: 800_000,
                synced_to_chain: true,
                version: "mock-1.0".into(),
                network: "mainnet".into(),
            })
        }

        async fn create_invoice(
            &self,
            request: crate::lightning::InvoiceRequest,
        ) -> Result<crate::lightning::Invoice> {
            Ok(crate::lightning::Invoice {
                payment_hash: "mock_payment_hash".into(),
                payment_preimage: None,
                bolt11: "lnbc1mock".into(),
                amount_msat: request.amount_msat,
                description: request.description,
                created_at: 0,
                expires_at: 3600,
                status: crate::lightning::InvoiceStatus::Open,
                amount_received_msat: None,
                settled_at: None,
                metadata: std::collections::HashMap::new(),
            })
        }

        async fn get_invoice(&self, payment_hash: &str) -> Result<crate::lightning::Invoice> {
            Ok(crate::lightning::Invoice {
                payment_hash: payment_hash.to_string(),
                payment_preimage: None,
                bolt11: "lnbc1mock".into(),
                amount_msat: 0,
                description: String::new(),
                created_at: 0,
                expires_at: 3600,
                status: crate::lightning::InvoiceStatus::Open,
                amount_received_msat: None,
                settled_at: None,
                metadata: std::collections::HashMap::new(),
            })
        }

        async fn pay_invoice(
            &self,
            _bolt11: &str,
            _max_fee_msat: Option<u64>,
        ) -> Result<crate::lightning::Payment> {
            Ok(crate::lightning::Payment {
                payment_hash: "mock_payment_hash".into(),
                payment_preimage: "mock_preimage".into(),
                amount_msat: 1_000_000,
                fee_msat: 1_000,
                status: PaymentStatus::Succeeded,
                created_at: 0,
                num_hops: 3,
                route: None,
            })
        }

        async fn get_balance(&self) -> Result<crate::lightning::ChannelBalance> {
            Ok(crate::lightning::ChannelBalance {
                local_balance_msat: 0,
                remote_balance_msat: 0,
                pending_local_msat: 0,
                pending_remote_msat: 0,
                unsettled_local_msat: 0,
                unsettled_remote_msat: 0,
            })
        }

        async fn list_channels(&self) -> Result<Vec<crate::lightning::Channel>> {
            Ok(vec![])
        }

        async fn open_channel(
            &self,
            _request: crate::lightning::OpenChannelRequest,
        ) -> Result<ChannelPoint> {
            Ok(ChannelPoint {
                txid: "mock_funding_txid".into(),
                output_index: 0,
            })
        }

        async fn close_channel(
            &self,
            _channel_point: &ChannelPoint,
            _force: bool,
        ) -> Result<String> {
            Ok("mock_closing_txid".into())
        }

        async fn subscribe_invoices(&self) -> Result<crate::lightning::InvoiceSubscription> {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            drop(tx);
            Ok(crate::lightning::InvoiceSubscription::new(rx))
        }
    }

    #[async_trait]
    impl LightningV2Provider for MockProvider {
        async fn open_dual_funded_channel(
            &self,
            _request: DualFundedChannelRequest,
        ) -> Result<ChannelPoint> {
            Ok(ChannelPoint {
                txid: "mock_dual_funded_txid".into(),
                output_index: 0,
            })
        }

        async fn splice_channel(&self, _request: SpliceRequest) -> Result<SpliceResult> {
            Ok(SpliceResult {
                new_channel_point: ChannelPoint {
                    txid: "mock_splice_txid".into(),
                    output_index: 0,
                },
                splice_txid: "mock_splice_txid".into(),
                new_capacity_sats: 0,
                splice_type: SpliceType::SpliceIn,
            })
        }

        async fn accept_zero_conf_channel(&self, _channel_point: &ChannelPoint) -> Result<()> {
            Ok(())
        }

        async fn pay_multipath(
            &self,
            _request: MultiPathPaymentRequest,
        ) -> Result<MultiPathPayment> {
            Ok(MultiPathPayment {
                payment_hash: "mock_mpay_hash".into(),
                payment_preimage: "mock_mpay_preimage".into(),
                amount_msat: 1_000_000,
                fee_msat: 1_000,
                paths: vec![],
                status: PaymentStatus::Succeeded,
                duration_ms: 0,
            })
        }

        async fn create_offer(&self, request: OfferRequest) -> Result<Offer> {
            Ok(Offer {
                offer_id: "mock_offer_id".into(),
                offer_string: "lno1mock".into(),
                description: request.description,
                amount_msat: None,
                issuer: None,
                absolute_expiry: None,
                active: true,
            })
        }

        async fn pay_offer(
            &self,
            _offer_string: &str,
            amount_msat: Option<u64>,
        ) -> Result<OfferPayment> {
            Ok(OfferPayment {
                payment_hash: "mock_offer_pay_hash".into(),
                payment_preimage: Some("mock_offer_preimage".into()),
                amount_msat: amount_msat.unwrap_or(0),
                fee_msat: 0,
                status: PaymentStatus::Succeeded,
            })
        }

        async fn get_route_suggestions(
            &self,
            _destination: &str,
            _amount_msat: u64,
        ) -> Result<Vec<RouteSuggestion>> {
            Ok(vec![])
        }

        async fn optimize_liquidity(&self) -> Result<LiquidityOptimization> {
            Ok(LiquidityOptimization {
                rebalance_operations: vec![],
                needs_inbound: vec![],
                needs_outbound: vec![],
                liquidity_score: 100,
            })
        }
    }

    #[tokio::test]
    async fn test_mock_provider_basic_queries() {
        let provider = MockProvider;
        provider.get_info().await.expect("get_info should succeed");
        let balance = provider
            .get_balance()
            .await
            .expect("get_balance should succeed");
        assert_eq!(balance.local_balance_msat, 0);
        let channels = provider
            .list_channels()
            .await
            .expect("list_channels should succeed");
        assert!(channels.is_empty());
    }

    #[tokio::test]
    async fn test_mock_provider_invoice_lifecycle() {
        let provider = MockProvider;
        let inv = provider
            .create_invoice(crate::lightning::InvoiceRequest::new(1_000, "test"))
            .await
            .expect("create_invoice should succeed");
        assert!(!inv.payment_hash.is_empty());

        let pay = provider
            .pay_invoice(&inv.bolt11, None)
            .await
            .expect("pay_invoice should succeed");
        assert_eq!(pay.status, PaymentStatus::Succeeded);
    }

    #[tokio::test]
    async fn test_mock_provider_v2_operations() {
        let provider = MockProvider;
        let offer = provider
            .create_offer(OfferRequest::new("test offer"))
            .await
            .expect("create_offer should succeed");
        assert!(offer.active);

        let liquidity = provider
            .optimize_liquidity()
            .await
            .expect("optimize_liquidity should succeed");
        assert_eq!(liquidity.liquidity_score, 100);
    }
}
