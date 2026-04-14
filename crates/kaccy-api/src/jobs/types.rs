use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a job
pub type JobId = Uuid;

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is pending execution
    Pending,
    /// Job is currently being processed
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed
    Failed,
    /// Job was cancelled
    Cancelled,
    /// Job is scheduled for future execution
    Scheduled,
}

/// Job types supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum JobType {
    /// Send email notification
    SendEmail {
        /// Recipient email address
        to: String,
        /// Email subject line
        subject: String,
        /// Email body content
        body: String,
    },
    /// Send SMS notification
    SendSms {
        /// Recipient phone number
        to: String,
        /// SMS message text
        message: String,
    },
    /// Send push notification
    SendPush {
        /// Target user ID
        user_id: Uuid,
        /// Notification title
        title: String,
        /// Notification body text
        body: String,
    },
    /// Process order confirmation
    ProcessOrderConfirmation {
        /// Order ID to confirm
        order_id: Uuid,
    },
    /// Process trade settlement
    ProcessTradeSettlement {
        /// Trade ID to settle
        trade_id: Uuid,
    },
    /// Process KYC verification reminder
    KycVerificationReminder {
        /// User ID to remind
        user_id: Uuid,
    },
    /// Clean up expired sessions
    CleanupExpiredSessions,
    /// Generate analytics report
    GenerateAnalyticsReport {
        /// Type of report to generate
        report_type: String,
    },
    /// Archive old data
    ArchiveOldData {
        /// Archive data before this date
        before_date: DateTime<Utc>,
    },
    /// Refresh materialized views (if using)
    RefreshMaterializedViews,
    /// Custom job type with JSON payload
    Custom {
        /// Job name
        name: String,
        /// Job payload as JSON string
        payload: String,
    },
}

/// Job definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique job identifier
    pub id: JobId,
    /// Job type and parameters
    pub job_type: JobType,
    /// Job status
    pub status: JobStatus,
    /// Number of retry attempts
    pub attempts: u32,
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Job priority (higher = more urgent)
    pub priority: i32,
    /// When the job was created
    pub created_at: DateTime<Utc>,
    /// When the job was last updated
    pub updated_at: DateTime<Utc>,
    /// When the job should be executed (for scheduled jobs)
    pub scheduled_at: Option<DateTime<Utc>>,
    /// When the job started processing
    pub started_at: Option<DateTime<Utc>>,
    /// When the job completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Error message if failed
    pub error: Option<String>,
    /// Job result data (JSON)
    pub result: Option<String>,
}

impl Job {
    /// Create a new job
    pub fn new(job_type: JobType) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            job_type,
            status: JobStatus::Pending,
            attempts: 0,
            max_attempts: 3,
            priority: 0,
            created_at: now,
            updated_at: now,
            scheduled_at: None,
            started_at: None,
            completed_at: None,
            error: None,
            result: None,
        }
    }

    /// Create a scheduled job
    pub fn scheduled(job_type: JobType, scheduled_at: DateTime<Utc>) -> Self {
        let mut job = Self::new(job_type);
        job.scheduled_at = Some(scheduled_at);
        job.status = JobStatus::Scheduled;
        job
    }

    /// Set job priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retry attempts
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Check if job can be retried
    pub fn can_retry(&self) -> bool {
        self.attempts < self.max_attempts
    }

    /// Check if job is ready to execute (for scheduled jobs)
    pub fn is_ready(&self) -> bool {
        match self.scheduled_at {
            Some(scheduled_at) => Utc::now() >= scheduled_at,
            None => true,
        }
    }
}

/// Result of job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobResult {
    /// Job succeeded with optional result data
    Success {
        /// Optional result data as JSON string
        data: Option<String>,
    },
    /// Job failed with error message
    Failure {
        /// Error description
        error: String,
    },
    /// Job should be retried later
    Retry {
        /// Reason for retry
        reason: String,
        /// Delay before retry in seconds
        delay_seconds: u64,
    },
}

impl JobResult {
    /// Check if result indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, JobResult::Success { .. })
    }

    /// Check if result indicates failure
    pub fn is_failure(&self) -> bool {
        matches!(self, JobResult::Failure { .. })
    }

    /// Check if result indicates retry
    pub fn is_retry(&self) -> bool {
        matches!(self, JobResult::Retry { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        let job = Job::new(JobType::SendEmail {
            to: "test@example.com".to_string(),
            subject: "Test".to_string(),
            body: "Test body".to_string(),
        });

        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.attempts, 0);
        assert_eq!(job.max_attempts, 3);
    }

    #[test]
    fn test_scheduled_job() {
        let future = Utc::now() + chrono::Duration::hours(1);
        let job = Job::scheduled(JobType::CleanupExpiredSessions, future);

        assert_eq!(job.status, JobStatus::Scheduled);
        assert!(job.scheduled_at.is_some());
        assert!(!job.is_ready());
    }

    #[test]
    fn test_job_retry_logic() {
        let mut job = Job::new(JobType::CleanupExpiredSessions);
        assert!(job.can_retry());

        job.attempts = 2;
        assert!(job.can_retry());

        job.attempts = 3;
        assert!(!job.can_retry());
    }

    #[test]
    fn test_job_result_variants() {
        let success = JobResult::Success { data: None };
        assert!(success.is_success());
        assert!(!success.is_failure());

        let failure = JobResult::Failure {
            error: "test error".to_string(),
        };
        assert!(failure.is_failure());
        assert!(!failure.is_success());

        let retry = JobResult::Retry {
            reason: "temporary error".to_string(),
            delay_seconds: 60,
        };
        assert!(retry.is_retry());
    }
}
