use super::queue::JobQueue;
use super::types::{Job, JobResult, JobType};
use anyhow::Result;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

/// Worker configuration
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Worker poll interval when queue is empty
    pub poll_interval: Duration,
    /// Enable graceful shutdown
    pub graceful_shutdown: bool,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(1),
            graceful_shutdown: true,
        }
    }
}

/// Job worker that processes jobs from the queue
pub struct JobWorker {
    /// Worker ID
    id: usize,
    /// Job queue
    queue: Arc<JobQueue>,
    /// Worker configuration
    config: WorkerConfig,
    /// Worker task handle
    handle: Option<JoinHandle<()>>,
    /// Shutdown signal
    shutdown: Arc<tokio::sync::Notify>,
}

impl JobWorker {
    /// Create a new job worker
    pub fn new(id: usize, queue: Arc<JobQueue>) -> Self {
        Self {
            id,
            queue,
            config: WorkerConfig::default(),
            handle: None,
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Create a job worker with custom configuration
    pub fn with_config(id: usize, queue: Arc<JobQueue>, config: WorkerConfig) -> Self {
        Self {
            id,
            queue,
            config,
            handle: None,
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Start the worker
    pub async fn start(&mut self) -> Result<()> {
        if self.handle.is_some() {
            anyhow::bail!("Worker already started");
        }

        let id = self.id;
        let queue = self.queue.clone();
        let config = self.config.clone();
        let shutdown = self.shutdown.clone();

        let handle = tokio::spawn(async move {
            tracing::info!("Worker {} started", id);

            loop {
                tokio::select! {
                    _ = shutdown.notified() => {
                        tracing::info!("Worker {} shutting down", id);
                        break;
                    }
                    _ = Self::process_job_loop(id, queue.clone(), config.clone()) => {
                        // Loop continues
                    }
                }
            }

            tracing::info!("Worker {} stopped", id);
        });

        self.handle = Some(handle);

        Ok(())
    }

    /// Stop the worker
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(handle) = self.handle.take() {
            // Signal shutdown
            self.shutdown.notify_one();

            // Wait for worker to finish
            if self.config.graceful_shutdown {
                handle.await?;
            } else {
                handle.abort();
            }
        }

        Ok(())
    }

    /// Process job loop
    async fn process_job_loop(id: usize, queue: Arc<JobQueue>, config: WorkerConfig) {
        // Try to dequeue a job
        if let Some(job) = queue.dequeue().await {
            tracing::debug!("Worker {} processing job {}", id, job.id);

            // Execute the job
            let result = Self::execute_job(&job).await;

            // Handle result
            match result {
                JobResult::Success { data } => {
                    if let Err(e) = queue.complete(&job.id, data).await {
                        tracing::error!("Failed to mark job {} as completed: {}", job.id, e);
                    }
                }
                JobResult::Failure { error } => {
                    if let Err(e) = queue.fail(&job.id, error).await {
                        tracing::error!("Failed to mark job {} as failed: {}", job.id, e);
                    }
                }
                JobResult::Retry {
                    reason,
                    delay_seconds,
                } => {
                    tracing::warn!("Job {} will be retried: {}", job.id, reason);
                    sleep(Duration::from_secs(delay_seconds)).await;
                    if let Err(e) = queue.fail(&job.id, reason).await {
                        tracing::error!("Failed to requeue job {}: {}", job.id, e);
                    }
                }
            }
        } else {
            // No jobs available, wait for notification or poll interval
            tokio::select! {
                _ = queue.wait_for_job() => {
                    // New job available
                }
                _ = sleep(config.poll_interval) => {
                    // Poll interval elapsed
                }
            }
        }
    }

    /// Execute a job
    async fn execute_job(job: &Job) -> JobResult {
        match &job.job_type {
            JobType::SendEmail { to, subject, body } => {
                Self::execute_send_email(to, subject, body).await
            }
            JobType::SendSms { to, message } => Self::execute_send_sms(to, message).await,
            JobType::SendPush {
                user_id,
                title,
                body,
            } => Self::execute_send_push(*user_id, title, body).await,
            JobType::ProcessOrderConfirmation { order_id } => {
                Self::execute_process_order_confirmation(*order_id).await
            }
            JobType::ProcessTradeSettlement { trade_id } => {
                Self::execute_process_trade_settlement(*trade_id).await
            }
            JobType::KycVerificationReminder { user_id } => {
                Self::execute_kyc_verification_reminder(*user_id).await
            }
            JobType::CleanupExpiredSessions => Self::execute_cleanup_expired_sessions().await,
            JobType::GenerateAnalyticsReport { report_type } => {
                Self::execute_generate_analytics_report(report_type).await
            }
            JobType::ArchiveOldData { before_date } => {
                Self::execute_archive_old_data(*before_date).await
            }
            JobType::RefreshMaterializedViews => Self::execute_refresh_materialized_views().await,
            JobType::Custom { name, payload } => Self::execute_custom(name, payload).await,
        }
    }

    // Job execution implementations

    async fn execute_send_email(to: &str, subject: &str, body: &str) -> JobResult {
        tracing::info!("Sending email to: {}, subject: {}", to, subject);

        // Note: Email sending should be integrated with the EmailNotifier
        // For now, this is a placeholder that would use the notification system
        // In production, initialize EmailNotifier with proper SMTP config

        // Validate email format
        if !to.contains('@') {
            return JobResult::Failure {
                error: format!("Invalid email address: {}", to),
            };
        }

        // Log email details for debugging
        tracing::debug!("Email body: {}", body);

        // Simulate email sending (replace with actual EmailNotifier in production)
        match Self::simulate_email_send(to, subject, body).await {
            Ok(_) => JobResult::Success {
                data: Some(
                    serde_json::json!({
                        "sent_to": to,
                        "subject": subject
                    })
                    .to_string(),
                ),
            },
            Err(e) => JobResult::Retry {
                reason: format!("Failed to send email: {}", e),
                delay_seconds: 60, // Retry after 1 minute
            },
        }
    }

    async fn execute_send_sms(to: &str, message: &str) -> JobResult {
        tracing::info!("Sending SMS to: {}", to);

        // Validate phone number format (basic validation)
        if !to.starts_with('+') || to.len() < 10 {
            return JobResult::Failure {
                error: format!("Invalid phone number format: {}", to),
            };
        }

        tracing::debug!("SMS message: {}", message);

        // In production, integrate with SmsNotifier configured with Twilio/AWS SNS
        // For now, simulate SMS sending
        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "sent_to": to,
                    "message_length": message.len()
                })
                .to_string(),
            ),
        }
    }

    async fn execute_send_push(user_id: uuid::Uuid, title: &str, body: &str) -> JobResult {
        tracing::info!("Sending push notification to user: {}", user_id);

        tracing::debug!("Push title: {}, body: {}", title, body);

        // In production, integrate with PushNotifier configured with FCM/APNS
        // For now, simulate push notification
        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "user_id": user_id,
                    "title": title
                })
                .to_string(),
            ),
        }
    }

    async fn execute_process_order_confirmation(order_id: uuid::Uuid) -> JobResult {
        tracing::info!("Processing order confirmation for order: {}", order_id);

        // In production:
        // 1. Fetch order details from database
        // 2. Verify payment/transaction
        // 3. Update order status
        // 4. Send confirmation email to user
        // 5. Trigger any post-order webhooks

        // For now, simulate order confirmation processing
        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "order_id": order_id,
                    "confirmed": true
                })
                .to_string(),
            ),
        }
    }

    async fn execute_process_trade_settlement(trade_id: uuid::Uuid) -> JobResult {
        tracing::info!("Processing trade settlement for trade: {}", trade_id);

        // In production:
        // 1. Fetch trade details from database
        // 2. Verify all parties' balances
        // 3. Execute token transfers
        // 4. Update balances
        // 5. Mark trade as settled
        // 6. Send settlement notifications

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "trade_id": trade_id,
                    "settled": true
                })
                .to_string(),
            ),
        }
    }

    async fn execute_kyc_verification_reminder(user_id: uuid::Uuid) -> JobResult {
        tracing::info!("Sending KYC verification reminder to user: {}", user_id);

        // In production:
        // 1. Fetch user details from database
        // 2. Check KYC status
        // 3. If pending, send reminder email/push notification
        // 4. Update reminder sent timestamp

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "user_id": user_id,
                    "reminder_sent": true
                })
                .to_string(),
            ),
        }
    }

    async fn execute_cleanup_expired_sessions() -> JobResult {
        tracing::info!("Cleaning up expired sessions");

        // In production:
        // 1. Connect to session store (Redis/database)
        // 2. Find sessions older than expiry time
        // 3. Delete expired sessions
        // 4. Log cleanup statistics

        // For now, simulate cleanup
        let cleaned_count = 0; // Would be actual count in production

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "cleaned_sessions": cleaned_count
                })
                .to_string(),
            ),
        }
    }

    async fn execute_generate_analytics_report(report_type: &str) -> JobResult {
        tracing::info!("Generating analytics report: {}", report_type);

        // In production:
        // 1. Fetch analytics data based on report_type
        // 2. Aggregate data (daily/weekly/monthly)
        // 3. Generate report (PDF/CSV/JSON)
        // 4. Store report or email to admins
        // 5. Update report generation timestamp

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "report_type": report_type,
                    "generated": true
                })
                .to_string(),
            ),
        }
    }

    async fn execute_archive_old_data(before_date: chrono::DateTime<chrono::Utc>) -> JobResult {
        tracing::info!("Archiving data before: {}", before_date);

        // In production:
        // 1. Query old records (audit logs, old trades, etc.)
        // 2. Export to archive storage (S3, cold storage)
        // 3. Verify archive integrity
        // 4. Delete archived records from main database
        // 5. Log archival statistics

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "archived_before": before_date,
                    "records_archived": 0
                })
                .to_string(),
            ),
        }
    }

    async fn execute_refresh_materialized_views() -> JobResult {
        tracing::info!("Refreshing materialized views");

        // In production:
        // 1. Execute REFRESH MATERIALIZED VIEW for each view
        // 2. Handle concurrency (CONCURRENTLY option)
        // 3. Log refresh duration
        // 4. Update view statistics

        // Example materialized views:
        // - token_stats (holder counts, volumes)
        // - user_portfolios (aggregated holdings)
        // - trade_summaries (24h volumes, prices)

        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "views_refreshed": 0
                })
                .to_string(),
            ),
        }
    }

    async fn execute_custom(name: &str, payload: &str) -> JobResult {
        tracing::info!("Executing custom job: {}", name);
        tracing::debug!("Custom job payload: {}", payload);

        // In production:
        // Custom job handler registry where jobs can be registered dynamically
        // This allows extending the system without modifying core worker code

        // For now, just log and succeed
        JobResult::Success {
            data: Some(
                serde_json::json!({
                    "job_name": name,
                    "payload_size": payload.len()
                })
                .to_string(),
            ),
        }
    }

    /// Helper: Simulate email sending (replace with actual EmailNotifier)
    async fn simulate_email_send(_to: &str, _subject: &str, _body: &str) -> Result<()> {
        // Simulate network delay
        sleep(Duration::from_millis(100)).await;

        // Simulate occasional failures for testing retry logic
        // In production, this would be actual SMTP sending via EmailNotifier

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::queue::JobQueueConfig;

    #[tokio::test]
    async fn test_worker_creation() {
        let queue = Arc::new(JobQueue::new(JobQueueConfig::default()));
        let worker = JobWorker::new(0, queue);

        assert_eq!(worker.id, 0);
        assert!(worker.handle.is_none());
    }

    #[tokio::test]
    async fn test_worker_lifecycle() {
        let queue = Arc::new(JobQueue::new(JobQueueConfig::default()));
        let mut worker = JobWorker::new(0, queue);

        // Start worker
        worker.start().await.unwrap();
        assert!(worker.handle.is_some());

        // Give worker time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop worker
        worker.stop().await.unwrap();
        assert!(worker.handle.is_none());
    }
}
