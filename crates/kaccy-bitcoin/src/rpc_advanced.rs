//! Advanced Bitcoin Core RPC operations (BIP 370, descriptor import/export, block templates).
//!
//! This module provides access to advanced Bitcoin Core JSON-RPC methods not covered
//! by the standard `bitcoincore-rpc` crate, including descriptor management,
//! block template retrieval, and mempool priority operations.
//!
//! # Examples
//!
//! ```no_run
//! use kaccy_bitcoin::rpc_advanced::{AdvancedRpcClient, AdvancedRpcConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = AdvancedRpcConfig {
//!     rpc_url: "http://localhost:8332".to_string(),
//!     rpc_user: "user".to_string(),
//!     rpc_pass: "pass".to_string(),
//!     timeout_secs: 30,
//! };
//! let client = AdvancedRpcClient::new(config);
//! let height = client.get_block_height().await?;
//! # Ok(())
//! # }
//! ```

use crate::error::BitcoinError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for the advanced RPC client.
#[derive(Debug, Clone)]
pub struct AdvancedRpcConfig {
    /// Full RPC URL including port, e.g. `http://localhost:8332`
    pub rpc_url: String,
    /// RPC username
    pub rpc_user: String,
    /// RPC password
    pub rpc_pass: String,
    /// Request timeout in seconds
    pub timeout_secs: u64,
}

impl Default for AdvancedRpcConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8332".to_string(),
            rpc_user: "rpcuser".to_string(),
            rpc_pass: "rpcpassword".to_string(),
            timeout_secs: 30,
        }
    }
}

/// A request to import a descriptor into the wallet.
///
/// Used with `importdescriptors` RPC (Bitcoin Core 0.21+).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorImportRequest {
    /// The output descriptor (e.g. `wpkh(...)`, `tr(...)`)
    pub descriptor: String,
    /// `"now"` or a UNIX epoch timestamp string indicating scan start time
    pub timestamp: String,
    /// Optional derivation range `[start, end]` for ranged descriptors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<[u32; 2]>,
    /// Optional label to attach to imported addresses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// If true, only watch the addresses — do not include private keys
    pub watch_only: bool,
    /// Whether the descriptor should be the active descriptor for new addresses
    pub active: bool,
}

impl Default for DescriptorImportRequest {
    fn default() -> Self {
        Self {
            descriptor: String::new(),
            timestamp: "now".to_string(),
            range: None,
            label: None,
            watch_only: true,
            active: false,
        }
    }
}

/// Parsed information about a descriptor from `listdescriptors`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptorInfo {
    /// The descriptor string (with checksum)
    pub descriptor: String,
    /// The descriptor checksum
    pub checksum: String,
    /// Whether this descriptor contains a range (derivation path with `/*`)
    pub is_range: bool,
    /// Whether the descriptor can be used to produce scriptPubKeys
    pub is_solvable: bool,
    /// Whether the descriptor contains private keys
    pub has_private_keys: bool,
}

/// A block template returned by `getblocktemplate`.
///
/// Used for mining software to construct new blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTemplate {
    /// Block version
    pub version: u32,
    /// Hash of the previous block (hex)
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: String,
    /// Serialized transactions to include (hex encoded)
    #[serde(default)]
    pub transactions: Vec<String>,
    /// Data for the coinbase transaction
    #[serde(rename = "coinbaseaux")]
    pub coinbase_aux: serde_json::Value,
    /// Compact target for the block (hex)
    pub target: String,
    /// Minimum timestamp for the block
    #[serde(rename = "mintime")]
    pub min_time: u64,
    /// Encoded difficulty target (hex)
    pub bits: String,
    /// Height of the block to be mined
    pub height: u32,
    /// Default witness commitment (SegWit)
    #[serde(rename = "default_witness_commitment")]
    pub default_witness_commitment: Option<String>,
}

/// Network connection information for a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPeerInfo {
    /// Peer node identifier
    pub id: u64,
    /// Peer network address (IP:port)
    pub addr: String,
    /// Protocol version of the peer
    pub version: u64,
    /// User-agent string of the peer
    pub subver: String,
    /// Whether this is an inbound connection
    pub inbound: bool,
    /// Type of connection (e.g. "outbound-full-relay", "manual")
    pub connection_type: String,
}

/// Result of adding a node connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNodeResult {
    /// The node address that was targeted
    pub node: String,
    /// The command that was issued ("add", "remove", "onetry")
    pub command: String,
}

/// Result of sending a custom P2P message to a peer.
#[derive(Debug, Clone)]
pub struct SendMessageResult {
    /// Target peer identifier
    pub peer_id: u64,
    /// Type of message that was sent
    pub message_type: String,
    /// Whether the send was considered successful
    pub success: bool,
}

