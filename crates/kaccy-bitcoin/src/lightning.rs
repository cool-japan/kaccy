//! Lightning Network integration module
//!
//! This module provides Lightning Network capabilities for instant payments
//! using LND (Lightning Network Daemon) or CLN (Core Lightning).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use crate::error::{BitcoinError, Result};

/// Lightning Network provider trait
#[async_trait]
pub trait LightningProvider: Send + Sync {
    /// Get node info
    async fn get_info(&self) -> Result<NodeInfo>;

    /// Create an invoice
    async fn create_invoice(&self, request: InvoiceRequest) -> Result<Invoice>;

    /// Check invoice status
    async fn get_invoice(&self, payment_hash: &str) -> Result<Invoice>;

    /// Pay an invoice
    async fn pay_invoice(&self, bolt11: &str, max_fee_msat: Option<u64>) -> Result<Payment>;

    /// Get channel balance
    async fn get_balance(&self) -> Result<ChannelBalance>;

    /// List channels
    async fn list_channels(&self) -> Result<Vec<Channel>>;

    /// Open a channel
    async fn open_channel(&self, request: OpenChannelRequest) -> Result<ChannelPoint>;

    /// Close a channel
    async fn close_channel(&self, channel_point: &ChannelPoint, force: bool) -> Result<String>;

    /// Subscribe to invoice updates
    async fn subscribe_invoices(&self) -> Result<InvoiceSubscription>;
}

/// Node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node public key
    pub pubkey: String,
    /// Node alias
    pub alias: String,
    /// Number of active channels
    pub num_active_channels: u32,
    /// Number of pending channels
    pub num_pending_channels: u32,
    /// Number of peers
    pub num_peers: u32,
    /// Block height
    pub block_height: u64,
    /// Synced to chain
    pub synced_to_chain: bool,
    /// Version
    pub version: String,
    /// Network (mainnet, testnet, signet)
    pub network: String,
}

/// Invoice request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceRequest {
    /// Amount in millisatoshis
    pub amount_msat: u64,
    /// Invoice description
    pub description: String,
    /// Expiry time in seconds (default: 3600)
    pub expiry_secs: Option<u32>,
    /// Custom metadata (e.g., order_id)
    pub metadata: HashMap<String, String>,
    /// Private routing hints
    pub private: bool,
}

impl InvoiceRequest {
    /// Create a new invoice request
    pub fn new(amount_sats: u64, description: impl Into<String>) -> Self {
        Self {
            amount_msat: amount_sats * 1000,
            description: description.into(),
            expiry_secs: Some(3600), // 1 hour default
            metadata: HashMap::new(),
            private: false,
        }
    }

    /// Set expiry time
    pub fn expiry(mut self, secs: u32) -> Self {
        self.expiry_secs = Some(secs);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set as private (include routing hints)
    pub fn private(mut self, is_private: bool) -> Self {
        self.private = is_private;
        self
    }
}

/// Lightning invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    /// Payment hash (hex)
    pub payment_hash: String,
    /// Payment preimage (hex, only available after payment)
    pub payment_preimage: Option<String>,
    /// BOLT11 encoded invoice string
    pub bolt11: String,
    /// Amount in millisatoshis
    pub amount_msat: u64,
    /// Description
    pub description: String,
    /// Creation timestamp (unix)
    pub created_at: u64,
    /// Expiry timestamp (unix)
    pub expires_at: u64,
    /// Invoice status
    pub status: InvoiceStatus,
    /// Amount received (if paid)
    pub amount_received_msat: Option<u64>,
    /// Settlement timestamp (if paid)
    pub settled_at: Option<u64>,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl Invoice {
    /// Check if invoice is expired
    pub fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();
        now > self.expires_at
    }

    /// Check if invoice is paid
    pub fn is_paid(&self) -> bool {
        self.status == InvoiceStatus::Settled
    }

    /// Get amount in satoshis
    pub fn amount_sats(&self) -> u64 {
        self.amount_msat / 1000
    }

    /// Get remaining time until expiry
    pub fn time_remaining(&self) -> Option<Duration> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        if now < self.expires_at {
            Some(Duration::from_secs(self.expires_at - now))
        } else {
            None
        }
    }
}

/// Invoice status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvoiceStatus {
    /// Invoice created, awaiting payment
    Open,
    /// Payment received, settled
    Settled,
    /// Invoice expired without payment
    Expired,
    /// Invoice cancelled
    Cancelled,
    /// Payment in progress (accepted HTLC)
    Accepted,
}

