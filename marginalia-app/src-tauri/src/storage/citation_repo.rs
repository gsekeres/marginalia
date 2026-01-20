//! Citation repository for database operations on citations and related papers

use rusqlite::{params, Connection};

use crate::models::paper::{Citation, RelatedPaper};
use super::DatabaseError;

/// Repository for Citation and RelatedPaper operations
pub struct CitationRepo<'a> {
    conn: &'a Connection,
}

impl<'a> CitationRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get all citations for a paper
    pub fn get_citations(&self, source_citekey: &str) -> Result<Vec<Citation>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT citekey, title, authors, year, doi, status
             FROM citations WHERE source_citekey = ?"
        )?;

        let rows = stmt.query_map([source_citekey], |row| {
            Ok(Citation {
                citekey: row.get(0)?,
                title: row.get(1)?,
                authors: row.get(2)?,
                year: row.get(3)?,
                doi: row.get(4)?,
                status: row.get(5)?,
            })
        })?;

        let mut citations = Vec::new();
        for row in rows {
            citations.push(row?);
        }
        Ok(citations)
    }

    /// Add a citation
    pub fn add_citation(&self, source_citekey: &str, citation: &Citation) -> Result<i64, DatabaseError> {
        self.conn.execute(
            "INSERT INTO citations (source_citekey, citekey, title, authors, year, doi, status)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                source_citekey,
                citation.citekey,
                citation.title,
                citation.authors,
                citation.year,
                citation.doi,
                citation.status,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Delete all citations for a paper
    pub fn delete_citations(&self, source_citekey: &str) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM citations WHERE source_citekey = ?",
            [source_citekey],
        )?;
        Ok(count)
    }

    /// Update citation status (e.g., "in_vault", "unknown")
    pub fn update_citation_status(&self, source_citekey: &str, citekey: &str, status: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE citations SET status = ? WHERE source_citekey = ? AND citekey = ?",
            params![status, source_citekey, citekey],
        )?;
        Ok(())
    }

    /// Get all related papers for a paper
    pub fn get_related_papers(&self, source_citekey: &str) -> Result<Vec<RelatedPaper>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT title, authors_json, year, why_related, vault_citekey
             FROM related_papers WHERE source_citekey = ?"
        )?;

        let rows = stmt.query_map([source_citekey], |row| {
            let authors_json: String = row.get(1)?;
            let authors: Vec<String> = serde_json::from_str(&authors_json).unwrap_or_default();

            Ok(RelatedPaper {
                title: row.get(0)?,
                authors,
                year: row.get(2)?,
                why_related: row.get(3)?,
                vault_citekey: row.get(4)?,
            })
        })?;

        let mut related = Vec::new();
        for row in rows {
            related.push(row?);
        }
        Ok(related)
    }

    /// Add a related paper
    pub fn add_related_paper(&self, source_citekey: &str, related: &RelatedPaper) -> Result<i64, DatabaseError> {
        let authors_json = serde_json::to_string(&related.authors)?;
        self.conn.execute(
            "INSERT INTO related_papers (source_citekey, title, authors_json, year, why_related, vault_citekey)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                source_citekey,
                related.title,
                authors_json,
                related.year,
                related.why_related,
                related.vault_citekey,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Delete all related papers for a paper
    pub fn delete_related_papers(&self, source_citekey: &str) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM related_papers WHERE source_citekey = ?",
            [source_citekey],
        )?;
        Ok(count)
    }

    /// Update related paper vault_citekey (when a related paper is added to vault)
    pub fn update_related_vault_citekey(&self, source_citekey: &str, title: &str, vault_citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE related_papers SET vault_citekey = ? WHERE source_citekey = ? AND title = ?",
            params![vault_citekey, source_citekey, title],
        )?;
        Ok(())
    }

    /// Find papers that cite a given citekey
    pub fn get_cited_by(&self, citekey: &str) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT source_citekey FROM citations WHERE citekey = ?"
        )?;

        let rows = stmt.query_map([citekey], |row| row.get::<_, String>(0))?;

        let mut citing_papers = Vec::new();
        for row in rows {
            citing_papers.push(row?);
        }
        Ok(citing_papers)
    }

    /// Check if a citation exists
    pub fn citation_exists(&self, source_citekey: &str, citekey: &str) -> Result<bool, DatabaseError> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM citations WHERE source_citekey = ? AND citekey = ?",
            params![source_citekey, citekey],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if a related paper already exists (by title similarity)
    pub fn related_paper_exists(&self, source_citekey: &str, title: &str) -> Result<bool, DatabaseError> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM related_papers WHERE source_citekey = ? AND LOWER(title) = LOWER(?)",
            params![source_citekey, title],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