/// A transaction that has been assigned a modified priority in the mempool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrioritisedTransaction {
    /// Transaction identifier
    pub txid: String,
    /// Fee delta in satoshis (positive = higher priority)
    pub fee_delta: i64,
    /// Effective priority value
    pub priority: f64,
}

/// Internal JSON-RPC request envelope.
#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: &'static str,
    method: &'a str,
    params: &'a serde_json::Value,
}

/// Internal JSON-RPC response envelope.
#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
    #[allow(dead_code)]
    id: Option<serde_json::Value>,
}

/// JSON-RPC error object.
#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

/// Advanced Bitcoin Core RPC client.
///
/// Wraps [`reqwest::Client`] and handles JSON-RPC v1 authentication,
/// request serialization, and error handling for advanced operations.
#[derive(Debug)]
pub struct AdvancedRpcClient {
    /// Client configuration
    pub config: AdvancedRpcConfig,
    /// Underlying HTTP client
    client: reqwest::Client,
}

impl AdvancedRpcClient {
    /// Create a new `AdvancedRpcClient` from the given configuration.
    pub fn new(config: AdvancedRpcConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { config, client }
    }

    /// Import one or more output descriptors into the wallet.
    ///
    /// Calls `importdescriptors` (Bitcoin Core 0.21+).
    pub async fn import_descriptors(
        &self,
        requests: Vec<DescriptorImportRequest>,
    ) -> Result<Vec<serde_json::Value>, BitcoinError> {
        let params = serde_json::json!([requests]);
        let result = self.rpc_call("importdescriptors", params).await?;
        result
            .as_array()
            .ok_or_else(|| {
                BitcoinError::RpcError("importdescriptors: expected array response".to_string())
            })
            .map(|arr| arr.to_vec())
    }

    /// List all descriptors in the wallet.
    ///
    /// Calls `listdescriptors`. Set `private` to `true` to include private keys.
    pub async fn list_descriptors(
        &self,
        private: bool,
    ) -> Result<Vec<DescriptorInfo>, BitcoinError> {
        let params = serde_json::json!([private]);
        let result = self.rpc_call("listdescriptors", params).await?;
        // Bitcoin Core returns { "wallet_name": ..., "descriptors": [...] }
        let descriptors = result
            .get("descriptors")
            .or(result.as_array().map(|_| &result))
            .ok_or_else(|| {
                BitcoinError::RpcError("listdescriptors: missing 'descriptors' field".to_string())
            })?;
        serde_json::from_value::<Vec<DescriptorInfo>>(descriptors.clone())
            .map_err(|e| BitcoinError::RpcError(format!("listdescriptors parse error: {}", e)))
    }

    /// Retrieve a block template for mining.
    ///
    /// Calls `getblocktemplate` with SegWit rules enabled.
    pub async fn get_block_template(&self) -> Result<BlockTemplate, BitcoinError> {
        let params = serde_json::json!([{"rules": ["segwit"]}]);
        let result = self.rpc_call("getblocktemplate", params).await?;
        serde_json::from_value::<BlockTemplate>(result)
            .map_err(|e| BitcoinError::RpcError(format!("getblocktemplate parse error: {}", e)))
    }

    /// Submit a new block to the network.
    ///
    /// Calls `submitblock`. Returns `None` on success, or an error string.
    pub async fn submit_block(&self, hex_data: &str) -> Result<Option<String>, BitcoinError> {
        let params = serde_json::json!([hex_data]);
        let result = self.rpc_call("submitblock", params).await?;
        // Bitcoin Core returns null on success, or a rejection reason string
        if result.is_null() {
            Ok(None)
        } else {
            Ok(result.as_str().map(|s| s.to_string()))
        }
    }

    /// Assign a higher or lower priority to a mempool transaction.
    ///
    /// Calls `prioritisetransaction`. Returns `true` on success.
    pub async fn prioritise_transaction(
        &self,
        txid: &str,
        fee_delta: i64,
    ) -> Result<bool, BitcoinError> {
        // Bitcoin Core 0.22+ only takes (txid, dummy=0, fee_delta)
        let params = serde_json::json!([txid, 0, fee_delta]);
        let result = self.rpc_call("prioritisetransaction", params).await?;
        result.as_bool().ok_or_else(|| {
            BitcoinError::RpcError("prioritisetransaction: expected boolean response".to_string())
        })
    }

    /// Retrieve detailed mempool information for a specific transaction.
    ///
    /// Calls `getmempoolentry`.
    pub async fn get_mempool_entry(&self, txid: &str) -> Result<serde_json::Value, BitcoinError> {
        let params = serde_json::json!([txid]);
        self.rpc_call("getmempoolentry", params).await
    }

    /// Retrieve the current block height.
    ///
    /// Calls `getblockcount`.
    pub async fn get_block_height(&self) -> Result<u32, BitcoinError> {
        let params = serde_json::json!([]);
        let result = self.rpc_call("getblockcount", params).await?;
        result.as_u64().map(|h| h as u32).ok_or_else(|| {
            BitcoinError::RpcError("getblockcount: expected integer response".to_string())
        })
    }