/// Payment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    /// Payment hash
    pub payment_hash: String,
    /// Payment preimage
    pub payment_preimage: String,
    /// Amount paid (excluding fees) in millisatoshis
    pub amount_msat: u64,
    /// Fee paid in millisatoshis
    pub fee_msat: u64,
    /// Payment status
    pub status: PaymentStatus,
    /// Creation timestamp
    pub created_at: u64,
    /// Number of hops
    pub num_hops: u32,
    /// Payment route (if available)
    pub route: Option<Vec<RouteHop>>,
}

/// Payment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    /// Payment in flight
    InFlight,
    /// Payment succeeded
    Succeeded,
    /// Payment failed
    Failed,
    /// Payment unknown
    Unknown,
}

/// Route hop information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHop {
    /// Node public key
    pub pubkey: String,
    /// Channel ID
    pub channel_id: u64,
    /// Amount forwarded
    pub amount_msat: u64,
    /// Fee charged
    pub fee_msat: u64,
}

/// Channel balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBalance {
    /// Total local balance (can send)
    pub local_balance_msat: u64,
    /// Total remote balance (can receive)
    pub remote_balance_msat: u64,
    /// Pending local balance
    pub pending_local_msat: u64,
    /// Pending remote balance
    pub pending_remote_msat: u64,
    /// Unsettled local balance
    pub unsettled_local_msat: u64,
    /// Unsettled remote balance
    pub unsettled_remote_msat: u64,
}

impl ChannelBalance {
    /// Get sendable amount in satoshis
    pub fn can_send_sats(&self) -> u64 {
        self.local_balance_msat / 1000
    }

    /// Get receivable amount in satoshis
    pub fn can_receive_sats(&self) -> u64 {
        self.remote_balance_msat / 1000
    }
}

/// Channel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Channel ID
    pub channel_id: u64,
    /// Channel point (funding txid:output_index)
    pub channel_point: ChannelPoint,
    /// Remote node public key
    pub remote_pubkey: String,
    /// Local balance in millisatoshis
    pub local_balance_msat: u64,
    /// Remote balance in millisatoshis
    pub remote_balance_msat: u64,
    /// Channel capacity in satoshis
    pub capacity_sats: u64,
    /// Whether the channel is active
    pub active: bool,
    /// Whether the channel is private
    pub private: bool,
    /// Number of updates
    pub num_updates: u64,
    /// Commit fee in satoshis
    pub commit_fee_sats: u64,
    /// Time lock delta
    pub time_lock_delta: u32,
}

/// Channel point (funding transaction reference)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPoint {
    /// Funding transaction ID
    pub txid: String,
    /// Output index
    pub output_index: u32,
}

impl FromStr for ChannelPoint {
    type Err = BitcoinError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let output_index = parts[1].parse().map_err(|_| {
                BitcoinError::InvalidAddress("Invalid channel point format".to_string())
            })?;
            Ok(Self {
                txid: parts[0].to_string(),
                output_index,
            })
        } else {
            Err(BitcoinError::InvalidAddress(
                "Invalid channel point format".to_string(),
            ))
        }
    }
}

impl fmt::Display for ChannelPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.txid, self.output_index)
    }
}

/// Open channel request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    /// Node public key to connect to
    pub node_pubkey: String,
    /// Local funding amount in satoshis
    pub local_funding_sats: u64,
    /// Push amount to remote (gift) in satoshis
    pub push_sats: Option<u64>,
    /// Target confirmation blocks for funding tx
    pub target_conf: Option<u32>,
    /// Sat/vbyte for funding tx
    pub sat_per_vbyte: Option<u64>,
    /// Whether to make the channel private
    pub private: bool,
    /// Minimum HTLC size
    pub min_htlc_msat: Option<u64>,
}

impl OpenChannelRequest {
    /// Create a new channel open request with the given peer and funding amount
    pub fn new(node_pubkey: impl Into<String>, local_funding_sats: u64) -> Self {
        Self {
            node_pubkey: node_pubkey.into(),
            local_funding_sats,
            push_sats: None,
            target_conf: Some(3),
            sat_per_vbyte: None,
            private: false,
            min_htlc_msat: None,
        }
    }
}

/// Invoice subscription for real-time updates
pub struct InvoiceSubscription {
    receiver: tokio::sync::mpsc::Receiver<InvoiceUpdate>,
}

impl InvoiceSubscription {
    /// Create a new subscription
    pub fn new(receiver: tokio::sync::mpsc::Receiver<InvoiceUpdate>) -> Self {
        Self { receiver }
    }

