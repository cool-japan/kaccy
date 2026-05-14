use super::types::{Job, JobId, JobStatus};
use anyhow::Result;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Notify;

/// Configuration for job queue
#[derive(Debug, Clone)]
pub struct JobQueueConfig {
    /// Maximum number of jobs in queue
    pub max_queue_size: usize,
    /// Enable persistence to Redis
    pub enable_persistence: bool,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10_000,
            enable_persistence: true,
        }
    }
}

/// Job queue implementation
pub struct JobQueue {
    /// In-memory job storage
    jobs: Arc<DashMap<JobId, Job>>,
    /// Pending job queue (sorted by priority)
    pending: Arc<DashMap<i32, Vec<JobId>>>,
    /// Notify workers when new jobs are added
    notify: Arc<Notify>,
    /// Configuration
    config: JobQueueConfig,
}

impl JobQueue {
    /// Create a new job queue
    pub fn new(config: JobQueueConfig) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            pending: Arc::new(DashMap::new()),
            notify: Arc::new(Notify::new()),
            config,
        }
    }

    /// Enqueue a new job
    pub async fn enqueue(&self, mut job: Job) -> Result<JobId> {
        // Check queue size limit
        if self.jobs.len() >= self.config.max_queue_size {
            anyhow::bail!("Job queue is full");
        }

        let job_id = job.id;
        job.status = JobStatus::Pending;
        job.updated_at = Utc::now();

        // Store job
        self.jobs.insert(job_id, job.clone());

        // Add to pending queue by priority
        let priority = job.priority;
        self.pending.entry(priority).or_default().push(job_id);

        // Notify workers
        self.notify.notify_one();

        tracing::debug!("Enqueued job {} with priority {}", job_id, priority);

        Ok(job_id)
    }

    /// Dequeue the next pending job (highest priority first)
    pub async fn dequeue(&self) -> Option<Job> {
        // Find highest priority with pending jobs
        let mut priorities: Vec<i32> = self
            .pending
            .iter()
            .filter(|entry| !entry.value().is_empty())
            .map(|entry| *entry.key())
            .collect();

        priorities.sort_by(|a, b| b.cmp(a)); // Descending order

        for priority in priorities {
            if let Some(mut entry) = self.pending.get_mut(&priority) {
                if let Some(job_id) = entry.pop() {
                    // Get and update job
                    if let Some(mut job_entry) = self.jobs.get_mut(&job_id) {
                        let job = job_entry.value_mut();
                        job.status = JobStatus::Processing;
                        job.started_at = Some(Utc::now());
                        job.updated_at = Utc::now();
                        job.attempts += 1;

                        tracing::debug!("Dequeued job {} (attempt {})", job_id, job.attempts);

                        return Some(job.clone());
                    }
                }
            }
        }

        None
    }

    /// Wait for a job to become available
    pub async fn wait_for_job(&self) {
        self.notify.notified().await;
    }

    /// Get a job by ID
    pub async fn get(&self, job_id: &JobId) -> Option<Job> {
        self.jobs.get(job_id).map(|entry| entry.value().clone())
    }

    /// Update job status
    pub async fn update_status(&self, job_id: &JobId, status: JobStatus) -> Result<()> {
        if let Some(mut entry) = self.jobs.get_mut(job_id) {
            let job = entry.value_mut();
            job.status = status;
            job.updated_at = Utc::now();

            if status == JobStatus::Completed || status == JobStatus::Failed {
                job.completed_at = Some(Utc::now());
            }

            Ok(())
        } else {
            anyhow::bail!("Job not found: {}", job_id)
        }
    }

    /// Mark job as completed
    pub async fn complete(&self, job_id: &JobId, result: Option<String>) -> Result<()> {
        if let Some(mut entry) = self.jobs.get_mut(job_id) {
            let job = entry.value_mut();
            job.status = JobStatus::Completed;
            job.completed_at = Some(Utc::now());
            job.updated_at = Utc::now();
            job.result = result;

            tracing::info!("Job {} completed", job_id);

            Ok(())
        } else {
            anyhow::bail!("Job not found: {}", job_id)
        }
    }

    /// Mark job as failed
    pub async fn fail(&self, job_id: &JobId, error: String) -> Result<()> {
        if let Some(mut entry) = self.jobs.get_mut(job_id) {
            let job = entry.value_mut();

            if job.can_retry() {
                // Re-enqueue for retry
                job.status = JobStatus::Pending;
                job.error = Some(error.clone());
                job.updated_at = Utc::now();

                let priority = job.priority;
                let job_id = job.id;

                // Add back to pending queue
                self.pending.entry(priority).or_default().push(job_id);

                self.notify.notify_one();

                tracing::warn!(
                    "Job {} failed, will retry (attempt {})",
                    job_id,
                    job.attempts
                );
            } else {
                // Max retries reached
                job.status = JobStatus::Failed;
                job.completed_at = Some(Utc::now());
                job.error = Some(error);
                job.updated_at = Utc::now();

                tracing::error!(
                    "Job {} failed permanently after {} attempts",
                    job_id,
                    job.attempts
                );
            }

            Ok(())
        } else {
            anyhow::bail!("Job not found: {}", job_id)
        }
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        let total = self.jobs.len();
        let mut pending = 0;
        let mut processing = 0;
        let mut completed = 0;
        let mut failed = 0;

        for entry in self.jobs.iter() {
            match entry.value().status {
                JobStatus::Pending | JobStatus::Scheduled => pending += 1,
                JobStatus::Processing => processing += 1,
                JobStatus::Completed => completed += 1,
                JobStatus::Failed => failed += 1,
                JobStatus::Cancelled => {}
            }
        }

        QueueStats {
            total,
            pending,
            processing,
            completed,
            failed,
        }
    }

    /// Clear completed jobs (cleanup)
    pub async fn cleanup_completed(&self, keep_recent: usize) -> Result<usize> {
        let mut completed_jobs: Vec<(JobId, chrono::DateTime<Utc>)> = self
            .jobs
            .iter()
            .filter(|entry| entry.value().status == JobStatus::Completed)
            .map(|entry| {
                (
                    *entry.key(),
                    entry.value().completed_at.unwrap_or(Utc::now()),
                )
            })
            .collect();

        // Sort by completion time (oldest first)
        completed_jobs.sort_by_key(|a| a.1);

        // Remove old completed jobs
        let to_remove = completed_jobs.len().saturating_sub(keep_recent);
        let mut removed = 0;

        for (job_id, _) in completed_jobs.iter().take(to_remove) {
            self.jobs.remove(job_id);
            removed += 1;
        }

        if removed > 0 {
            tracing::info!("Cleaned up {} completed jobs", removed);
        }

        Ok(removed)
    }
}

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Total jobs in the queue (all states)
    pub total: usize,
    /// Jobs waiting to be processed
    pub pending: usize,
    /// Jobs currently being processed
    pub processing: usize,
    /// Jobs that completed successfully
    pub completed: usize,
    /// Jobs that failed
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::types::JobType;

    #[tokio::test]
    async fn test_enqueue_dequeue() {
        let queue = JobQueue::new(JobQueueConfig::default());
        let job = Job::new(JobType::CleanupExpiredSessions);
        let job_id = job.id;

        queue.enqueue(job).await.unwrap();

        let dequeued = queue.dequeue().await.unwrap();
        assert_eq!(dequeued.id, job_id);
        assert_eq!(dequeued.status, JobStatus::Processing);
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = JobQueue::new(JobQueueConfig::default());

        let low_priority = Job::new(JobType::CleanupExpiredSessions).with_priority(1);
        let high_priority = Job::new(JobType::CleanupExpiredSessions).with_priority(10);

        queue.enqueue(low_priority.clone()).await.unwrap();
        queue.enqueue(high_priority.clone()).await.unwrap();

        // High priority should be dequeued first
        let first = queue.dequeue().await.unwrap();
        assert_eq!(first.id, high_priority.id);

        let second = queue.dequeue().await.unwrap();
        assert_eq!(second.id, low_priority.id);
    }

    #[tokio::test]
    async fn test_job_retry() {
        let queue = JobQueue::new(JobQueueConfig::default());
        let job = Job::new(JobType::CleanupExpiredSessions).with_max_attempts(3);
        let job_id = job.id;

        queue.enqueue(job).await.unwrap();

        let _job = queue.dequeue().await.unwrap();
        queue.fail(&job_id, "Test error".to_string()).await.unwrap();

        // Job should be re-queued
        let stats = queue.stats().await;
        assert_eq!(stats.pending, 1);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = JobQueue::new(JobQueueConfig::default());

        queue
            .enqueue(Job::new(JobType::CleanupExpiredSessions))
            .await
            .unwrap();
        queue
            .enqueue(Job::new(JobType::CleanupExpiredSessions))
            .await
            .unwrap();

        let stats = queue.stats().await;
        assert_eq!(stats.total, 2);
        assert_eq!(stats.pending, 2);
    }
}