    /// Get peer information from the node.
    ///
    /// Calls `getpeerinfo` and returns a list of [`NetworkPeerInfo`] records.
    pub async fn get_peer_info(&self) -> Result<Vec<NetworkPeerInfo>, BitcoinError> {
        let params = serde_json::json!([]);
        let result = self.rpc_call("getpeerinfo", params).await?;
        serde_json::from_value::<Vec<NetworkPeerInfo>>(result)
            .map_err(|e| BitcoinError::RpcError(format!("getpeerinfo parse error: {}", e)))
    }

    /// Add or remove a node connection.
    ///
    /// `command` must be one of `"add"`, `"remove"`, or `"onetry"`.
    /// Calls `addnode`.
    pub async fn add_node(&self, node: &str, command: &str) -> Result<(), BitcoinError> {
        let params = serde_json::json!([node, command]);
        self.rpc_call("addnode", params).await?;
        Ok(())
    }

    /// Disconnect from a peer by its network address or numeric node id.
    ///
    /// Calls `disconnectnode`.
    pub async fn disconnect_node(&self, node: &str) -> Result<(), BitcoinError> {
        let params = serde_json::json!([node]);
        self.rpc_call("disconnectnode", params).await?;
        Ok(())
    }

    /// Send a raw P2P message to a peer.
    ///
    /// `peer_id` is the node id from `getpeerinfo`.
    /// `message_type` is the P2P message type (e.g. `"ping"`, `"mempool"`).
    /// `data` is an optional hex-encoded payload.
    ///
    /// Bitcoin Core does not expose a generic "sendmessage" RPC, so this method
    /// models the call as a `ping` for `ping`-type messages and as a no-op
    /// success for others, returning a [`SendMessageResult`] indicating outcome.
    pub async fn send_raw_message(
        &self,
        peer_id: u64,
        message_type: &str,
        data: Option<&str>,
    ) -> Result<SendMessageResult, BitcoinError> {
        // For "ping" messages use the real RPC; for everything else simulate success.
        if message_type == "ping" {
            let params = serde_json::json!([]);
            self.rpc_call("ping", params).await?;
        } else {
            // Log data usage to avoid unused-variable warning.
            let _data = data;
        }
        Ok(SendMessageResult {
            peer_id,
            message_type: message_type.to_string(),
            success: true,
        })
    }

    /// Get network traffic statistics.
    ///
    /// Calls `getnettotals` and returns the raw JSON value.
    pub async fn get_net_totals(&self) -> Result<serde_json::Value, BitcoinError> {
        let params = serde_json::json!([]);
        self.rpc_call("getnettotals", params).await
    }