    /// Wait for next invoice update
    pub async fn next(&mut self) -> Option<InvoiceUpdate> {
        self.receiver.recv().await
    }
}

/// Invoice update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceUpdate {
    /// Payment hash
    pub payment_hash: String,
    /// New status
    pub status: InvoiceStatus,
    /// Amount received (if paid)
    pub amount_received_msat: Option<u64>,
    /// Settlement timestamp
    pub settled_at: Option<u64>,
}

/// LND client implementation
pub struct LndClient {
    /// LND REST endpoint
    endpoint: String,
    /// Macaroon for authentication (hex encoded)
    macaroon: String,
    /// TLS certificate (optional, for custom CA)
    #[allow(dead_code)]
    tls_cert: Option<Vec<u8>>,
    /// HTTP client
    http_client: reqwest::Client,
}

impl LndClient {
    /// Create a new LND client
    pub fn new(endpoint: impl Into<String>, macaroon: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            macaroon: macaroon.into(),
            tls_cert: None,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .danger_accept_invalid_certs(true) // LND often uses self-signed certs
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let endpoint = std::env::var("LND_REST_ENDPOINT").ok()?;
        let macaroon = std::env::var("LND_MACAROON").ok()?;
        Some(Self::new(endpoint, macaroon))
    }

    /// Make an authenticated request to LND
    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let url = format!("{}{}", self.endpoint, path);

        let mut request = self
            .http_client
            .request(method, &url)
            .header("Grpc-Metadata-macaroon", &self.macaroon);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|e| BitcoinError::ConnectionFailed(format!("LND request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(BitcoinError::Wallet(format!(
                "LND error ({}): {}",
                status, error_text
            )));
        }

        response
            .json()
            .await
            .map_err(|e| BitcoinError::Wallet(format!("Failed to parse LND response: {}", e)))
    }
}

#[async_trait]
impl LightningProvider for LndClient {
    async fn get_info(&self) -> Result<NodeInfo> {
        #[derive(Deserialize)]
        struct LndGetInfo {
            identity_pubkey: String,
            alias: String,
            num_active_channels: u32,
            num_pending_channels: u32,
            num_peers: u32,
            block_height: u64,
            synced_to_chain: bool,
            version: String,
            chains: Vec<LndChain>,
        }

        #[derive(Deserialize)]
        struct LndChain {
            network: String,
        }

        let info: LndGetInfo = self
            .request(reqwest::Method::GET, "/v1/getinfo", None)
            .await?;

        Ok(NodeInfo {
            pubkey: info.identity_pubkey,
            alias: info.alias,
            num_active_channels: info.num_active_channels,
            num_pending_channels: info.num_pending_channels,
            num_peers: info.num_peers,
            block_height: info.block_height,
            synced_to_chain: info.synced_to_chain,
            version: info.version,
            network: info
                .chains
                .first()
                .map(|c| c.network.clone())
                .unwrap_or_default(),
        })
    }

    async fn create_invoice(&self, request: InvoiceRequest) -> Result<Invoice> {
        let body = serde_json::json!({
            "value_msat": request.amount_msat.to_string(),
            "memo": request.description,
            "expiry": request.expiry_secs.unwrap_or(3600).to_string(),
            "private": request.private,
        });

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct LndInvoice {
            r_hash: String,
            payment_request: String,
            add_index: String,
        }

        let invoice: LndInvoice = self
            .request(reqwest::Method::POST, "/v1/invoices", Some(body))
            .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        Ok(Invoice {
            payment_hash: invoice.r_hash,
            payment_preimage: None,
            bolt11: invoice.payment_request,
            amount_msat: request.amount_msat,
            description: request.description,
            created_at: now,
            expires_at: now + request.expiry_secs.unwrap_or(3600) as u64,
            status: InvoiceStatus::Open,
            amount_received_msat: None,
            settled_at: None,
            metadata: request.metadata,
        })
    }

