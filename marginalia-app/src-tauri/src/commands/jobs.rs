//! Job management commands

use tauri::State;
use tracing::info;

use crate::storage::JobRepo;
use crate::storage::job_repo::{Job, JobType, JobStatus};
use crate::AppState;

#[derive(serde::Serialize)]
pub struct JobInfo {
    pub id: String,
    pub job_type: String,
    pub citekey: Option<String>,
    pub status: String,
    pub progress: i32,
    pub error: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
}

impl From<Job> for JobInfo {
    fn from(job: Job) -> Self {
        Self {
            id: job.id,
            job_type: job.job_type.as_str().to_string(),
            citekey: job.citekey,
            status: job.status.as_str().to_string(),
            progress: job.progress,
            error: job.error,
            started_at: job.started_at.map(|d| d.to_rfc3339()),
            finished_at: job.finished_at.map(|d| d.to_rfc3339()),
            created_at: job.created_at.to_rfc3339(),
        }
    }
}

/// Start a new job
#[tauri::command]
pub async fn start_job(
    job_type: String,
    citekey: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_type_enum = JobType::from_str(&job_type)
        .ok_or_else(|| format!("Invalid job type: {}", job_type))?;

    let job = Job::new(job_type_enum, citekey.clone());
    let job_id = job.id.clone();

    let job_repo = JobRepo::new(&db.conn);
    job_repo.create(&job)
        .map_err(|e| format!("Failed to create job: {}", e))?;

    info!("Created job {} of type {} for {:?}", job_id, job_type, citekey);

    Ok(job_id)
}

/// Get a job by ID
#[tauri::command]
pub async fn get_job(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<JobInfo>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_repo = JobRepo::new(&db.conn);
    let job = job_repo.get(&id)
        .map_err(|e| format!("Failed to get job: {}", e))?;

    Ok(job.map(JobInfo::from))
}

/// List jobs with optional status filter
#[tauri::command]
pub async fn list_jobs(
    status: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<JobInfo>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_repo = JobRepo::new(&db.conn);

    let status_enum = status.as_ref().map(|s| JobStatus::from_str(s));
    let limit = limit.unwrap_or(50);

    let jobs = job_repo.list(status_enum.as_ref(), limit)
        .map_err(|e| format!("Failed to list jobs: {}", e))?;

    Ok(jobs.into_iter().map(JobInfo::from).collect())
}

/// List active jobs (pending or running)
#[tauri::command]
pub async fn list_active_jobs(
    state: State<'_, AppState>,
) -> Result<Vec<JobInfo>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_repo = JobRepo::new(&db.conn);
    let jobs = job_repo.list_active()
        .map_err(|e| format!("Failed to list active jobs: {}", e))?;

    Ok(jobs.into_iter().map(JobInfo::from).collect())
}

/// Cancel a job
#[tauri::command]
pub async fn cancel_job(
    id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_repo = JobRepo::new(&db.conn);
    let cancelled = job_repo.cancel(&id)
        .map_err(|e| format!("Failed to cancel job: {}", e))?;

    if cancelled {
        info!("Cancelled job {}", id);
    }

    Ok(cancelled)
}

/// Update job progress
#[tauri::command]
pub async fn update_job_progress(
    id: String,
    progress: i32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let job_repo = JobRepo::new(&db.conn);
    job_repo.update_progress(&id, progress)
        .map_err(|e| format!("Failed to update progress: {}", e))?;

    Ok(())
}
