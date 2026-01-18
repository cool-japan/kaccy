//! Lightning Network payment gateway example
//!
//! This example demonstrates how to build a Lightning Network payment gateway
//! that accepts Lightning payments and manages invoices.

use kaccy_bitcoin::{
    BitcoinClient, BitcoinMetrics, BitcoinNetwork, HealthCheckManager, InMemoryMetricsBackend,
    InvoiceStatus, LightningPaymentManager, LightningProvider, LndClient,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Lightning Payment Gateway Example");
    info!("=================================");

    // Configuration
    let bitcoin_rpc_url = "http://localhost:8332";
    let bitcoin_rpc_user = "user";
    let bitcoin_rpc_pass = "pass";

    let lnd_url = "https://localhost:8080";
    let lnd_macaroon_path = "/path/to/admin.macaroon";

    // Initialize Bitcoin client
    info!("Connecting to Bitcoin Core...");
    let bitcoin_client = Arc::new(
        BitcoinClient::new(
            bitcoin_rpc_url,
            bitcoin_rpc_user,
            bitcoin_rpc_pass,
            BitcoinNetwork::Regtest,
        )
        .expect("Failed to create Bitcoin client"),
    );

    // Initialize metrics
    let metrics_backend = Arc::new(InMemoryMetricsBackend::new());
    let metrics = Arc::new(BitcoinMetrics::new(metrics_backend.clone()));

    // Initialize health check manager
    let health_manager = Arc::new(HealthCheckManager::with_defaults());

    // Start background health checks
    let _health_task = health_manager
        .clone()
        .start_background_checks(bitcoin_client.clone());

    // Initialize Lightning client
    info!("Connecting to LND...");
    info!("Note: This example requires a properly configured LND node.");
    let lnd_client = LndClient::new(lnd_url, lnd_macaroon_path);

    // Create a second client for direct API calls
    let lnd_client_direct = Arc::new(LndClient::new(lnd_url, lnd_macaroon_path));

    // Initialize payment manager
    let payment_manager = Arc::new(LightningPaymentManager::new(Box::new(lnd_client)));

    // Example: Create an invoice for an order
    info!("\n1. Creating Lightning invoice for order...");
    let order_id = "ORDER-12345";
    let amount_sats = 50_000; // 50,000 sats

    match payment_manager
        .create_order_invoice(
            order_id,
            amount_sats,
            &format!("Payment for order {}", order_id),
        )
        .await
    {
        Ok(invoice) => {
            info!("Invoice created successfully!");
            info!("  Payment Hash: {}", invoice.payment_hash);
            info!("  Amount: {} sats", invoice.amount_sats());
            info!("  Status: {:?}", invoice.status);

            // Record metrics
            metrics.record_payment_status("invoice_created");

            // In a real application, you would:
            // 1. Store this invoice in your database
            // 2. Associate it with the order
            // 3. Display the payment request to the user
            // 4. Monitor for payment completion

            info!("\n2. Monitoring invoice status...");
            let payment_hash = invoice.payment_hash.clone();

            // Poll for payment (in real app, use invoice subscriptions)
            for i in 0..10 {
                sleep(Duration::from_secs(3)).await;

                match payment_manager.check_order_payment(&payment_hash).await {
                    Ok(payment_status) => {
                        info!(
                            "  Check {}: Status = {:?}, Is Expired = {}",
                            i + 1,
                            payment_status.status,
                            payment_status.is_expired
                        );

                        match payment_status.status {
                            InvoiceStatus::Settled => {
                                info!("  ✓ Payment received!");
                                metrics.record_payment_status("paid");

                                // In a real application:
                                // 1. Update order status to paid
                                // 2. Trigger order fulfillment
                                // 3. Send confirmation to customer
                                break;
                            }
                            InvoiceStatus::Expired => {
                                warn!("  ✗ Invoice expired");
                                metrics.record_payment_status("expired");
                                break;
                            }
                            InvoiceStatus::Cancelled => {
                                warn!("  ✗ Invoice cancelled");
                                metrics.record_payment_status("cancelled");
                                break;
                            }
                            _ => {
                                // Still pending, continue monitoring
                            }
                        }
                    }
                    Err(e) => {
                        warn!("  Error checking payment status: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            warn!("Failed to create invoice: {}", e);
            return Err(e.into());
        }
    }

    // Example: Check Lightning node health
    info!("\n3. Checking Lightning node health...");
    match lnd_client_direct.get_info().await {
        Ok(node_info) => {
            info!("  Node Alias: {}", node_info.alias);
            info!("  Public Key: {}", node_info.pubkey);
            info!("  Active Channels: {}", node_info.num_active_channels);
        }
        Err(e) => {
            warn!("Failed to get node info: {}", e);
        }
    }

    // Example: Check channel balances
    info!("\n4. Checking channel balances...");
    match lnd_client_direct.get_balance().await {
        Ok(balance) => {
            info!("  Can Send: {} sats", balance.can_send_sats());
            info!("  Can Receive: {} sats", balance.can_receive_sats());
        }
        Err(e) => {
            warn!("Failed to get channel balance: {}", e);
        }
    }

    // Get health report
    info!("\n5. Health Check Report:");
    match health_manager.check_health(&bitcoin_client).await {
        Ok(report) => {
            info!("  Overall Status: {:?}", report.status);
            info!("  Uptime: {} seconds", report.uptime_seconds);
            info!("  Components:");
            for component in &report.components {
                info!("    - {}: {:?}", component.name, component.status);
                if let Some(ref msg) = component.message {
                    info!("      Message: {}", msg);
                }
            }
        }
        Err(e) => {
            warn!("Failed to get health report: {}", e);
        }
    }

    // Get metrics statistics
    info!("\n6. Metrics Summary:");
    let stats = metrics.get_stats().await;
    info!("  Total RPC Calls: {}", stats.total_rpc_calls);
    info!("  Success Rate: {:.2}%", stats.rpc_success_rate() * 100.0);
    info!("  Avg RPC Duration: {:.2}ms", stats.avg_rpc_duration_ms());

    info!("\nLightning Gateway Example Complete!");
    info!("\nIn a production environment, you would:");
    info!("  1. Use proper configuration management");
    info!("  2. Implement robust error handling and retries");
    info!("  3. Store invoices and payments in a database");
    info!("  4. Use WebSocket subscriptions for real-time updates");
    info!("  5. Implement proper security measures");
    info!("  6. Set up monitoring and alerting");
    info!("  7. Handle edge cases (reorgs, channel closures, etc.)");

    Ok(())
}