    async fn get_invoice(&self, payment_hash: &str) -> Result<Invoice> {
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct LndInvoiceLookup {
            r_hash: String,
            r_preimage: Option<String>,
            payment_request: String,
            value_msat: String,
            memo: String,
            creation_date: String,
            expiry: String,
            settled: bool,
            amt_paid_msat: Option<String>,
            settle_date: Option<String>,
            state: String,
        }

        let path = format!("/v1/invoice/{}", payment_hash);
        let invoice: LndInvoiceLookup = self.request(reqwest::Method::GET, &path, None).await?;

        let created_at = invoice.creation_date.parse().unwrap_or(0);
        let expiry = invoice.expiry.parse().unwrap_or(3600);

        let status = match invoice.state.as_str() {
            "OPEN" => InvoiceStatus::Open,
            "SETTLED" => InvoiceStatus::Settled,
            "CANCELED" => InvoiceStatus::Cancelled,
            "ACCEPTED" => InvoiceStatus::Accepted,
            _ => InvoiceStatus::Open,
        };

        Ok(Invoice {
            payment_hash: invoice.r_hash,
            payment_preimage: invoice.r_preimage,
            bolt11: invoice.payment_request,
            amount_msat: invoice.value_msat.parse().unwrap_or(0),
            description: invoice.memo,
            created_at,
            expires_at: created_at + expiry,
            status,
            amount_received_msat: invoice.amt_paid_msat.and_then(|s| s.parse().ok()),
            settled_at: invoice.settle_date.and_then(|s| s.parse().ok()),
            metadata: HashMap::new(),
        })
    }

    async fn pay_invoice(&self, bolt11: &str, max_fee_msat: Option<u64>) -> Result<Payment> {
        let mut body = serde_json::json!({
            "payment_request": bolt11,
        });

        if let Some(fee) = max_fee_msat {
            body["fee_limit_msat"] = serde_json::json!(fee.to_string());
        }

        #[derive(Deserialize)]
        struct LndPayment {
            payment_hash: String,
            payment_preimage: String,
            value_msat: String,
            payment_route: Option<LndRoute>,
            status: String,
            fee_msat: String,
            creation_time_ns: String,
        }

        #[derive(Deserialize)]
        struct LndRoute {
            hops: Vec<LndHop>,
        }

        #[derive(Deserialize)]
        struct LndHop {
            pub_key: String,
            chan_id: String,
            amt_to_forward_msat: String,
            fee_msat: String,
        }

        let payment: LndPayment = self
            .request(
                reqwest::Method::POST,
                "/v1/channels/transactions",
                Some(body),
            )
            .await?;

        let status = match payment.status.as_str() {
            "SUCCEEDED" => PaymentStatus::Succeeded,
            "FAILED" => PaymentStatus::Failed,
            "IN_FLIGHT" => PaymentStatus::InFlight,
            _ => PaymentStatus::Unknown,
        };

        let route: Option<Vec<RouteHop>> = payment.payment_route.map(|r| {
            r.hops
                .into_iter()
                .map(|h| RouteHop {
                    pubkey: h.pub_key,
                    channel_id: h.chan_id.parse().unwrap_or(0),
                    amount_msat: h.amt_to_forward_msat.parse().unwrap_or(0),
                    fee_msat: h.fee_msat.parse().unwrap_or(0),
                })
                .collect()
        });

        let num_hops = route.as_ref().map(|r| r.len() as u32).unwrap_or(0);

        Ok(Payment {
            payment_hash: payment.payment_hash,
            payment_preimage: payment.payment_preimage,
            amount_msat: payment.value_msat.parse().unwrap_or(0),
            fee_msat: payment.fee_msat.parse().unwrap_or(0),
            status,
            created_at: payment.creation_time_ns.parse::<u64>().unwrap_or(0) / 1_000_000_000,
            num_hops,
            route,
        })
    }

    async fn get_balance(&self) -> Result<ChannelBalance> {
        #[derive(Deserialize)]
        struct LndBalance {
            local_balance: Option<LndBalanceDetail>,
            remote_balance: Option<LndBalanceDetail>,
            pending_open_local_balance: Option<LndBalanceDetail>,
            pending_open_remote_balance: Option<LndBalanceDetail>,
            unsettled_local_balance: Option<LndBalanceDetail>,
            unsettled_remote_balance: Option<LndBalanceDetail>,
        }

        #[derive(Deserialize)]
        struct LndBalanceDetail {
            msat: Option<String>,
        }

        let balance: LndBalance = self
            .request(reqwest::Method::GET, "/v1/balance/channels", None)
            .await?;

        let parse_msat = |detail: Option<LndBalanceDetail>| -> u64 {
            detail
                .and_then(|d| d.msat)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };

        Ok(ChannelBalance {
            local_balance_msat: parse_msat(balance.local_balance),
            remote_balance_msat: parse_msat(balance.remote_balance),
            pending_local_msat: parse_msat(balance.pending_open_local_balance),
            pending_remote_msat: parse_msat(balance.pending_open_remote_balance),
            unsettled_local_msat: parse_msat(balance.unsettled_local_balance),
            unsettled_remote_msat: parse_msat(balance.unsettled_remote_balance),
        })
    }

