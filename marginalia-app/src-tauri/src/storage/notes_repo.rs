//! Notes repository for paper notes and PDF highlights

use rusqlite::{params, Connection};
use chrono::{DateTime, Utc};

use crate::models::notes::{PaperNotes, Highlight, HighlightRect};
use super::DatabaseError;

/// Repository for notes and highlights
pub struct NotesRepo<'a> {
    conn: &'a Connection,
}

impl<'a> NotesRepo<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Get notes for a paper (including highlights)
    pub fn get(&self, citekey: &str) -> Result<Option<PaperNotes>, DatabaseError> {
        let result = self.conn.query_row(
            "SELECT citekey, content, last_modified FROM notes WHERE citekey = ?",
            [citekey],
            |row| {
                let last_modified_str: String = row.get(2)?;
                let last_modified = DateTime::parse_from_rfc3339(&last_modified_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(PaperNotes {
                    citekey: row.get(0)?,
                    content: row.get(1)?,
                    highlights: Vec::new(), // Loaded separately
                    last_modified,
                })
            },
        );

        match result {
            Ok(mut notes) => {
                notes.highlights = self.get_highlights(citekey)?;
                Ok(Some(notes))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DatabaseError::from(e)),
        }
    }

    /// Get or create notes for a paper
    pub fn get_or_create(&self, citekey: &str) -> Result<PaperNotes, DatabaseError> {
        if let Some(notes) = self.get(citekey)? {
            Ok(notes)
        } else {
            let notes = PaperNotes::new(citekey.to_string());
            self.save(&notes)?;
            Ok(notes)
        }
    }

    /// Save notes (insert or update)
    pub fn save(&self, notes: &PaperNotes) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO notes (citekey, content, last_modified)
             VALUES (?, ?, ?)",
            params![
                notes.citekey,
                notes.content,
                notes.last_modified.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Update just the content
    pub fn update_content(&self, citekey: &str, content: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "INSERT INTO notes (citekey, content, last_modified)
             VALUES (?, ?, ?)
             ON CONFLICT(citekey) DO UPDATE SET
                content = excluded.content,
                last_modified = excluded.last_modified",
            params![citekey, content, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Delete notes for a paper
    pub fn delete(&self, citekey: &str) -> Result<(), DatabaseError> {
        self.conn.execute("DELETE FROM notes WHERE citekey = ?", [citekey])?;
        Ok(())
    }

    // Highlight operations

    /// Get all highlights for a paper
    pub fn get_highlights(&self, citekey: &str) -> Result<Vec<Highlight>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, page, rects_json, text, color, note, created_at
             FROM highlights WHERE citekey = ? ORDER BY page, created_at"
        )?;

        let rows = stmt.query_map([citekey], |row| {
            let rects_json: String = row.get(2)?;
            let rects: Vec<HighlightRect> = serde_json::from_str(&rects_json).unwrap_or_default();

            let created_at_str: String = row.get(6)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Highlight {
                id: row.get(0)?,
                page: row.get(1)?,
                rects,
                text: row.get(3)?,
                color: row.get(4)?,
                note: row.get(5)?,
                created_at,
            })
        })?;

        let mut highlights = Vec::new();
        for row in rows {
            highlights.push(row?);
        }
        Ok(highlights)
    }

    /// Add a highlight
    pub fn add_highlight(&self, citekey: &str, highlight: &Highlight) -> Result<(), DatabaseError> {
        let rects_json = serde_json::to_string(&highlight.rects)?;

        self.conn.execute(
            "INSERT INTO highlights (id, citekey, page, rects_json, text, color, note, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                highlight.id,
                citekey,
                highlight.page,
                rects_json,
                highlight.text,
                highlight.color,
                highlight.note,
                highlight.created_at.to_rfc3339(),
            ],
        )?;

        // Ensure notes entry exists
        self.conn.execute(
            "INSERT OR IGNORE INTO notes (citekey, content, last_modified)
             VALUES (?, '', ?)",
            params![citekey, Utc::now().to_rfc3339()],
        )?;

        Ok(())
    }

    /// Delete a highlight
    pub fn delete_highlight(&self, citekey: &str, highlight_id: &str) -> Result<bool, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM highlights WHERE citekey = ? AND id = ?",
            params![citekey, highlight_id],
        )?;
        Ok(count > 0)
    }

    /// Update highlight note
    pub fn update_highlight_note(&self, highlight_id: &str, note: Option<&str>) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE highlights SET note = ? WHERE id = ?",
            params![note, highlight_id],
        )?;
        Ok(())
    }

    /// Update highlight color
    pub fn update_highlight_color(&self, highlight_id: &str, color: &str) -> Result<(), DatabaseError> {
        self.conn.execute(
            "UPDATE highlights SET color = ? WHERE id = ?",
            params![color, highlight_id],
        )?;
        Ok(())
    }

    /// Get highlights for a specific page
    pub fn get_highlights_for_page(&self, citekey: &str, page: i32) -> Result<Vec<Highlight>, DatabaseError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, page, rects_json, text, color, note, created_at
             FROM highlights WHERE citekey = ? AND page = ? ORDER BY created_at"
        )?;

        let rows = stmt.query_map(params![citekey, page], |row| {
            let rects_json: String = row.get(2)?;
            let rects: Vec<HighlightRect> = serde_json::from_str(&rects_json).unwrap_or_default();

            let created_at_str: String = row.get(6)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Highlight {
                id: row.get(0)?,
                page: row.get(1)?,
                rects,
                text: row.get(3)?,
                color: row.get(4)?,
                note: row.get(5)?,
                created_at,
            })
        })?;

        let mut highlights = Vec::new();
        for row in rows {
            highlights.push(row?);
        }
        Ok(highlights)
    }

    /// Count highlights for a paper
    pub fn count_highlights(&self, citekey: &str) -> Result<i64, DatabaseError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM highlights WHERE citekey = ?",
            [citekey],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Delete all highlights for a paper
    pub fn delete_all_highlights(&self, citekey: &str) -> Result<usize, DatabaseError> {
        let count = self.conn.execute(
            "DELETE FROM highlights WHERE citekey = ?",
            [citekey],
        )?;
        Ok(count)
    }
}
