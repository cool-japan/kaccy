use super::queue::JobQueue;
use super::types::{Job, JobType};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{Duration as TokioDuration, interval};
use uuid::Uuid;

/// Schedule configuration
#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    /// Cron-like expression (simplified)
    /// Format: "every N seconds/minutes/hours/days"
    pub expression: String,
    /// Job type to execute
    pub job_type: JobType,
    /// Maximum number of concurrent instances
    pub max_instances: usize,
    /// Enable/disable this schedule
    pub enabled: bool,
}

/// Schedule entry
#[derive(Debug, Clone)]
struct ScheduleEntry {
    id: Uuid,
    config: ScheduleConfig,
    next_run: DateTime<Utc>,
    last_run: Option<DateTime<Utc>>,
}

impl ScheduleEntry {
    fn new(config: ScheduleConfig) -> Result<Self> {
        let next_run = Self::calculate_next_run(&config.expression, Utc::now())?;

        Ok(Self {
            id: Uuid::new_v4(),
            config,
            next_run,
            last_run: None,
        })
    }

    fn calculate_next_run(expression: &str, from: DateTime<Utc>) -> Result<DateTime<Utc>> {
        // Parse simple schedule expressions
        // Format: "every N seconds|minutes|hours|days"
        let parts: Vec<&str> = expression.split_whitespace().collect();

        if parts.len() != 3 || parts[0] != "every" {
            anyhow::bail!("Invalid schedule expression: {}", expression);
        }

        let interval: i64 = parts[1]
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid interval: {}", parts[1]))?;

        let next = match parts[2] {
            "seconds" | "second" => from + Duration::seconds(interval),
            "minutes" | "minute" => from + Duration::minutes(interval),
            "hours" | "hour" => from + Duration::hours(interval),
            "days" | "day" => from + Duration::days(interval),
            _ => anyhow::bail!("Invalid time unit: {}", parts[2]),
        };

        Ok(next)
    }

    fn update_next_run(&mut self) -> Result<()> {
        self.last_run = Some(self.next_run);
        self.next_run = Self::calculate_next_run(&self.config.expression, Utc::now())?;
        Ok(())
    }

    fn is_due(&self) -> bool {
        self.config.enabled && Utc::now() >= self.next_run
    }
}

/// Job scheduler for cron-like recurring jobs
pub struct JobScheduler {
    /// Schedule entries
    schedules: HashMap<Uuid, ScheduleEntry>,
    /// Tick interval for checking schedules
    tick_interval: TokioDuration,
    /// Shutdown signal
    shutdown: bool,
}

impl JobScheduler {
    /// Create a new job scheduler
    pub fn new() -> Self {
        Self {
            schedules: HashMap::new(),
            tick_interval: TokioDuration::from_secs(1),
            shutdown: false,
        }
    }

    /// Create a scheduler with custom tick interval
    pub fn with_tick_interval(tick_interval: TokioDuration) -> Self {
        Self {
            schedules: HashMap::new(),
            tick_interval,
            shutdown: false,
        }
    }

    /// Add a schedule
    pub fn add_schedule(&mut self, config: ScheduleConfig) -> Result<Uuid> {
        let entry = ScheduleEntry::new(config)?;
        let id = entry.id;
        self.schedules.insert(id, entry);

        tracing::info!("Added schedule {}", id);

        Ok(id)
    }

    /// Remove a schedule
    pub fn remove_schedule(&mut self, id: &Uuid) -> Result<()> {
        if self.schedules.remove(id).is_some() {
            tracing::info!("Removed schedule {}", id);
            Ok(())
        } else {
            anyhow::bail!("Schedule not found: {}", id)
        }
    }

    /// Enable a schedule
    pub fn enable_schedule(&mut self, id: &Uuid) -> Result<()> {
        if let Some(entry) = self.schedules.get_mut(id) {
            entry.config.enabled = true;
            tracing::info!("Enabled schedule {}", id);
            Ok(())
        } else {
            anyhow::bail!("Schedule not found: {}", id)
        }
    }

    /// Disable a schedule
    pub fn disable_schedule(&mut self, id: &Uuid) -> Result<()> {
        if let Some(entry) = self.schedules.get_mut(id) {
            entry.config.enabled = false;
            tracing::info!("Disabled schedule {}", id);
            Ok(())
        } else {
            anyhow::bail!("Schedule not found: {}", id)
        }
    }

    /// Get all schedules
    pub fn get_schedules(&self) -> Vec<(Uuid, ScheduleConfig, DateTime<Utc>)> {
        self.schedules
            .iter()
            .map(|(id, entry)| (*id, entry.config.clone(), entry.next_run))
            .collect()
    }

    /// Start the scheduler
    pub async fn start(&mut self, queue: Arc<JobQueue>) -> Result<()> {
        let mut ticker = interval(self.tick_interval);

        tracing::info!("Scheduler started");

        while !self.shutdown {
            ticker.tick().await;

            // Check all schedules
            for entry in self.schedules.values_mut() {
                if entry.is_due() {
                    // Enqueue job
                    let job = Job::new(entry.config.job_type.clone());

                    match queue.enqueue(job).await {
                        Ok(job_id) => {
                            tracing::info!("Scheduled job {} enqueued: {}", entry.id, job_id);

                            // Update next run time
                            if let Err(e) = entry.update_next_run() {
                                tracing::error!(
                                    "Failed to update next run for schedule {}: {}",
                                    entry.id,
                                    e
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to enqueue scheduled job {}: {}", entry.id, e);
                        }
                    }
                }
            }
        }

        tracing::info!("Scheduler stopped");

        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop(&mut self) -> Result<()> {
        self.shutdown = true;
        Ok(())
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_parsing() {
        let result = ScheduleEntry::calculate_next_run("every 5 seconds", Utc::now());
        assert!(result.is_ok());

        let result = ScheduleEntry::calculate_next_run("every 10 minutes", Utc::now());
        assert!(result.is_ok());

        let result = ScheduleEntry::calculate_next_run("every 1 hour", Utc::now());
        assert!(result.is_ok());

        let result = ScheduleEntry::calculate_next_run("every 7 days", Utc::now());
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_schedule() {
        let result = ScheduleEntry::calculate_next_run("invalid", Utc::now());
        assert!(result.is_err());

        let result = ScheduleEntry::calculate_next_run("every 5", Utc::now());
        assert!(result.is_err());

        let result = ScheduleEntry::calculate_next_run("every 5 years", Utc::now());
        assert!(result.is_err());
    }

    #[test]
    fn test_scheduler_add_remove() {
        let mut scheduler = JobScheduler::new();

        let config = ScheduleConfig {
            expression: "every 5 minutes".to_string(),
            job_type: JobType::CleanupExpiredSessions,
            max_instances: 1,
            enabled: true,
        };

        let id = scheduler.add_schedule(config).unwrap();
        assert_eq!(scheduler.schedules.len(), 1);

        scheduler.remove_schedule(&id).unwrap();
        assert_eq!(scheduler.schedules.len(), 0);
    }

    #[test]
    fn test_scheduler_enable_disable() {
        let mut scheduler = JobScheduler::new();

        let config = ScheduleConfig {
            expression: "every 5 minutes".to_string(),
            job_type: JobType::CleanupExpiredSessions,
            max_instances: 1,
            enabled: true,
        };

        let id = scheduler.add_schedule(config).unwrap();

        scheduler.disable_schedule(&id).unwrap();
        assert!(!scheduler.schedules.get(&id).unwrap().config.enabled);

        scheduler.enable_schedule(&id).unwrap();
        assert!(scheduler.schedules.get(&id).unwrap().config.enabled);
    }
}
