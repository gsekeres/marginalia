//! Paper repository for database operations on papers

use rusqlite::{params, Connection, Row};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

use crate::models::paper::{Paper, PaperStatus, Citation, RelatedPaper};
use crate::models::vault::VaultStats;
use super::DatabaseError;

/// Repository for Paper operations
pub struct PaperRepo<'a> {
    conn: &'a Connection,
}

impl<'a> PaperRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get a paper by citekey
    pub fn get(&self, citekey: &str) -> Result<Option<Paper>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM papers WHERE citekey = ?"
        )?;

        let result = stmt.query_row([citekey], |row| {
            self.row_to_paper(row)
        });

        match result {
            Ok(paper) => {
                // Load citations and related papers
                let mut paper = paper;
                paper.citations = self.get_citations(citekey)?;
                paper.related_papers = self.get_related_papers(citekey)?;
                paper.cited_by = self.get_cited_by(citekey)?;
                Ok(Some(paper))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    /// List papers with optional filtering
    pub fn list(
        &self,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Paper>, DatabaseError> {
        let mut papers = Vec::new();

        if let Some(s) = status {
            let mut stmt = self.conn.prepare(
                "SELECT * FROM papers WHERE status = ? ORDER BY added_at DESC LIMIT ? OFFSET ?"
            )?;
            let rows = stmt.query_map(params![s, limit, offset], |row| self.row_to_paper(row))?;
            for row in rows {
                let mut paper = row?;
                paper.citations = self.get_citations(&paper.citekey).unwrap_or_default();
                paper.related_papers = self.get_related_papers(&paper.citekey).unwrap_or_default();
                paper.cited_by = self.get_cited_by(&paper.citekey).unwrap_or_default();
                papers.push(paper);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT * FROM papers ORDER BY added_at DESC LIMIT ? OFFSET ?"
            )?;
            let rows = stmt.query_map(params![limit, offset], |row| self.row_to_paper(row))?;
            for row in rows {
                let mut paper = row?;
                paper.citations = self.get_citations(&paper.citekey).unwrap_or_default();
                paper.related_papers = self.get_related_papers(&paper.citekey).unwrap_or_default();
                paper.cited_by = self.get_cited_by(&paper.citekey).unwrap_or_default();
                papers.push(paper);
            }
        }

        Ok(papers)
    }

    /// Get all papers as a HashMap (for compatibility with existing code)
    pub fn get_all(&self) -> Result<HashMap<String, Paper>, DatabaseError> {
        let mut stmt = self.conn.prepare("SELECT * FROM papers")?;
        let rows = stmt.query_map([], |row| self.row_to_paper(row))?;

        let mut papers = HashMap::new();
        for row in rows {
            let mut paper = row?;
            paper.citations = self.get_citations(&paper.citekey).unwrap_or_default();
            paper.related_papers = self.get_related_papers(&paper.citekey).unwrap_or_default();
            paper.cited_by = self.get_cited_by(&paper.citekey).unwrap_or_default();
            papers.insert(paper.citekey.clone(), paper);
        }

        Ok(papers)
    }

    /// Insert a new paper
    pub fn insert(&self, paper: &Paper) -> Result<(), DatabaseError> {
        let status_str = status_to_string(&paper.status);
        let authors_json = serde_json::to_string(&paper.authors)?;
        let manual_links_json = serde_json::to_string(&paper.manual_download_links)?;

        self.conn.execute(
            "INSERT INTO papers (
                citekey, title, authors_json, year, journal, volume, number, pages,
                doi, url, abstract, status, pdf_path, summary_path, notes_path,
                added_at, downloaded_at, summarized_at, search_attempts,
                last_search_error, manual_download_links_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                paper.citekey,
                paper.title,
                authors_json,
                paper.year,
                paper.journal,
                paper.volume,
                paper.number,
                paper.pages,
                paper.doi,
                paper.url,
                paper.r#abstract,
                status_str,
                paper.pdf_path,
                paper.summary_path,
                paper.notes_path,
                paper.added_at.to_rfc3339(),
                paper.downloaded_at.map(|d| d.to_rfc3339()),
                paper.summarized_at.map(|d| d.to_rfc3339()),
                paper.search_attempts,
                paper.last_search_error,
                manual_links_json,
            ],
        )?;

        // Insert citations
        for citation in &paper.citations {
            self.insert_citation(&paper.citekey, citation)?;
        }

        // Insert related papers
        for related in &paper.related_papers {
            self.insert_related_paper(&paper.citekey, related)?;
        }

        Ok(())
    }

    /// Update an existing paper
    pub fn update(&self, paper: &Paper) -> Result<(), DatabaseError> {
        let status_str = status_to_string(&paper.status);
        let authors_json = serde_json::to_string(&paper.authors)?;
        let manual_links_json = serde_json::to_string(&paper.manual_download_links)?;

        self.conn.execute(
            "UPDATE papers SET
                title = ?, authors_json = ?, year = ?, journal = ?, volume = ?,
                number = ?, pages = ?, doi = ?, url = ?, abstract = ?, status = ?,
                pdf_path = ?, summary_path = ?, notes_path = ?, downloaded_at = ?,
                summarized_at = ?, search_attempts = ?, last_search_error = ?,
                manual_download_links_json = ?, updated_at = datetime('now')
            WHERE citekey = ?",
            params![
                paper.title,
                authors_json,
                paper.year,
                paper.journal,
                paper.volume,
                paper.number,
                paper.pages,
                paper.doi,
                paper.url,
                paper.r#abstract,
                status_str,
                paper.pdf_path,
                paper.summary_path,
                paper.notes_path,
                paper.downloaded_at.map(|d| d.to_rfc3339()),
                paper.summarized_at.map(|d| d.to_rfc3339()),
                paper.search_attempts,
                paper.last_search_error,
                manual_links_json,
                paper.citekey,
            ],
        )?;

        Ok(())
    }

    /// Update only the status of a paper
    pub fn update_status(&self, citekey: &str, status: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE papers SET status = ?, updated_at = datetime('now') WHERE citekey = ?",
            params![status, citekey],
        )?;
        Ok(())
    }

    /// Search papers by title, authors, or abstract
    pub fn search(&self, query: &str) -> Result<Vec<Paper>, DatabaseError> {
        let search_pattern = format!("%{}%", query.to_lowercase());

        let mut stmt = self.conn.prepare(
            "SELECT * FROM papers
             WHERE LOWER(title) LIKE ?
                OR LOWER(authors_json) LIKE ?
                OR LOWER(abstract) LIKE ?
                OR LOWER(citekey) LIKE ?
             ORDER BY added_at DESC
             LIMIT 100"
        )?;

        let rows = stmt.query_map(
            params![&search_pattern, &search_pattern, &search_pattern, &search_pattern],
            |row| self.row_to_paper(row),
        )?;

        let mut papers = Vec::new();
        for row in rows {
            let mut paper = row?;
            paper.citations = self.get_citations(&paper.citekey).unwrap_or_default();
            paper.related_papers = self.get_related_papers(&paper.citekey).unwrap_or_default();
            paper.cited_by = self.get_cited_by(&paper.citekey).unwrap_or_default();
            papers.push(paper);
        }

        Ok(papers)
    }

    /// Get vault statistics
    pub fn stats(&self) -> Result<VaultStats, DatabaseError> {
        let total: usize = self.conn.query_row(
            "SELECT COUNT(*) FROM papers",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*) FROM papers GROUP BY status"
        )?;

        let mut by_status = HashMap::new();
        let rows = stmt.query_map([], |row| {
            let status: String = row.get(0)?;
            let count: i32 = row.get(1)?;
            Ok((status, count))
        })?;

        for row in rows {
            let (status, count) = row?;
            by_status.insert(status, count);
        }

        Ok(VaultStats {
            total,
            by_status,
            last_updated: Utc::now().to_rfc3339(),
        })
    }

    /// Delete a paper by citekey
    pub fn delete(&self, citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute("DELETE FROM papers WHERE citekey = ?", [citekey])?;
        Ok(())
    }

    /// Check if a paper exists
    pub fn exists(&self, citekey: &str) -> Result<bool, DatabaseError> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM papers WHERE citekey = ?",
            [citekey],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Count papers by status
    pub fn count_by_status(&self, status: &str) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM papers WHERE status = ?",
            [status],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // Helper methods

    fn row_to_paper(&self, row: &Row) -> rusqlite::Result<Paper> {
        let status_str: String = row.get("status")?;
        let status = string_to_status(&status_str);

        let authors_json: String = row.get("authors_json")?;
        let authors: Vec<String> = serde_json::from_str(&authors_json).unwrap_or_default();

        let manual_links_json: Option<String> = row.get("manual_download_links_json")?;
        let manual_download_links: Vec<String> = manual_links_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();

        let added_at_str: String = row.get("added_at")?;
        let added_at = DateTime::parse_from_rfc3339(&added_at_str)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let downloaded_at: Option<DateTime<Utc>> = row.get::<_, Option<String>>("downloaded_at")?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc));

        let summarized_at: Option<DateTime<Utc>> = row.get::<_, Option<String>>("summarized_at")?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc));

        Ok(Paper {
            citekey: row.get("citekey")?,
            title: row.get("title")?,
            authors,
            year: row.get("year")?,
            journal: row.get("journal")?,
            volume: row.get("volume")?,
            number: row.get("number")?,
            pages: row.get("pages")?,
            doi: row.get("doi")?,
            url: row.get("url")?,
            r#abstract: row.get("abstract")?,
            status,
            pdf_path: row.get("pdf_path")?,
            summary_path: row.get("summary_path")?,
            notes_path: row.get("notes_path")?,
            added_at,
            downloaded_at,
            summarized_at,
            citations: Vec::new(), // Loaded separately
            cited_by: Vec::new(), // Loaded separately
            related_papers: Vec::new(), // Loaded separately
            search_attempts: row.get("search_attempts")?,
            last_search_error: row.get("last_search_error")?,
            manual_download_links,
        })
    }

    fn get_citations(&self, citekey: &str) -> Result<Vec<Citation>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT citekey, title, authors, year, doi, status
             FROM citations WHERE source_citekey = ?"
        )?;

        let rows = stmt.query_map([citekey], |row| {
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

    fn get_related_papers(&self, citekey: &str) -> Result<Vec<RelatedPaper>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT title, authors_json, year, why_related, vault_citekey
             FROM related_papers WHERE source_citekey = ?"
        )?;

        let rows = stmt.query_map([citekey], |row| {
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

    fn get_cited_by(&self, citekey: &str) -> Result<Vec<String>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT source_citekey FROM citations WHERE citekey = ?"
        )?;

        let rows = stmt.query_map([citekey], |row| {
            row.get::<_, String>(0)
        })?;

        let mut cited_by = Vec::new();
        for row in rows {
            cited_by.push(row?);
        }

        Ok(cited_by)
    }

    fn insert_citation(&self, source_citekey: &str, citation: &Citation) -> Result<(), DatabaseError> {
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
        Ok(())
    }

    fn insert_related_paper(&self, source_citekey: &str, related: &RelatedPaper) -> Result<(), DatabaseError> {
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
        Ok(())
    }

    /// Add a citation to a paper
    pub fn add_citation(&self, source_citekey: &str, citation: &Citation) -> Result<(), DatabaseError> {
        self.insert_citation(source_citekey, citation)
    }

    /// Add a related paper
    pub fn add_related_paper(&self, source_citekey: &str, related: &RelatedPaper) -> Result<(), DatabaseError> {
        self.insert_related_paper(source_citekey, related)
    }

    /// Clear all citations for a paper
    pub fn clear_citations(&self, citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "DELETE FROM citations WHERE source_citekey = ?",
            [citekey],
        )?;
        Ok(())
    }

    /// Clear all related papers for a paper
    pub fn clear_related_papers(&self, citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "DELETE FROM related_papers WHERE source_citekey = ?",
            [citekey],
        )?;
        Ok(())
    }
}

// Helper functions for status conversion

fn status_to_string(status: &PaperStatus) -> &'static str {
    match status {
        PaperStatus::Discovered => "discovered",
        PaperStatus::Wanted => "wanted",
        PaperStatus::Queued => "queued",
        PaperStatus::Downloaded => "downloaded",
        PaperStatus::Summarized => "summarized",
        PaperStatus::Failed => "failed",
    }
}

fn string_to_status(s: &str) -> PaperStatus {
    match s {
        "discovered" => PaperStatus::Discovered,
        "wanted" => PaperStatus::Wanted,
        "queued" => PaperStatus::Queued,
        "downloaded" => PaperStatus::Downloaded,
        "summarized" => PaperStatus::Summarized,
        "failed" => PaperStatus::Failed,
        _ => PaperStatus::Discovered,
    }
}