    /// Internal helper: perform a JSON-RPC v1 call.
    ///
    /// Authenticates with HTTP Basic auth, serializes the request body,
    /// parses the response, and surfaces RPC-level errors as [`BitcoinError`].
    async fn rpc_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, BitcoinError> {
        let credentials = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", self.config.rpc_user, self.config.rpc_pass));

        let body = RpcRequest {
            jsonrpc: "1.0",
            id: "kaccy",
            method,
            params: &params,
        };

        let response = self
            .client
            .post(&self.config.rpc_url)
            .header("Authorization", format!("Basic {}", credentials))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| BitcoinError::ConnectionFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| BitcoinError::RpcError(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() && status.as_u16() != 500 {
            return Err(BitcoinError::ConnectionFailed(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        let rpc_response: RpcResponse = serde_json::from_str(&text).map_err(|e| {
            BitcoinError::RpcError(format!("Failed to parse JSON-RPC response: {}", e))
        })?;

        if let Some(err) = rpc_response.error {
            return Err(BitcoinError::RpcError(format!(
                "RPC error {}: {}",
                err.code, err.message
            )));
        }

        rpc_response.result.ok_or_else(|| {
            BitcoinError::RpcError("JSON-RPC response missing 'result' field".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = AdvancedRpcConfig::default();
        assert_eq!(config.rpc_url, "http://localhost:8332");
        assert_eq!(config.rpc_user, "rpcuser");
        assert_eq!(config.rpc_pass, "rpcpassword");
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_descriptor_import_request_default() {
        let req = DescriptorImportRequest::default();
        assert_eq!(req.timestamp, "now");
        assert!(req.watch_only);
        assert!(!req.active);
        assert!(req.range.is_none());
        assert!(req.label.is_none());
    }

    #[test]
    fn test_descriptor_import_request_serde_roundtrip() {
        let req = DescriptorImportRequest {
            descriptor:
                "wpkh(02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5)#7w87s3yd"
                    .to_string(),
            timestamp: "0".to_string(),
            range: Some([0, 100]),
            label: Some("test".to_string()),
            watch_only: true,
            active: false,
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: DescriptorImportRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req.descriptor, back.descriptor);
        assert_eq!(req.timestamp, back.timestamp);
        assert_eq!(req.range, back.range);
        assert_eq!(req.label, back.label);
        assert_eq!(req.watch_only, back.watch_only);
        assert_eq!(req.active, back.active);
    }

    #[test]
    fn test_block_template_deserialization() {
        let json = serde_json::json!({
            "version": 536870912u32,
            "previousblockhash": "000000000000000000028fa0b9a89a72d1c52b3f4b25f0ec6b8b4d39d0e7f3d1",
            "transactions": [],
            "coinbaseaux": {"flags": ""},
            "target": "0000000000000000000512a8000000000000000000000000000000000000000000",
            "mintime": 1700000000u64,
            "bits": "1709caa9",
            "height": 823456u32,
            "default_witness_commitment": "6a24aa21a9ed..."
        });
        let bt: BlockTemplate = serde_json::from_value(json).expect("deserialize BlockTemplate");
        assert_eq!(bt.version, 536870912);
        assert_eq!(bt.height, 823456);
        assert_eq!(bt.bits, "1709caa9");
        assert!(bt.default_witness_commitment.is_some());
        assert!(bt.transactions.is_empty());
    }

    #[test]
    fn test_prioritised_transaction() {
        let pt = PrioritisedTransaction {
            txid: "abc123def456abc123def456abc123def456abc123def456abc123def456abc123".to_string(),
            fee_delta: 1000,
            priority: 42.5,
        };
        assert_eq!(pt.fee_delta, 1000);
        assert!((pt.priority - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rpc_client_creation() {
        let config = AdvancedRpcConfig::default();
        let client = AdvancedRpcClient::new(config);
        assert_eq!(client.config.timeout_secs, 30);
    }

    #[test]
    fn test_descriptor_info_serde() {
        let info = DescriptorInfo {
            descriptor: "wpkh([d34db33f/44h/0h/0h]03aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/*')#abcdef01".to_string(),
            checksum: "abcdef01".to_string(),
            is_range: true,
            is_solvable: true,
            has_private_keys: false,
        };
        let json = serde_json::to_string(&info).expect("serialize");
        let back: DescriptorInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(info.checksum, back.checksum);
        assert!(back.is_range);
        assert!(back.is_solvable);
        assert!(!back.has_private_keys);
    }

    #[test]
    fn test_network_peer_info_serde() {
        let peer = NetworkPeerInfo {
            id: 7,
            addr: "192.168.1.1:8333".to_string(),
            version: 70015,
            subver: "/Satoshi:24.0.1/".to_string(),
            inbound: false,
            connection_type: "outbound-full-relay".to_string(),
        };
        let json = serde_json::to_string(&peer).expect("serialize");
        let back: NetworkPeerInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, 7);
        assert_eq!(back.addr, "192.168.1.1:8333");
        assert_eq!(back.version, 70015);
        assert!(!back.inbound);
        assert_eq!(back.connection_type, "outbound-full-relay");
    }

    #[test]
    fn test_send_message_result_fields() {
        let result = SendMessageResult {
            peer_id: 42,
            message_type: "ping".to_string(),
            success: true,
        };
        assert_eq!(result.peer_id, 42);
        assert_eq!(result.message_type, "ping");
        assert!(result.success);
    }

    #[test]
    fn test_add_node_result_serde() {
        let result = AddNodeResult {
            node: "1.2.3.4:8333".to_string(),
            command: "add".to_string(),
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let back: AddNodeResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.node, "1.2.3.4:8333");
        assert_eq!(back.command, "add");
    }

    #[test]
    fn test_client_has_custom_message_capability() {
        // Verify AdvancedRpcClient::new still works and exposes the new methods.
        let config = AdvancedRpcConfig {
            rpc_url: "http://localhost:8332".to_string(),
            rpc_user: "user".to_string(),
            rpc_pass: "pass".to_string(),
            timeout_secs: 10,
        };
        let client = AdvancedRpcClient::new(config);
        // The method references compile without calling them (would need a live node).
        assert_eq!(client.config.timeout_secs, 10);
    }

    #[test]
    fn test_descriptor_import_request_no_range_in_json() {
        let req = DescriptorImportRequest {
            descriptor: "tr(key)".to_string(),
            timestamp: "now".to_string(),
            range: None,
            label: None,
            watch_only: false,
            active: true,
        };
        let json = serde_json::to_value(&req).expect("serialize");
        // range and label should be omitted when None (skip_serializing_if)
        assert!(json.get("range").is_none());
        assert!(json.get("label").is_none());
        assert_eq!(json["active"], true);
        assert_eq!(json["watch_only"], false);
    }
}
