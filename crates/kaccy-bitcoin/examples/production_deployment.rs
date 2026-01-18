//! Production Deployment Example
//!
//! This example demonstrates production-ready Bitcoin payment monitoring with:
//! - Security best practices (watch-only wallet, no private keys)
//! - Health monitoring and alerting
//! - Graceful error handling and recovery
//! - Structured logging
//!
//! # Security Best Practices
//!
//! 1. **Key Management**: Never store private keys - use watch-only wallet (xpub only)
//! 2. **Network Isolation**: Keep Bitcoin Core behind firewall, use TLS for RPC
//! 3. **Confirmation Depth**: Wait for sufficient confirmations (6 for high-value)
//! 4. **Health Monitoring**: Regular health checks with automatic alerting
//!
//! # Environment Setup
//!
//! ```bash
//! export BITCOIN_RPC_URL=https://bitcoin-rpc.example.com:8332
//! export BITCOIN_RPC_USER=your_rpc_user
//! export BITCOIN_RPC_PASSWORD=your_secure_password
//! export WALLET_XPUB=xpub6...  # BIP84 extended public key
//! ```

use kaccy_bitcoin::{
    BitcoinClient, BitcoinNetwork, HdWallet, HdWalletConfig, HealthCheckConfig, HealthCheckManager,
    PaymentMonitor,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    info!("Starting production Bitcoin payment system");

    // Load configuration from environment
    let rpc_url =
        std::env::var("BITCOIN_RPC_URL").unwrap_or_else(|_| "http://localhost:8332".to_string());
    let rpc_user = std::env::var("BITCOIN_RPC_USER")?;
    let rpc_password = std::env::var("BITCOIN_RPC_PASSWORD")?;
    let wallet_xpub = std::env::var("WALLET_XPUB")?;

    // Initialize Bitcoin Core client (mainnet)
    let bitcoin_client = Arc::new(BitcoinClient::new(
        &rpc_url,
        &rpc_user,
        &rpc_password,
        BitcoinNetwork::Mainnet,
    )?);

    // Verify connection
    match bitcoin_client.health_check() {
        Ok(true) => info!("Bitcoin Core connection established"),
        Ok(false) => {
            error!("Bitcoin Core health check failed");
            return Err("Health check failed".into());
        }
        Err(e) => {
            error!("Failed to connect to Bitcoin Core: {}", e);
            return Err(e.into());
        }
    }

    // Initialize HD wallet (watch-only, no private keys stored)
    let wallet_config = HdWalletConfig::mainnet(wallet_xpub);
    let hd_wallet = Arc::new(HdWallet::new(wallet_config)?);
    info!("HD wallet initialized (watch-only mode, no private keys)");

    // Initialize payment monitoring (6 confirmations for security)
    let _payment_monitor = Arc::new(PaymentMonitor::new(bitcoin_client.clone(), 6));
    info!("Payment monitor initialized (6 confirmations required)");

    // Initialize health check manager
    let health_config = HealthCheckConfig {
        check_interval: Duration::from_secs(30),
        check_timeout: Duration::from_secs(10),
        max_block_age_secs: 3600, // Alert if blocks are > 1 hour old
        monitor_resources: false,
    };

    let health_manager = Arc::new(HealthCheckManager::new(health_config));
    info!("Health monitoring configured");

    // Start background health checks
    let health_check_task = {
        let health_manager = health_manager.clone();
        let bitcoin_client = bitcoin_client.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                match health_manager.check_health(&bitcoin_client).await {
                    Ok(report) => {
                        if !report.is_all_healthy() {
                            warn!(
                                status = ?report.status,
                                "Health check issues detected"
                            );
                            // In production: send alert to monitoring system
                        }
                    }
                    Err(e) => {
                        error!("Health check failed: {}", e);
                        // In production: trigger alert
                    }
                }
            }
        })
    };

    info!("Production Bitcoin payment system started successfully");
    info!("✓ Watch-only wallet (no private keys stored)");
    info!("✓ 6 confirmations required for payment finality");
    info!("✓ Health checks running every 30 seconds");
    info!("✓ Ready to monitor payments");

    // Example: Generate a unique address for an order (BIP84: m/84'/0'/0'/0/index)
    let order_index = 0; // In production, increment for each new order
    match hd_wallet.derive_address(0, order_index) {
        Ok(address) => {
            info!("Example order address (index {}): {}", order_index, address);
        }
        Err(e) => {
            warn!("Failed to derive address: {}", e);
        }
    }

    // Wait for shutdown signal (Ctrl+C)
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received, gracefully shutting down...");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Graceful cleanup
    info!("Shutting down services...");
    health_check_task.abort();

    info!("Production Bitcoin payment system shut down successfully");
    Ok(())
}
