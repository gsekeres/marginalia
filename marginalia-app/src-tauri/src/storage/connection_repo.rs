//! Connection repository for graph edge operations

use rusqlite::{params, Connection};
use chrono::{DateTime, Utc};

use crate::models::vault::PaperConnection;
use super::DatabaseError;

/// Repository for graph connection operations
pub struct ConnectionRepo<'a> {
    conn: &'a Connection,
}

impl<'a> ConnectionRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get all connections
    pub fn get_all(&self) -> Result<Vec<PaperConnection>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT source, target, reason, created_at FROM connections"
        )?;

        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(PaperConnection {
                source: row.get(0)?,
                target: row.get(1)?,
                reason: row.get(2)?,
                created_at,
            })
        })?;

        let mut connections = Vec::new();
        for row in rows {
            connections.push(row?);
        }
        Ok(connections)
    }

    /// Get connections for a specific paper (as source or target)
    pub fn get_for_paper(&self, citekey: &str) -> Result<Vec<PaperConnection>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT source, target, reason, created_at
             FROM connections
             WHERE source = ? OR target = ?"
        )?;

        let rows = stmt.query_map(params![citekey, citekey], |row| {
            let created_at_str: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(PaperConnection {
                source: row.get(0)?,
                target: row.get(1)?,
                reason: row.get(2)?,
                created_at,
            })
        })?;

        let mut connections = Vec::new();
        for row in rows {
            connections.push(row?);
        }
        Ok(connections)
    }

    /// Add a connection between two papers
    pub fn add(&self, source: &str, target: &str, reason: &str) -> Result<i64, DatabaseError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO connections (source, target, reason, created_at)
             VALUES (?, ?, ?, ?)",
            params![source, target, reason, Utc::now().to_rfc3339()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Remove a connection between two papers
    pub fn remove(&self, source: &str, target: &str) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM connections WHERE source = ? AND target = ?",
            params![source, target],
        )?;
        Ok(count)
    }

    /// Check if a connection exists
    pub fn exists(&self, source: &str, target: &str) -> Result<bool, DatabaseError> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM connections WHERE source = ? AND target = ?",
            params![source, target],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get neighbors of a paper (directly connected papers)
    pub fn get_neighbors(&self, citekey: &str) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT
                CASE WHEN source = ? THEN target ELSE source END as neighbor
             FROM connections
             WHERE source = ? OR target = ?"
        )?;

        let rows = stmt.query_map(params![citekey, citekey, citekey], |row| {
            row.get::<_, String>(0)
        })?;

        let mut neighbors = Vec::new();
        for row in rows {
            neighbors.push(row?);
        }
        Ok(neighbors)
    }

    /// Count total connections
    pub fn count(&self) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM connections",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Update the reason for a connection
    pub fn update_reason(&self, source: &str, target: &str, reason: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE connections SET reason = ? WHERE source = ? AND target = ?",
            params![reason, source, target],
        )?;
        Ok(())
    }

    /// Remove all connections for a paper (when paper is deleted)
    pub fn remove_all_for_paper(&self, citekey: &str) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM connections WHERE source = ? OR target = ?",
            params![citekey, citekey],
        )?;
        Ok(count)
    }
}
