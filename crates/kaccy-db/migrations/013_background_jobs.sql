-- Background Jobs Queue System
-- Supports asynchronous task processing with priorities, retries, and scheduling

CREATE TABLE background_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_type VARCHAR(100) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending', -- pending, running, completed, failed, cancelled
    priority INTEGER NOT NULL DEFAULT 0, -- higher number = higher priority
    payload JSONB NOT NULL, -- job-specific data
    result JSONB, -- job result on completion
    error_message TEXT, -- error details on failure
    max_retries INTEGER NOT NULL DEFAULT 3,
    retry_count INTEGER NOT NULL DEFAULT 0,
    retry_backoff_ms INTEGER NOT NULL DEFAULT 1000, -- exponential backoff base

    -- Scheduling
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), -- when to run
    started_at TIMESTAMPTZ, -- when execution started
    completed_at TIMESTAMPTZ, -- when completed/failed
    expires_at TIMESTAMPTZ, -- job expiration

    -- Metadata
    created_by UUID, -- user who created the job
    correlation_id VARCHAR(255), -- for distributed tracing
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for efficient job queue polling (fetch pending/failed jobs ordered by priority and schedule)
CREATE INDEX idx_background_jobs_status_priority_scheduled
    ON background_jobs(status, priority DESC, scheduled_at ASC)
    WHERE status IN ('pending', 'failed');

-- Index for filtering by job type and status
CREATE INDEX idx_background_jobs_job_type_status
    ON background_jobs(job_type, status);

-- Index for distributed tracing support
CREATE INDEX idx_background_jobs_correlation_id
    ON background_jobs(correlation_id)
    WHERE correlation_id IS NOT NULL;

-- Index for user job tracking
CREATE INDEX idx_background_jobs_created_by
    ON background_jobs(created_by)
    WHERE created_by IS NOT NULL;

-- Index for cleanup of expired jobs
CREATE INDEX idx_background_jobs_expires_at
    ON background_jobs(expires_at)
    WHERE expires_at IS NOT NULL AND status != 'completed';

-- Index for monitoring running jobs (detect stuck jobs)
CREATE INDEX idx_background_jobs_running_started
    ON background_jobs(status, started_at)
    WHERE status = 'running';

COMMENT ON TABLE background_jobs IS 'Background job queue for asynchronous task processing';
COMMENT ON COLUMN background_jobs.job_type IS 'Type of job (e.g., send_email, process_blockchain_confirmation)';
COMMENT ON COLUMN background_jobs.priority IS 'Job priority (higher number = higher priority)';
COMMENT ON COLUMN background_jobs.payload IS 'Job-specific input data in JSON format';
COMMENT ON COLUMN background_jobs.result IS 'Job result data on successful completion';
COMMENT ON COLUMN background_jobs.max_retries IS 'Maximum number of retry attempts';
COMMENT ON COLUMN background_jobs.retry_backoff_ms IS 'Base delay in milliseconds for exponential backoff';
COMMENT ON COLUMN background_jobs.scheduled_at IS 'When the job should be executed (supports delayed jobs)';
COMMENT ON COLUMN background_jobs.expires_at IS 'Job expiration timestamp (cleanup old jobs)';
COMMENT ON COLUMN background_jobs.correlation_id IS 'Correlation ID for distributed tracing';