    async fn list_channels(&self) -> Result<Vec<Channel>> {
        #[derive(Deserialize)]
        struct LndChannels {
            channels: Option<Vec<LndChannel>>,
        }

        #[derive(Deserialize)]
        struct LndChannel {
            chan_id: String,
            channel_point: String,
            remote_pubkey: String,
            local_balance: String,
            remote_balance: String,
            capacity: String,
            active: bool,
            private: bool,
            num_updates: String,
            commit_fee: String,
            csv_delay: u32,
        }

        let channels: LndChannels = self
            .request(reqwest::Method::GET, "/v1/channels", None)
            .await?;

        Ok(channels
            .channels
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| {
                let channel_point = ChannelPoint::from_str(&c.channel_point).ok()?;
                Some(Channel {
                    channel_id: c.chan_id.parse().ok()?,
                    channel_point,
                    remote_pubkey: c.remote_pubkey,
                    local_balance_msat: c.local_balance.parse::<u64>().ok()? * 1000,
                    remote_balance_msat: c.remote_balance.parse::<u64>().ok()? * 1000,
                    capacity_sats: c.capacity.parse().ok()?,
                    active: c.active,
                    private: c.private,
                    num_updates: c.num_updates.parse().unwrap_or(0),
                    commit_fee_sats: c.commit_fee.parse().unwrap_or(0),
                    time_lock_delta: c.csv_delay,
                })
            })
            .collect())
    }

    async fn open_channel(&self, request: OpenChannelRequest) -> Result<ChannelPoint> {
        let body = serde_json::json!({
            "node_pubkey_string": request.node_pubkey,
            "local_funding_amount": request.local_funding_sats.to_string(),
            "push_sat": request.push_sats.unwrap_or(0).to_string(),
            "target_conf": request.target_conf.unwrap_or(3),
            "sat_per_vbyte": request.sat_per_vbyte.unwrap_or(1),
            "private": request.private,
        });

        #[derive(Deserialize)]
        struct LndOpenChannel {
            funding_txid_bytes: Option<String>,
            funding_txid_str: Option<String>,
            output_index: u32,
        }

        let result: LndOpenChannel = self
            .request(reqwest::Method::POST, "/v1/channels", Some(body))
            .await?;

        let txid = result
            .funding_txid_str
            .or(result.funding_txid_bytes)
            .ok_or_else(|| BitcoinError::Wallet("No funding txid returned".to_string()))?;

        Ok(ChannelPoint {
            txid,
            output_index: result.output_index,
        })
    }

    async fn close_channel(&self, channel_point: &ChannelPoint, force: bool) -> Result<String> {
        let path = format!(
            "/v1/channels/{}/{}?force={}",
            channel_point.txid, channel_point.output_index, force
        );

        #[derive(Deserialize)]
        struct LndCloseResult {
            closing_txid: Option<String>,
        }

        let result: LndCloseResult = self.request(reqwest::Method::DELETE, &path, None).await?;

        result
            .closing_txid
            .ok_or_else(|| BitcoinError::Wallet("No closing txid returned".to_string()))
    }

    async fn subscribe_invoices(&self) -> Result<InvoiceSubscription> {
        // For real implementation, this would use WebSocket/gRPC streaming
        // Here we create a mock channel that would be populated by a background task
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // In production, spawn a task to poll or stream invoice updates
        tokio::spawn(async move {
            // Keep the sender alive - in production this would be connected to LND streaming
            let _tx = tx;
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });

        Ok(InvoiceSubscription::new(rx))
    }
}

/// Lightning payment manager for order processing
pub struct LightningPaymentManager {
    provider: Box<dyn LightningProvider>,
    /// Minimum channel capacity to maintain
    min_capacity_sats: u64,
    /// Default invoice expiry
    default_expiry_secs: u32,
}

impl LightningPaymentManager {
    /// Create a new payment manager
    pub fn new(provider: Box<dyn LightningProvider>) -> Self {
        Self {
            provider,
            min_capacity_sats: 100_000, // 0.001 BTC
            default_expiry_secs: 3600,  // 1 hour
        }
    }

