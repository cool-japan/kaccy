//! Background job processing system for kaccy-api
//!
//! This module provides a comprehensive job processing infrastructure including:
//! - Task queue system
//! - Job scheduling (cron-like)
//! - Job retry and failure handling
//! - Job monitoring and statistics

/// Job queue for enqueueing and dequeuing jobs
pub mod queue;
/// Job scheduler for recurring/cron-based jobs
pub mod scheduler;
/// Job and result type definitions
pub mod types;
/// Job worker for executing queued jobs
pub mod worker;

pub use queue::{JobQueue, JobQueueConfig};
pub use scheduler::{JobScheduler, ScheduleConfig};
pub use types::{Job, JobId, JobResult, JobStatus, JobType};
pub use worker::{JobWorker, WorkerConfig};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Main job processor that coordinates queue, scheduler, and workers
pub struct JobProcessor {
    queue: Arc<JobQueue>,
    scheduler: Arc<RwLock<JobScheduler>>,
    workers: Vec<JobWorker>,
}

impl JobProcessor {
    /// Create a new job processor
    pub fn new(queue: JobQueue, scheduler: JobScheduler, worker_count: usize) -> Self {
        let queue = Arc::new(queue);
        let scheduler = Arc::new(RwLock::new(scheduler));

        let workers = (0..worker_count)
            .map(|id| JobWorker::new(id, queue.clone()))
            .collect();

        Self {
            queue,
            scheduler,
            workers,
        }
    }

    /// Start the job processor
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // Start scheduler
        let scheduler = self.scheduler.clone();
        let queue = self.queue.clone();
        tokio::spawn(async move {
            let mut scheduler = scheduler.write().await;
            if let Err(e) = scheduler.start(queue).await {
                tracing::error!("Scheduler error: {}", e);
            }
        });

        // Start workers
        for worker in &mut self.workers {
            worker.start().await?;
        }

        Ok(())
    }

    /// Stop the job processor gracefully
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        // Stop scheduler
        {
            let mut scheduler = self.scheduler.write().await;
            scheduler.stop().await?;
        }

        // Stop workers
        for worker in &mut self.workers {
            worker.stop().await?;
        }

        Ok(())
    }

    /// Get job queue for enqueueing jobs
    pub fn queue(&self) -> Arc<JobQueue> {
        self.queue.clone()
    }

    /// Get scheduler for scheduling recurring jobs
    pub fn scheduler(&self) -> Arc<RwLock<JobScheduler>> {
        self.scheduler.clone()
    }
}
