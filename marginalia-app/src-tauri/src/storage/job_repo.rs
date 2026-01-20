//! Job repository for background job queue management

use rusqlite::{params, Connection};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::DatabaseError;

/// Job type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    ImportBib,
    FindPdf,
    DownloadPdf,
    ExtractText,
    Summarize,
    BuildGraph,
}

impl JobType {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobType::ImportBib => "import_bib",
            JobType::FindPdf => "find_pdf",
            JobType::DownloadPdf => "download_pdf",
            JobType::ExtractText => "extract_text",
            JobType::Summarize => "summarize",
            JobType::BuildGraph => "build_graph",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "import_bib" => Some(JobType::ImportBib),
            "find_pdf" => Some(JobType::FindPdf),
            "download_pdf" => Some(JobType::DownloadPdf),
            "extract_text" => Some(JobType::ExtractText),
            "summarize" => Some(JobType::Summarize),
            "build_graph" => Some(JobType::BuildGraph),
            _ => None,
        }
    }
}

/// Job status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Pending => "pending",
            JobStatus::Running => "running",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => JobStatus::Pending,
            "running" => JobStatus::Running,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Pending,
        }
    }
}

/// Job record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub job_type: JobType,
    pub citekey: Option<String>,
    pub status: JobStatus,
    pub progress: i32,
    pub error: Option<String>,
    pub log_path: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Job {
    /// Create a new pending job
    pub fn new(job_type: JobType, citekey: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            job_type,
            citekey,
            status: JobStatus::Pending,
            progress: 0,
            error: None,
            log_path: None,
            started_at: None,
            finished_at: None,
            created_at: Utc::now(),
        }
    }
}

/// Job update event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobUpdate {
    pub id: String,
    pub status: String,
    pub progress: i32,
    pub error: Option<String>,
}

/// Repository for job queue operations
pub struct JobRepo<'a> {
    conn: &'a Connection,
}

impl<'a> JobRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new job
    pub fn create(&self, job: &Job) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO jobs (id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                job.id,
                job.job_type.as_str(),
                job.citekey,
                job.status.as_str(),
                job.progress,
                job.error,
                job.log_path,
                job.started_at.map(|d| d.to_rfc3339()),
                job.finished_at.map(|d| d.to_rfc3339()),
                job.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Get a job by ID
    pub fn get(&self, id: &str) -> Result<Option<Job>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at
             FROM jobs WHERE id = ?",
            [id],
            |row| self.row_to_job(row),
        );

        match result {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    /// List jobs with optional status filter
    pub fn list(&self, status: Option<&JobStatus>, limit: i64) -> Result<Vec<Job>, DatabaseError> {
        let mut jobs = Vec::new();

        if let Some(s) = status {
            let mut stmt = self.conn.prepare(
                "SELECT id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at
                 FROM jobs WHERE status = ? ORDER BY created_at DESC LIMIT ?"
            )?;
            let rows = stmt.query_map(params![s.as_str(), limit], |row| self.row_to_job(row))?;
            for row in rows {
                jobs.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at
                 FROM jobs ORDER BY created_at DESC LIMIT ?"
            )?;
            let rows = stmt.query_map(params![limit], |row| self.row_to_job(row))?;
            for row in rows {
                jobs.push(row?);
            }
        }

        Ok(jobs)
    }

    /// List active jobs (pending or running)
    pub fn list_active(&self) -> Result<Vec<Job>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at
             FROM jobs WHERE status IN ('pending', 'running') ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map([], |row| self.row_to_job(row))?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    /// Update job status
    pub fn update_status(&self, id: &str, status: &JobStatus) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();

        let (started_update, finished_update) = match status {
            JobStatus::Running => (Some(&now), None),
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled => (None, Some(&now)),
            _ => (None, None),
        };

        if let Some(started) = started_update {
            self.conn.execute(
                "UPDATE jobs SET status = ?, started_at = COALESCE(started_at, ?) WHERE id = ?",
                params![status.as_str(), started, id],
            )?;
        } else if let Some(finished) = finished_update {
            self.conn.execute(
                "UPDATE jobs SET status = ?, finished_at = ? WHERE id = ?",
                params![status.as_str(), finished, id],
            )?;
        } else {
            self.conn.execute(
                "UPDATE jobs SET status = ? WHERE id = ?",
                params![status.as_str(), id],
            )?;
        }

        Ok(())
    }

    /// Update job progress
    pub fn update_progress(&self, id: &str, progress: i32) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE jobs SET progress = ? WHERE id = ?",
            params![progress, id],
        )?;
        Ok(())
    }

    /// Mark job as failed with error
    pub fn fail(&self, id: &str, error: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE jobs SET status = 'failed', error = ?, finished_at = ? WHERE id = ?",
            params![error, Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Mark job as completed
    pub fn complete(&self, id: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE jobs SET status = 'completed', progress = 100, finished_at = ? WHERE id = ?",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(())
    }

    /// Cancel a job
    pub fn cancel(&self, id: &str) -> Result<bool, DatabaseError> {
        let count = self.conn.execute(
            "UPDATE jobs SET status = 'cancelled', finished_at = ? WHERE id = ? AND status IN ('pending', 'running')",
            params![Utc::now().to_rfc3339(), id],
        )?;
        Ok(count > 0)
    }

    /// Delete old completed/failed jobs (cleanup)
    pub fn cleanup(&self, days_old: i64) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM jobs WHERE status IN ('completed', 'failed', 'cancelled')
             AND created_at < datetime('now', ? || ' days')",
            params![format!("-{}", days_old)],
        )?;
        Ok(count)
    }

    /// Check if there's an active job for a citekey
    pub fn has_active_job(&self, citekey: &str) -> Result<bool, DatabaseError> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM jobs WHERE citekey = ? AND status IN ('pending', 'running')",
            [citekey],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get the next pending job
    pub fn get_next_pending(&self) -> Result<Option<Job>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT id, job_type, citekey, status, progress, error, log_path, started_at, finished_at, created_at
             FROM jobs WHERE status = 'pending' ORDER BY created_at ASC LIMIT 1",
            [],
            |row| self.row_to_job(row),
        );

        match result {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    // Helper methods

    fn row_to_job(&self, row: &rusqlite::Row) -> rusqlite::Result<Job> {
        let job_type_str: String = row.get(1)?;
        let job_type = JobType::from_str(&job_type_str).unwrap_or(JobType::Summarize);

        let status_str: String = row.get(3)?;
        let status = JobStatus::from_str(&status_str);

        let created_at_str: String = row.get(9)?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let started_at: Option<DateTime<Utc>> = row.get::<_, Option<String>>(7)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc));

        let finished_at: Option<DateTime<Utc>> = row.get::<_, Option<String>>(8)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc));

        Ok(Job {
            id: row.get(0)?,
            job_type,
            citekey: row.get(2)?,
            status,
            progress: row.get(4)?,
            error: row.get(5)?,
            log_path: row.get(6)?,
            started_at,
            finished_at,
            created_at,
        })
    }
}
