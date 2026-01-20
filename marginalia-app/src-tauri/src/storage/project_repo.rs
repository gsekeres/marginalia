//! Project repository for managing paper collections

use rusqlite::{params, Connection};
use chrono::{DateTime, Utc};

use crate::models::project::{Project, PaperProject};
use super::DatabaseError;

/// Repository for projects and paper-project assignments
pub struct ProjectRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ProjectRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get all projects with paper counts
    pub fn list(&self) -> Result<Vec<Project>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.color, p.description, p.created_at, p.updated_at,
                    COUNT(pp.paper_citekey) as paper_count
             FROM projects p
             LEFT JOIN paper_projects pp ON p.id = pp.project_id
             GROUP BY p.id
             ORDER BY p.name"
        )?;

        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(4)?;
            let updated_at_str: String = row.get(5)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                description: row.get(3)?,
                created_at,
                updated_at,
                paper_count: row.get(6)?,
            })
        })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    /// Get a project by ID
    pub fn get(&self, id: &str) -> Result<Option<Project>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT p.id, p.name, p.color, p.description, p.created_at, p.updated_at,
                    COUNT(pp.paper_citekey) as paper_count
             FROM projects p
             LEFT JOIN paper_projects pp ON p.id = pp.project_id
             WHERE p.id = ?
             GROUP BY p.id",
            [id],
            |row| {
                let created_at_str: String = row.get(4)?;
                let updated_at_str: String = row.get(5)?;

                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    description: row.get(3)?,
                    created_at,
                    updated_at,
                    paper_count: row.get(6)?,
                })
            },
        );

        match result {
            Ok(project) => Ok(Some(project)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    /// Create a new project
    pub fn create(&self, project: &Project) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO projects (id, name, color, description, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                project.id,
                project.name,
                project.color,
                project.description,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Update a project
    pub fn update(&self, project: &Project) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE projects SET name = ?, color = ?, description = ?, updated_at = ?
             WHERE id = ?",
            params![
                project.name,
                project.color,
                project.description,
                Utc::now().to_rfc3339(),
                project.id,
            ],
        )?;
        Ok(())
    }

    /// Delete a project (paper assignments are cascade deleted)
    pub fn delete(&self, id: &str) -> Result<bool, DatabaseError> {
        let count = self.conn.execute("DELETE FROM projects WHERE id = ?", [id])?;
        Ok(count > 0)
    }

    /// Add a paper to a project
    pub fn add_paper(&self, project_id: &str, citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO paper_projects (paper_citekey, project_id, added_at)
             VALUES (?, ?, ?)",
            params![citekey, project_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Remove a paper from a project
    pub fn remove_paper(&self, project_id: &str, citekey: &str) -> Result<bool, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM paper_projects WHERE project_id = ? AND paper_citekey = ?",
            params![project_id, citekey],
        )?;
        Ok(count > 0)
    }

    /// Get all papers in a project (returns citekeys)
    pub fn get_papers(&self, project_id: &str) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT paper_citekey FROM paper_projects WHERE project_id = ? ORDER BY added_at DESC"
        )?;

        let rows = stmt.query_map([project_id], |row| row.get(0))?;

        let mut citekeys = Vec::new();
        for row in rows {
            citekeys.push(row?);
        }
        Ok(citekeys)
    }

    /// Get all projects a paper belongs to
    pub fn get_paper_projects(&self, citekey: &str) -> Result<Vec<Project>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.color, p.description, p.created_at, p.updated_at, 0 as paper_count
             FROM projects p
             JOIN paper_projects pp ON p.id = pp.project_id
             WHERE pp.paper_citekey = ?
             ORDER BY p.name"
        )?;

        let rows = stmt.query_map([citekey], |row| {
            let created_at_str: String = row.get(4)?;
            let updated_at_str: String = row.get(5)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                description: row.get(3)?,
                created_at,
                updated_at,
                paper_count: row.get(6)?,
            })
        })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }
        Ok(projects)
    }

    /// Count papers in a project
    pub fn count_papers(&self, project_id: &str) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM paper_projects WHERE project_id = ?",
            [project_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Set the projects for a paper (replaces existing assignments)
    pub fn set_paper_projects(&self, citekey: &str, project_ids: &[String]) -> Result<(), DatabaseError> {
        // Remove existing assignments
        self.conn.execute(
            "DELETE FROM paper_projects WHERE paper_citekey = ?",
            [citekey],
        )?;

        // Add new assignments
        let now = Utc::now().to_rfc3339();
        for project_id in project_ids {
            self.conn.execute(
                "INSERT INTO paper_projects (paper_citekey, project_id, added_at)
                 VALUES (?, ?, ?)",
                params![citekey, project_id, now],
            )?;
        }

        Ok(())
    }
}
