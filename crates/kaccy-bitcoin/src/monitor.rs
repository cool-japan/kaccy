//! Transaction monitoring

use bitcoin::Amount;
use serde::Serialize;
use std::sync::Arc;

use crate::client::BitcoinClient;
use crate::error::{BitcoinError, Result};

/// Payment status with details
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PaymentStatus {
    /// No payment received yet
    NotReceived,
    /// Payment received but waiting for confirmations
    Pending {
        /// Amount received so far (may include mempool balance).
        received_amount: Amount,
        /// Number of confirmations the payment currently has.
        confirmations: u32,
    },
    /// Payment received but amount is incorrect
    Underpaid {
        /// The amount that was expected.
        expected: Amount,
        /// The amount that was actually received.
        received: Amount,
    },
    /// Payment received but amount is more than expected
    Overpaid {
        /// The amount that was expected.
        expected: Amount,
        /// The amount that was actually received.
        received: Amount,
    },
    /// Payment confirmed with required confirmations
    Confirmed {
        /// Total amount received and confirmed.
        received_amount: Amount,
        /// Number of confirmations at the time of confirmation.
        confirmations: u32,
    },
}

impl PaymentStatus {
    /// Check if payment can proceed (confirmed or overpaid with enough confirmations)
    pub fn is_actionable(&self) -> bool {
        matches!(
            self,
            PaymentStatus::Confirmed { .. } | PaymentStatus::Overpaid { .. }
        )
    }
}

/// Monitors payments for pending orders
pub struct PaymentMonitor {
    client: Arc<BitcoinClient>,
    required_confirmations: u32,
}

impl PaymentMonitor {
    /// Create a new `PaymentMonitor` with the given client and confirmation threshold.
    pub fn new(client: Arc<BitcoinClient>, required_confirmations: u32) -> Self {
        Self {
            client,
            required_confirmations,
        }
    }

    /// Check payment status for an address
    pub fn check_payment(&self, address: &str, expected_amount: Amount) -> Result<PaymentStatus> {
        let addr: bitcoin::Address<bitcoin::address::NetworkUnchecked> = address
            .parse()
            .map_err(|e| BitcoinError::InvalidAddress(format!("{:?}", e)))?;

        let checked_addr = addr.assume_checked();

        // Check with 0 confirmations first (mempool)
        let unconfirmed = self
            .client
            .get_received_by_address(&checked_addr, Some(0))?;

        if unconfirmed == Amount::ZERO {
            return Ok(PaymentStatus::NotReceived);
        }

        // Check with required confirmations
        let confirmed = self
            .client
            .get_received_by_address(&checked_addr, Some(self.required_confirmations))?;

        // Estimate confirmations based on difference
        let estimated_confirmations = if confirmed >= unconfirmed {
            self.required_confirmations
        } else if confirmed > Amount::ZERO {
            // Some confirmations but not enough
            self.required_confirmations / 2 // Rough estimate
        } else {
            0
        };

        // Determine status based on amount and confirmations
        if confirmed >= expected_amount {
            if confirmed > expected_amount {
                Ok(PaymentStatus::Overpaid {
                    expected: expected_amount,
                    received: confirmed,
                })
            } else {
                Ok(PaymentStatus::Confirmed {
                    received_amount: confirmed,
                    confirmations: self.required_confirmations,
                })
            }
        } else if unconfirmed >= expected_amount {
            // Received full amount but waiting for confirmations
            Ok(PaymentStatus::Pending {
                received_amount: unconfirmed,
                confirmations: estimated_confirmations,
            })
        } else if unconfirmed > Amount::ZERO {
            // Received some but not enough
            Ok(PaymentStatus::Underpaid {
                expected: expected_amount,
                received: unconfirmed,
            })
        } else {
            Ok(PaymentStatus::NotReceived)
        }
    }

    /// Check payment and get detailed info including transaction ID
    pub fn check_payment_detailed(
        &self,
        address: &str,
        expected_amount: Amount,
    ) -> Result<PaymentDetails> {
        let status = self.check_payment(address, expected_amount)?;

        Ok(PaymentDetails {
            address: address.to_string(),
            expected_amount,
            status,
            txid: None, // Would need additional lookup for txid
        })
    }

    /// Get required confirmations setting
    pub fn required_confirmations(&self) -> u32 {
        self.required_confirmations
    }
}

/// Detailed payment information
#[derive(Debug, Clone, Serialize)]
pub struct PaymentDetails {
    /// Bitcoin address being monitored for the payment.
    pub address: String,
    /// Amount expected for this payment.
    pub expected_amount: Amount,
    /// Current status of the payment.
    pub status: PaymentStatus,
    /// Transaction ID of the payment, if known.
    pub txid: Option<String>,
}