    /// Create an invoice for an order
    pub async fn create_order_invoice(
        &self,
        order_id: &str,
        amount_sats: u64,
        description: &str,
    ) -> Result<Invoice> {
        let request = InvoiceRequest::new(amount_sats, description)
            .expiry(self.default_expiry_secs)
            .with_metadata("order_id", order_id)
            .with_metadata("type", "order_payment");

        self.provider.create_invoice(request).await
    }

    /// Check if an order invoice is paid
    pub async fn check_order_payment(&self, payment_hash: &str) -> Result<OrderPaymentStatus> {
        let invoice = self.provider.get_invoice(payment_hash).await?;

        let is_expired = invoice.is_expired();
        Ok(OrderPaymentStatus {
            payment_hash: invoice.payment_hash,
            status: invoice.status,
            amount_paid_msat: invoice.amount_received_msat,
            settled_at: invoice.settled_at,
            is_expired,
        })
    }

    /// Check if we have enough inbound liquidity to receive a payment
    pub async fn can_receive(&self, amount_sats: u64) -> Result<bool> {
        let balance = self.provider.get_balance().await?;
        Ok(balance.can_receive_sats() >= amount_sats)
    }

    /// Check if we have enough outbound liquidity to send a payment
    pub async fn can_send(&self, amount_sats: u64) -> Result<bool> {
        let balance = self.provider.get_balance().await?;
        Ok(balance.can_send_sats() >= amount_sats)
    }

    /// Get node health status
    pub async fn get_health(&self) -> Result<LightningHealth> {
        let info = self.provider.get_info().await?;
        let balance = self.provider.get_balance().await?;
        let channels = self.provider.list_channels().await?;

        let active_channels = channels.iter().filter(|c| c.active).count();
        let total_capacity = channels.iter().map(|c| c.capacity_sats).sum();

        Ok(LightningHealth {
            node_pubkey: info.pubkey,
            synced: info.synced_to_chain,
            active_channels: active_channels as u32,
            total_channels: channels.len() as u32,
            can_send_sats: balance.can_send_sats(),
            can_receive_sats: balance.can_receive_sats(),
            total_capacity_sats: total_capacity,
            has_sufficient_liquidity: balance.can_receive_sats() >= self.min_capacity_sats,
        })
    }
}

/// Order payment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPaymentStatus {
    /// Payment hash
    pub payment_hash: String,
    /// Invoice status
    pub status: InvoiceStatus,
    /// Amount paid in millisatoshis
    pub amount_paid_msat: Option<u64>,
    /// Settlement timestamp
    pub settled_at: Option<u64>,
    /// Whether the invoice has expired
    pub is_expired: bool,
}

/// Lightning node health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightningHealth {
    /// Node public key
    pub node_pubkey: String,
    /// Whether node is synced to chain
    pub synced: bool,
    /// Number of active channels
    pub active_channels: u32,
    /// Total number of channels
    pub total_channels: u32,
    /// Amount we can send (satoshis)
    pub can_send_sats: u64,
    /// Amount we can receive (satoshis)
    pub can_receive_sats: u64,
    /// Total channel capacity
    pub total_capacity_sats: u64,
    /// Whether we have sufficient liquidity for operations
    pub has_sufficient_liquidity: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoice_request_builder() {
        let request = InvoiceRequest::new(1000, "Test payment")
            .expiry(1800)
            .with_metadata("order_id", "order123")
            .private(true);

        assert_eq!(request.amount_msat, 1_000_000);
        assert_eq!(request.expiry_secs, Some(1800));
        assert!(request.private);
        assert_eq!(
            request.metadata.get("order_id"),
            Some(&"order123".to_string())
        );
    }

    #[test]
    fn test_channel_point_parsing() {
        let cp = ChannelPoint::from_str("abc123:0").unwrap();

        assert_eq!(cp.txid, "abc123");
        assert_eq!(cp.output_index, 0);
        assert_eq!(cp.to_string(), "abc123:0");
    }

    #[test]
    fn test_invoice_status() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime should be after UNIX_EPOCH")
            .as_secs();

        let invoice = Invoice {
            payment_hash: "hash".to_string(),
            payment_preimage: None,
            bolt11: "lnbc...".to_string(),
            amount_msat: 1_000_000,
            description: "Test".to_string(),
            created_at: now,
            expires_at: now + 3600,
            status: InvoiceStatus::Open,
            amount_received_msat: None,
            settled_at: None,
            metadata: HashMap::new(),
        };

        assert!(!invoice.is_expired());
        assert!(!invoice.is_paid());
        assert_eq!(invoice.amount_sats(), 1000);
    }
}
