//! Job Manager service for background task execution
//!
//! Provides async job execution with progress tracking and Tauri event emission.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tracing::{info, error};

use crate::storage::{open_database, JobRepo, PaperRepo};
use crate::storage::job_repo::{Job, JobType, JobStatus, JobUpdate};

/// Job manager handles background task execution
pub struct JobManager {
    vault_path: PathBuf,
    app_handle: Option<AppHandle>,
}

impl JobManager {
    pub fn new(vault_path: PathBuf) -> Self {
        Self {
            vault_path,
            app_handle: None,
        }
    }

    pub fn with_app_handle(mut self, app: AppHandle) -> Self {
        self.app_handle = Some(app);
        self
    }

    /// Start a new job
    pub fn start_job(&self, job_type: JobType, citekey: Option<String>) -> Result<String, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job = Job::new(job_type, citekey);
        let job_id = job.id.clone();

        let job_repo = JobRepo::new(&db.conn);
        job_repo.create(&job)
            .map_err(|e| format!("Failed to create job: {}", e))?;

        info!("Created job {} of type {:?}", job_id, job.job_type);

        Ok(job_id)
    }

    /// Get a job by ID
    pub fn get_job(&self, id: &str) -> Result<Option<Job>, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.get(id)
            .map_err(|e| format!("Failed to get job: {}", e))
    }

    /// List jobs with optional status filter
    pub fn list_jobs(&self, status: Option<JobStatus>, limit: i64) -> Result<Vec<Job>, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.list(status.as_ref(), limit)
            .map_err(|e| format!("Failed to list jobs: {}", e))
    }

    /// List active jobs (pending or running)
    pub fn list_active_jobs(&self) -> Result<Vec<Job>, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.list_active()
            .map_err(|e| format!("Failed to list active jobs: {}", e))
    }

    /// Cancel a job
    pub fn cancel_job(&self, id: &str) -> Result<bool, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        let cancelled = job_repo.cancel(id)
            .map_err(|e| format!("Failed to cancel job: {}", e))?;

        if cancelled {
            self.emit_job_update(id, "cancelled", 0, None);
            info!("Cancelled job {}", id);
        }

        Ok(cancelled)
    }

    /// Update job progress and emit event
    pub fn update_progress(&self, id: &str, progress: i32) -> Result<(), String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.update_progress(id, progress)
            .map_err(|e| format!("Failed to update progress: {}", e))?;

        self.emit_job_update(id, "running", progress, None);

        Ok(())
    }

    /// Mark job as running
    pub fn mark_running(&self, id: &str) -> Result<(), String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.update_status(id, &JobStatus::Running)
            .map_err(|e| format!("Failed to update status: {}", e))?;

        self.emit_job_update(id, "running", 0, None);

        Ok(())
    }

    /// Mark job as completed
    pub fn mark_completed(&self, id: &str) -> Result<(), String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.complete(id)
            .map_err(|e| format!("Failed to complete job: {}", e))?;

        self.emit_job_update(id, "completed", 100, None);
        info!("Completed job {}", id);

        Ok(())
    }

    /// Mark job as failed
    pub fn mark_failed(&self, id: &str, error: &str) -> Result<(), String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.fail(id, error)
            .map_err(|e| format!("Failed to mark job failed: {}", e))?;

        self.emit_job_update(id, "failed", 0, Some(error.to_string()));
        error!("Job {} failed: {}", id, error);

        Ok(())
    }

    /// Emit a job update event to the frontend
    fn emit_job_update(&self, id: &str, status: &str, progress: i32, error: Option<String>) {
        if let Some(app) = &self.app_handle {
            let update = JobUpdate {
                id: id.to_string(),
                status: status.to_string(),
                progress,
                error,
            };

            if let Err(e) = app.emit("job:updated", &update) {
                error!("Failed to emit job update: {}", e);
            }
        }
    }

    /// Clean up old completed/failed jobs
    pub fn cleanup_old_jobs(&self, days_old: i64) -> Result<usize, String> {
        let db = open_database(&self.vault_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let job_repo = JobRepo::new(&db.conn);
        job_repo.cleanup(days_old)
            .map_err(|e| format!("Failed to cleanup jobs: {}", e))
    }
}

/// Spawn a job as a tokio task
pub async fn spawn_job<F, Fut>(
    job_manager: Arc<Mutex<JobManager>>,
    job_id: String,
    task: F,
) where
    F: FnOnce(Arc<Mutex<JobManager>>, String) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), String>> + Send,
{
    // Mark job as running
    {
        let manager = job_manager.lock().await;
        if let Err(e) = manager.mark_running(&job_id) {
            error!("Failed to mark job running: {}", e);
            return;
        }
    }

    // Execute the task
    let result = task(job_manager.clone(), job_id.clone()).await;

    // Update job status based on result
    let manager = job_manager.lock().await;
    match result {
        Ok(()) => {
            if let Err(e) = manager.mark_completed(&job_id) {
                error!("Failed to mark job completed: {}", e);
            }
        }
        Err(e) => {
            if let Err(err) = manager.mark_failed(&job_id, &e) {
                error!("Failed to mark job failed: {}", err);
            }
        }
    }
}