/// Pending order for monitoring
#[derive(Debug, Clone)]
pub struct PendingOrder {
    /// Unique identifier for this order.
    pub order_id: uuid::Uuid,
    /// Bitcoin address assigned for this order's payment.
    pub address: String,
    /// Payment amount expected for this order.
    pub expected_amount: Amount,
    /// UTC timestamp when this order was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Payment event emitted by the monitor
#[derive(Debug, Clone)]
pub enum PaymentEvent {
    /// Payment received and confirmed
    Confirmed {
        /// Unique identifier of the order whose payment was confirmed.
        order_id: uuid::Uuid,
        /// Confirmed amount received.
        amount: Amount,
        /// Number of block confirmations at the time of this event.
        confirmations: u32,
    },
    /// Payment pending (in mempool or insufficient confirmations)
    Pending {
        /// Unique identifier of the order awaiting payment.
        order_id: uuid::Uuid,
        /// Amount detected so far (may be unconfirmed).
        amount: Amount,
        /// Current number of confirmations.
        confirmations: u32,
    },
    /// Underpayment detected
    Underpaid {
        /// Unique identifier of the underpaid order.
        order_id: uuid::Uuid,
        /// Amount that was expected.
        expected: Amount,
        /// Amount that was actually received.
        received: Amount,
    },
    /// Overpayment detected
    Overpaid {
        /// Unique identifier of the overpaid order.
        order_id: uuid::Uuid,
        /// Amount that was expected.
        expected: Amount,
        /// Amount that was actually received.
        received: Amount,
    },
    /// Order expired without payment
    Expired {
        /// Unique identifier of the expired order.
        order_id: uuid::Uuid,
    },
}

/// Configuration for payment monitor background task
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Polling interval in seconds
    pub poll_interval_secs: u64,
    /// Order expiry time in hours
    pub order_expiry_hours: u32,
    /// Required confirmations for payment to be confirmed
    pub required_confirmations: u32,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            order_expiry_hours: 24,
            required_confirmations: 3,
        }
    }
}

/// Background payment monitoring task
pub struct PaymentMonitorTask {
    client: Arc<BitcoinClient>,
    config: MonitorConfig,
    /// Channel to send payment events
    event_sender: Option<tokio::sync::mpsc::Sender<PaymentEvent>>,
    /// Shutdown signal
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl PaymentMonitorTask {
    /// Create a new payment monitor task
    pub fn new(
        client: Arc<BitcoinClient>,
        config: MonitorConfig,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self {
            client,
            config,
            event_sender: None,
            shutdown,
        }
    }

    /// Set the event sender channel
    pub fn with_event_sender(mut self, sender: tokio::sync::mpsc::Sender<PaymentEvent>) -> Self {
        self.event_sender = Some(sender);
        self
    }

    /// Start the monitoring loop (call in a spawned task)
    pub async fn run<F, Fut>(&mut self, get_pending_orders: F)
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Vec<PendingOrder>>,
    {
        let poll_interval = std::time::Duration::from_secs(self.config.poll_interval_secs);

        tracing::info!(
            poll_interval_secs = self.config.poll_interval_secs,
            required_confirmations = self.config.required_confirmations,
            "Payment monitor started"
        );

        loop {
            // Check shutdown signal
            if *self.shutdown.borrow() {
                tracing::info!("Payment monitor shutting down");
                break;
            }

            // Get pending orders
            let orders = get_pending_orders().await;

            if !orders.is_empty() {
                tracing::debug!(count = orders.len(), "Checking pending orders");
            }

            // Check each order
            for order in orders {
                if let Err(e) = self.check_order(&order).await {
                    tracing::warn!(
                        order_id = %order.order_id,
                        error = %e,
                        "Error checking payment"
                    );
                }
            }

            // Wait for next poll interval or shutdown
            tokio::select! {
                _ = tokio::time::sleep(poll_interval) => {}
                _ = self.shutdown.changed() => {
                    if *self.shutdown.borrow() {
                        tracing::info!("Payment monitor received shutdown signal");
                        break;
                    }
                }
            }
        }
    }

    /// Check payment status for a single order
    async fn check_order(&self, order: &PendingOrder) -> Result<()> {
        let monitor = PaymentMonitor::new(self.client.clone(), self.config.required_confirmations);

        // Check for expiry first
        let age = chrono::Utc::now() - order.created_at;
        if age.num_hours() >= self.config.order_expiry_hours as i64 {
            self.emit_event(PaymentEvent::Expired {
                order_id: order.order_id,
            })
            .await;
            return Ok(());
        }

        let status = monitor.check_payment(&order.address, order.expected_amount)?;

        let event = match status {
            PaymentStatus::Confirmed {
                received_amount,
                confirmations,
            } => Some(PaymentEvent::Confirmed {
                order_id: order.order_id,
                amount: received_amount,
                confirmations,
            }),
            PaymentStatus::Pending {
                received_amount,
                confirmations,
            } => Some(PaymentEvent::Pending {
                order_id: order.order_id,
                amount: received_amount,
                confirmations,
            }),
            PaymentStatus::Underpaid { expected, received } => Some(PaymentEvent::Underpaid {
                order_id: order.order_id,
                expected,
                received,
            }),
            PaymentStatus::Overpaid { expected, received } => Some(PaymentEvent::Overpaid {
                order_id: order.order_id,
                expected,
                received,
            }),
            PaymentStatus::NotReceived => None,
        };

        if let Some(event) = event {
            self.emit_event(event).await;
        }

        Ok(())
    }

    /// Emit a payment event
    async fn emit_event(&self, event: PaymentEvent) {
        tracing::info!(
            event = ?event,
            "Payment event"
        );

        if let Some(ref sender) = self.event_sender {
            if let Err(e) = sender.send(event.clone()).await {
                tracing::error!(error = %e, "Failed to send payment event");
            }
        }
    }
}

/// Create a shutdown signal pair for the monitor
pub fn create_shutdown_signal() -> (
    tokio::sync::watch::Sender<bool>,
    tokio::sync::watch::Receiver<bool>,
) {
    tokio::sync::watch::channel(false)
}
