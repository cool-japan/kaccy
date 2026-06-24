use super::queue::JobQueue;
use super::types::{Job, JobResult, JobType};
use crate::notifications::{
    EmailConfig, EmailNotifier, Notification, PushConfig, PushNotifier, SmsConfig, SmsNotifier,
};
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

    // ─────────────────────────────────────────────────────────────────────────
    // Notification job executors — real notifier paths, no simulation shims
    // ─────────────────────────────────────────────────────────────────────────

    /// Build an `EmailConfig` from environment variables.
    ///
    /// Required env vars (all must be non-empty for actual delivery):
    ///   - `SMTP_HOST`       — SMTP server hostname
    ///   - `SMTP_PORT`       — SMTP server port (default 587)
    ///   - `SMTP_USERNAME`   — SMTP login username
    ///   - `SMTP_PASSWORD`   — SMTP login password
    ///   - `EMAIL_FROM`      — Sender address (default: `noreply@kaccy.io`)
    ///   - `EMAIL_FROM_NAME` — Sender display name (default: `Kaccy`)
    ///   - `EMAIL_USE_TLS`   — `"false"` disables STARTTLS (default: enabled)
    ///   - `EMAIL_ENABLED`   — must be `"true"` to arm the notifier
    fn build_email_config() -> EmailConfig {
        let enabled = std::env::var("EMAIL_ENABLED")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let smtp_port = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(587);

        let use_tls = std::env::var("EMAIL_USE_TLS")
            .map(|v| !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);

        EmailConfig {
            smtp_host: std::env::var("SMTP_HOST").unwrap_or_default(),
            smtp_port,
            smtp_username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
            smtp_password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
            from_address: std::env::var("EMAIL_FROM")
                .unwrap_or_else(|_| "noreply@kaccy.io".to_string()),
            from_name: std::env::var("EMAIL_FROM_NAME").unwrap_or_else(|_| "Kaccy".to_string()),
            use_tls,
            enabled,
        }
    }

    /// Build an `SmsConfig` from environment variables.
    ///
    /// Required env vars:
    ///   - `SMS_PROVIDER`    — `"twilio"` (default) or `"aws_sns"`
    ///   - `SMS_API_KEY`     — Twilio AccountSid or AWS Access Key ID
    ///   - `SMS_API_SECRET`  — Twilio Auth Token or AWS Secret Access Key
    ///   - `SMS_FROM_NUMBER` — E.164 sender number or AWS SNS region prefix
    ///   - `SMS_ENABLED`     — must be `"true"` to arm the notifier
    fn build_sms_config() -> SmsConfig {
        let enabled = std::env::var("SMS_ENABLED")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        SmsConfig {
            provider: std::env::var("SMS_PROVIDER").unwrap_or_else(|_| "twilio".to_string()),
            api_key: std::env::var("SMS_API_KEY").unwrap_or_default(),
            api_secret: std::env::var("SMS_API_SECRET").unwrap_or_default(),
            from_number: std::env::var("SMS_FROM_NUMBER").unwrap_or_default(),
            enabled,
        }
    }

    /// Build a `PushConfig` from environment variables.
    ///
    /// Required env vars (FCM v1 path):
    ///   - `PUSH_PROVIDER`          — `"fcm"` (default) or `"apns"`
    ///   - `FCM_PROJECT_ID`         — Firebase project ID
    ///   - `FCM_OAUTH2_BEARER`      — Short-lived FCM OAuth2 bearer token
    ///   - `FCM_DEVICE_TOKEN`       — Target device registration token
    ///   - `PUSH_ENABLED`           — must be `"true"` to arm the notifier
    ///
    /// FCM legacy path (fallback when v1 fields absent):
    ///   - `PUSH_API_KEY`           — FCM legacy server key
    ///
    /// APNS path (when `PUSH_PROVIDER=apns`):
    ///   - `APNS_KEY_ID`, `APNS_TEAM_ID`, `APNS_BUNDLE_ID`
    ///   - `APNS_PRIVATE_KEY_PEM`   — Contents of the `.p8` file
    ///   - `APNS_DEVICE_TOKEN`      — Target device hex token
    ///   - `APNS_SANDBOX`           — `"true"` to use sandbox endpoint
    fn build_push_config() -> PushConfig {
        let enabled = std::env::var("PUSH_ENABLED")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let apns_sandbox = std::env::var("APNS_SANDBOX")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        PushConfig {
            provider: std::env::var("PUSH_PROVIDER").unwrap_or_else(|_| "fcm".to_string()),
            api_key: std::env::var("PUSH_API_KEY").unwrap_or_default(),
            api_secret: String::new(),
            enabled,
            fcm_project_id: std::env::var("FCM_PROJECT_ID").ok(),
            fcm_oauth2_bearer: std::env::var("FCM_OAUTH2_BEARER").ok(),
            fcm_device_token: std::env::var("FCM_DEVICE_TOKEN").ok(),
            apns_key_id: std::env::var("APNS_KEY_ID").ok(),
            apns_team_id: std::env::var("APNS_TEAM_ID").ok(),
            apns_bundle_id: std::env::var("APNS_BUNDLE_ID").ok(),
            apns_private_key_pem: std::env::var("APNS_PRIVATE_KEY_PEM").ok(),
            apns_device_token: std::env::var("APNS_DEVICE_TOKEN").ok(),
            apns_sandbox,
        }
    }

    /// Map the result of `EmailNotifier::send` to a `JobResult`.
    ///
    /// `EmailNotifier::send` returns:
    ///   - `Ok(_)` — SMTP delivery actually succeeded → `JobResult::Success`
    ///   - `Err(e)` — disabled, unconfigured, or SMTP failure → `JobResult::Failure`
    ///
    /// There is no retry escalation here: retries are driven by the queue's
    /// existing retry/failure-handling logic.
    fn map_email_send_result(
        to: &str,
        subject: &str,
        result: anyhow::Result<crate::notifications::NotificationId>,
    ) -> JobResult {
        match result {
            Ok(_id) => {
                tracing::info!("Email sent to {} with subject '{}'", to, subject);
                JobResult::Success {
                    data: Some(
                        serde_json::json!({
                            "sent_to": to,
                            "subject": subject
                        })
                        .to_string(),
                    ),
                }
            }
            Err(e) => {
                tracing::error!(
                    "Email job failed for {} (subject: '{}'): {}",
                    to,
                    subject,
                    e
                );
                JobResult::Failure {
                    error: format!("email send failed: {}", e),
                }
            }
        }
    }

    /// Map the result of `SmsNotifier::send` to a `JobResult`.
    ///
    /// `SmsNotifier::send` returns:
    ///   - `Ok(_)` when disabled (status = Failed internally) — we still treat
    ///     this as a job failure because no message was actually sent.
    ///   - `Err(e)` when credentials are missing or provider rejects — failure.
    ///
    /// We distinguish the two `Ok` cases via the `enabled` flag on the config
    /// so that a disabled provider does not silently succeed at the job level.
    fn map_sms_send_result(
        to: &str,
        sms_enabled: bool,
        result: anyhow::Result<crate::notifications::NotificationId>,
    ) -> JobResult {
        match result {
            Ok(_id) if !sms_enabled => {
                // SmsNotifier returns Ok when disabled but does not send.
                tracing::warn!(
                    "SMS job for {} produced no delivery: SMS provider disabled",
                    to
                );
                JobResult::Failure {
                    error: "SMS notifications disabled — no message delivered".to_string(),
                }
            }
            Ok(_id) => {
                tracing::info!("SMS sent to {}", to);
                JobResult::Success {
                    data: Some(
                        serde_json::json!({
                            "sent_to": to
                        })
                        .to_string(),
                    ),
                }
            }
            Err(e) => {
                tracing::error!("SMS job failed for {}: {}", to, e);
                JobResult::Failure {
                    error: format!("SMS send failed: {}", e),
                }
            }
        }
    }

    /// Map the result of `PushNotifier::send` to a `JobResult`.
    ///
    /// `PushNotifier::send` returns:
    ///   - `Ok(_)` when disabled (status = Failed internally) — treat as failure.
    ///   - `Ok(_)` when validation fails (title/body too long) — treat as failure.
    ///   - `Err(e)` when credentials missing or provider rejects — failure.
    ///
    /// We use the `enabled` flag to detect the disabled-Ok case.
    fn map_push_send_result(
        user_id: uuid::Uuid,
        push_enabled: bool,
        result: anyhow::Result<crate::notifications::NotificationId>,
    ) -> JobResult {
        match result {
            Ok(_id) if !push_enabled => {
                tracing::warn!(
                    "Push job for user {} produced no delivery: push provider disabled",
                    user_id
                );
                JobResult::Failure {
                    error: "push notifications disabled — no notification delivered".to_string(),
                }
            }
            Ok(_id) => {
                tracing::info!("Push notification sent to user {}", user_id);
                JobResult::Success {
                    data: Some(
                        serde_json::json!({
                            "user_id": user_id
                        })
                        .to_string(),
                    ),
                }
            }
            Err(e) => {
                tracing::error!("Push job failed for user {}: {}", user_id, e);
                JobResult::Failure {
                    error: format!("push send failed: {}", e),
                }
            }
        }
    }

    /// Execute a `SendEmail` job using the real `EmailNotifier`.
    ///
    /// Configuration is sourced from env vars via `build_email_config`.  The
    /// notifier returns `Err` for every non-delivery path (disabled, no SMTP
    /// transport, SMTP error), so `JobResult::Success` is emitted only when the
    /// message is actually delivered.
    async fn execute_send_email(to: &str, subject: &str, body: &str) -> JobResult {
        tracing::info!("Sending email to: {}, subject: {}", to, subject);

        // Basic format guard before touching the network.
        if !to.contains('@') {
            return JobResult::Failure {
                error: format!("invalid email address: {}", to),
            };
        }

        let config = Self::build_email_config();
        let notifier = EmailNotifier::new(config);

        let notification =
            Notification::email(to.to_string(), subject.to_string(), body.to_string());

        let result = notifier.send(notification).await;
        Self::map_email_send_result(to, subject, result)
    }

    /// Execute a `SendSms` job using the real `SmsNotifier`.
    ///
    /// Configuration is sourced from env vars via `build_sms_config`.  When
    /// SMS is disabled or credentials are absent the notifier fails honestly and
    /// the job is marked `Failure`.
    async fn execute_send_sms(to: &str, message: &str) -> JobResult {
        tracing::info!("Sending SMS to: {}", to);

        let config = Self::build_sms_config();
        let sms_enabled = config.enabled;
        let notifier = SmsNotifier::new(config);

        let notification = Notification::sms(to.to_string(), message.to_string());

        let result = notifier.send(notification).await;
        Self::map_sms_send_result(to, sms_enabled, result)
    }

    /// Execute a `SendPush` job using the real `PushNotifier`.
    ///
    /// Configuration is sourced from env vars via `build_push_config`.  When
    /// push is disabled or credentials are absent the notifier fails honestly and
    /// the job is marked `Failure`.
    async fn execute_send_push(user_id: uuid::Uuid, title: &str, body: &str) -> JobResult {
        tracing::info!("Sending push notification to user: {}", user_id);

        let config = Self::build_push_config();
        let push_enabled = config.enabled;
        let notifier = PushNotifier::new(config);

        let notification = Notification::push(user_id, title.to_string(), body.to_string());

        let result = notifier.send(notification).await;
        Self::map_push_send_result(user_id, push_enabled, result)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Non-notification job executors
    // ─────────────────────────────────────────────────────────────────────────

    async fn execute_process_order_confirmation(order_id: uuid::Uuid) -> JobResult {
        tracing::info!("Processing order confirmation for order: {}", order_id);

        // In production:
        // 1. Fetch order details from database
        // 2. Verify payment/transaction
        // 3. Update order status
        // 4. Send confirmation email to user
        // 5. Trigger any post-order webhooks

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

        let cleaned_count = 0_u64; // Would be actual count in production

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

        // Custom job handler registry where jobs can be registered dynamically.
        // This allows extending the system without modifying core worker code.

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
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::queue::JobQueueConfig;
    use crate::jobs::types::{Job, JobResult, JobType};

    // ── Worker lifecycle ─────────────────────────────────────────────────────

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

    // ── Email: no config → Failure (never Success) ───────────────────────────

    /// A `SendEmail` job with no provider configured must produce `JobResult::Failure`.
    /// This test exercises the real `EmailNotifier` path with an empty env
    /// (EMAIL_ENABLED not set → disabled → Err → Failure).
    #[tokio::test]
    async fn test_email_job_unconfigured_yields_failure() {
        // Ensure test isolation: unset all email env vars.
        for var in &[
            "EMAIL_ENABLED",
            "SMTP_HOST",
            "SMTP_USERNAME",
            "SMTP_PASSWORD",
        ] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }

        let job = Job::new(JobType::SendEmail {
            to: "user@example.com".to_string(),
            subject: "Hello".to_string(),
            body: "Test body".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;

        assert!(
            result.is_failure(),
            "expected Failure when email provider is unconfigured, got: {:?}",
            result
        );

        // Verify the error message is meaningful (not a fake success message).
        if let JobResult::Failure { error } = result {
            assert!(!error.is_empty(), "Failure error message must not be empty");
        }
    }

    /// Invalid email address must fail immediately (format guard), not call the notifier.
    #[tokio::test]
    async fn test_email_job_invalid_address_yields_failure() {
        let job = Job::new(JobType::SendEmail {
            to: "not-an-email".to_string(),
            subject: "Subject".to_string(),
            body: "Body".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;

        assert!(
            result.is_failure(),
            "invalid email address must produce Failure"
        );
        if let JobResult::Failure { error } = result {
            assert!(
                error.contains("invalid email address") || error.contains('@'),
                "error must describe the format problem, got: {error}"
            );
        }
    }

    // ── SMS: no config → Failure (never Success) ─────────────────────────────

    /// A `SendSms` job with no provider configured must produce `JobResult::Failure`.
    /// SMS_ENABLED is not set → disabled → `SmsNotifier::send` returns `Ok`
    /// with internal Failed status → `map_sms_send_result` converts to Failure.
    #[tokio::test]
    async fn test_sms_job_unconfigured_yields_failure() {
        for var in &["SMS_ENABLED", "SMS_API_KEY", "SMS_API_SECRET"] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }

        let job = Job::new(JobType::SendSms {
            to: "+12025551234".to_string(),
            message: "Test message".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;

        assert!(
            result.is_failure(),
            "expected Failure when SMS provider is unconfigured, got: {:?}",
            result
        );
    }

    /// SMS with `enabled=true` but empty credentials must also fail (Err path).
    #[tokio::test]
    async fn test_sms_job_enabled_no_credentials_yields_failure() {
        // SAFETY: test-only env setup
        unsafe {
            std::env::set_var("SMS_ENABLED", "true");
            std::env::remove_var("SMS_API_KEY");
            std::env::remove_var("SMS_API_SECRET");
        }
        // Restore after test
        let _guard = EnvGuard(vec!["SMS_ENABLED"]);

        let job = Job::new(JobType::SendSms {
            to: "+12025551234".to_string(),
            message: "Test".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;
        assert!(
            result.is_failure(),
            "enabled SMS with missing credentials must produce Failure, got: {:?}",
            result
        );
    }

    // ── Push: no config → Failure (never Success) ────────────────────────────

    /// A `SendPush` job with no provider configured must produce `JobResult::Failure`.
    /// PUSH_ENABLED not set → disabled → `PushNotifier::send` returns `Ok`
    /// with internal Failed status → `map_push_send_result` converts to Failure.
    #[tokio::test]
    async fn test_push_job_unconfigured_yields_failure() {
        for var in &[
            "PUSH_ENABLED",
            "PUSH_API_KEY",
            "FCM_PROJECT_ID",
            "FCM_OAUTH2_BEARER",
        ] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }

        let job = Job::new(JobType::SendPush {
            user_id: uuid::Uuid::new_v4(),
            title: "Test Title".to_string(),
            body: "Test Body".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;

        assert!(
            result.is_failure(),
            "expected Failure when push provider is unconfigured, got: {:?}",
            result
        );
    }

    /// Push with `enabled=true` but no FCM credentials must also fail (Err path).
    #[tokio::test]
    async fn test_push_job_enabled_no_credentials_yields_failure() {
        // SAFETY: test-only env setup
        unsafe {
            std::env::set_var("PUSH_ENABLED", "true");
            std::env::set_var("PUSH_PROVIDER", "fcm");
        }
        for var in &[
            "FCM_PROJECT_ID",
            "FCM_OAUTH2_BEARER",
            "FCM_DEVICE_TOKEN",
            "PUSH_API_KEY",
        ] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }
        let _guard = EnvGuard(vec!["PUSH_ENABLED", "PUSH_PROVIDER"]);

        let job = Job::new(JobType::SendPush {
            user_id: uuid::Uuid::new_v4(),
            title: "Title".to_string(),
            body: "Body".to_string(),
        });

        let result = JobWorker::execute_job(&job).await;
        assert!(
            result.is_failure(),
            "enabled push with missing FCM credentials must produce Failure, got: {:?}",
            result
        );
    }

    // ── Config builder unit tests ─────────────────────────────────────────────

    /// Default (no env vars set) email config must have `enabled = false`.
    #[test]
    fn test_build_email_config_defaults_to_disabled() {
        for var in &[
            "EMAIL_ENABLED",
            "SMTP_HOST",
            "SMTP_USERNAME",
            "SMTP_PASSWORD",
        ] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }
        let config = JobWorker::build_email_config();
        assert!(
            !config.enabled,
            "email config must be disabled when EMAIL_ENABLED is absent"
        );
        assert!(
            config.smtp_username.is_empty(),
            "smtp_username must be empty when SMTP_USERNAME is absent"
        );
    }

    /// `EMAIL_ENABLED=true` propagates into the config.
    #[test]
    fn test_build_email_config_reads_enabled_flag() {
        // SAFETY: test-only env setup
        unsafe {
            std::env::set_var("EMAIL_ENABLED", "true");
        }
        let _guard = EnvGuard(vec!["EMAIL_ENABLED"]);
        let config = JobWorker::build_email_config();
        assert!(config.enabled);
    }

    /// Default (no env vars set) SMS config must have `enabled = false`.
    #[test]
    fn test_build_sms_config_defaults_to_disabled() {
        for var in &["SMS_ENABLED", "SMS_API_KEY", "SMS_API_SECRET"] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }
        let config = JobWorker::build_sms_config();
        assert!(!config.enabled, "SMS config must be disabled by default");
        assert!(config.api_key.is_empty());
    }

    /// Default (no env vars set) push config must have `enabled = false`.
    #[test]
    fn test_build_push_config_defaults_to_disabled() {
        for var in &["PUSH_ENABLED", "PUSH_API_KEY", "FCM_PROJECT_ID"] {
            // SAFETY: test-only env setup
            unsafe {
                std::env::remove_var(var);
            }
        }
        let config = JobWorker::build_push_config();
        assert!(!config.enabled, "push config must be disabled by default");
        assert!(config.fcm_project_id.is_none());
    }

    // ── map_* unit tests: explicit result mapping without network ─────────────

    /// `map_email_send_result`: `Ok` input → `Success`.
    #[test]
    fn test_map_email_send_result_ok_yields_success() {
        let id = uuid::Uuid::new_v4();
        let result = JobWorker::map_email_send_result("a@b.com", "Subj", Ok(id));
        assert!(result.is_success());
    }

    /// `map_email_send_result`: `Err` input → `Failure`.
    #[test]
    fn test_map_email_send_result_err_yields_failure() {
        let result = JobWorker::map_email_send_result(
            "a@b.com",
            "Subj",
            Err(anyhow::anyhow!("SMTP refused")),
        );
        assert!(result.is_failure());
        if let JobResult::Failure { error } = result {
            assert!(error.contains("SMTP refused"));
        }
    }

    /// `map_sms_send_result`: `Ok` with enabled=true → `Success`.
    #[test]
    fn test_map_sms_send_result_ok_enabled_yields_success() {
        let id = uuid::Uuid::new_v4();
        let result = JobWorker::map_sms_send_result("+12025551234", true, Ok(id));
        assert!(result.is_success());
    }

    /// `map_sms_send_result`: `Ok` with enabled=false → `Failure` (disabled path).
    #[test]
    fn test_map_sms_send_result_ok_disabled_yields_failure() {
        let id = uuid::Uuid::new_v4();
        let result = JobWorker::map_sms_send_result("+12025551234", false, Ok(id));
        assert!(
            result.is_failure(),
            "disabled SMS provider must produce Failure even when notifier returns Ok"
        );
    }

    /// `map_sms_send_result`: `Err` → `Failure` regardless of enabled flag.
    #[test]
    fn test_map_sms_send_result_err_yields_failure() {
        let result = JobWorker::map_sms_send_result(
            "+12025551234",
            true,
            Err(anyhow::anyhow!("Twilio error 21211")),
        );
        assert!(result.is_failure());
        if let JobResult::Failure { error } = result {
            assert!(error.contains("Twilio error 21211"));
        }
    }

    /// `map_push_send_result`: `Ok` with enabled=true → `Success`.
    #[test]
    fn test_map_push_send_result_ok_enabled_yields_success() {
        let uid = uuid::Uuid::new_v4();
        let id = uuid::Uuid::new_v4();
        let result = JobWorker::map_push_send_result(uid, true, Ok(id));
        assert!(result.is_success());
    }

    /// `map_push_send_result`: `Ok` with enabled=false → `Failure` (disabled path).
    #[test]
    fn test_map_push_send_result_ok_disabled_yields_failure() {
        let uid = uuid::Uuid::new_v4();
        let id = uuid::Uuid::new_v4();
        let result = JobWorker::map_push_send_result(uid, false, Ok(id));
        assert!(
            result.is_failure(),
            "disabled push provider must produce Failure even when notifier returns Ok"
        );
    }

    /// `map_push_send_result`: `Err` → `Failure` regardless of enabled flag.
    #[test]
    fn test_map_push_send_result_err_yields_failure() {
        let uid = uuid::Uuid::new_v4();
        let result = JobWorker::map_push_send_result(
            uid,
            true,
            Err(anyhow::anyhow!("FCM credentials not configured")),
        );
        assert!(result.is_failure());
        if let JobResult::Failure { error } = result {
            assert!(error.contains("FCM credentials not configured"));
        }
    }

    // ── Non-notification jobs still succeed ───────────────────────────────────

    #[tokio::test]
    async fn test_order_confirmation_job_succeeds() {
        let job = Job::new(JobType::ProcessOrderConfirmation {
            order_id: uuid::Uuid::new_v4(),
        });
        let result = JobWorker::execute_job(&job).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_cleanup_sessions_job_succeeds() {
        let job = Job::new(JobType::CleanupExpiredSessions);
        let result = JobWorker::execute_job(&job).await;
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_custom_job_succeeds() {
        let job = Job::new(JobType::Custom {
            name: "test-job".to_string(),
            payload: r#"{"key":"value"}"#.to_string(),
        });
        let result = JobWorker::execute_job(&job).await;
        assert!(result.is_success());
    }

    // ── Helper: RAII guard that unsets env vars after the test ────────────────

    struct EnvGuard(Vec<&'static str>);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for var in &self.0 {
                // SAFETY: test-only env setup
                unsafe {
                    std::env::remove_var(var);
                }
            }
        }
    }
}
