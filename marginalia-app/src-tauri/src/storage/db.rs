//! Database connection management and migrations

use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::fs;
use tracing::{info, warn, error};

use crate::models::vault::VaultIndex;
use crate::models::paper::{Paper, PaperStatus, Citation, RelatedPaper};
use crate::models::notes::{PaperNotes, Highlight};

/// Database error type
#[derive(Debug)]
pub enum DatabaseError {
    ConnectionFailed(String),
    MigrationFailed(String),
    QueryFailed(String),
    JsonParseError(String),
}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            DatabaseError::MigrationFailed(msg) => write!(f, "Migration failed: {}", msg),
            DatabaseError::QueryFailed(msg) => write!(f, "Query failed: {}", msg),
            DatabaseError::JsonParseError(msg) => write!(f, "JSON parse error: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<rusqlite::Error> for DatabaseError {
    fn from(err: rusqlite::Error) -> Self {
        DatabaseError::QueryFailed(err.to_string())
    }
}

impl From<serde_json::Error> for DatabaseError {
    fn from(err: serde_json::Error) -> Self {
        DatabaseError::JsonParseError(err.to_string())
    }
}

/// Wrapper around SQLite connection with helper methods
pub struct Database {
    pub conn: Connection,
    pub vault_path: PathBuf,
}

impl Database {
    /// Get the database file path for a vault
    pub fn db_path(vault_path: &Path) -> PathBuf {
        vault_path.join(".marginalia.sqlite")
    }

    /// Get the legacy JSON index path
    pub fn json_index_path(vault_path: &Path) -> PathBuf {
        vault_path.join(".marginalia_index.json")
    }

    /// Get the backup path for the JSON index
    pub fn json_backup_path(vault_path: &Path) -> PathBuf {
        vault_path.join(".marginalia_index.json.bak")
    }
}

/// Open or create a database for the given vault path
///
/// If no SQLite database exists but a JSON index does, it will be migrated.
pub fn open_database(vault_path: &Path) -> Result<Database, DatabaseError> {
    let db_path = Database::db_path(vault_path);
    let json_path = Database::json_index_path(vault_path);
    let needs_migration = !db_path.exists() && json_path.exists();

    info!("Opening database at {:?}", db_path);

    // Open or create the database
    let conn = Connection::open(&db_path)
        .map_err(|e| DatabaseError::ConnectionFailed(e.to_string()))?;

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| DatabaseError::MigrationFailed(format!("Failed to enable foreign keys: {}", e)))?;

    // Run migrations
    run_migrations(&conn)?;

    let db = Database {
        conn,
        vault_path: vault_path.to_path_buf(),
    };

    // Migrate from JSON if needed
    if needs_migration {
        info!("Migrating from JSON index to SQLite");
        migrate_from_json(&db, &json_path)?;
    }

    Ok(db)
}

/// Run database schema migrations
fn run_migrations(conn: &Connection) -> Result<(), DatabaseError> {
    // Get current schema version
    let current_version: i32 = conn
        .query_row(
            "SELECT version FROM schema_version ORDER BY version DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    info!("Current schema version: {}", current_version);

    // Apply migrations based on version
    if current_version < 1 {
        info!("Applying migration v1: Initial schema");
        apply_v1_schema(conn)?;
    }

    // v2: Add projects support
    if current_version < 2 {
        info!("Applying migration v2: Projects");
        apply_v2_schema(conn)?;
    }

    Ok(())
}

/// Apply the initial v1 schema
fn apply_v1_schema(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute_batch(include_str!("schema.sql"))
        .map_err(|e| DatabaseError::MigrationFailed(format!("Failed to apply v1 schema: {}", e)))?;
    Ok(())
}

/// Apply v2 schema: Add projects support
fn apply_v2_schema(conn: &Connection) -> Result<(), DatabaseError> {
    conn.execute_batch(include_str!("migration_v2.sql"))
        .map_err(|e| DatabaseError::MigrationFailed(format!("Failed to apply v2 schema: {}", e)))?;
    Ok(())
}

/// Migrate data from JSON index to SQLite
fn migrate_from_json(db: &Database, json_path: &Path) -> Result<(), DatabaseError> {
    info!("Reading JSON index from {:?}", json_path);

    let json_content = fs::read_to_string(json_path)
        .map_err(|e| DatabaseError::MigrationFailed(format!("Failed to read JSON: {}", e)))?;

    let index: VaultIndex = serde_json::from_str(&json_content)?;

    info!("Migrating {} papers from JSON", index.papers.len());

    // Import papers
    for (citekey, paper) in &index.papers {
        import_paper(db, paper)?;
    }

    // Import connections
    for conn in &index.connections {
        import_connection(db, conn)?;
    }

    // Import notes from files if they exist
    for (citekey, paper) in &index.papers {
        if let Some(notes_path) = &paper.notes_path {
            let full_path = db.vault_path.join(notes_path);
            if full_path.exists() {
                if let Ok(notes_content) = fs::read_to_string(&full_path) {
                    if let Ok(notes) = serde_json::from_str::<PaperNotes>(&notes_content) {
                        import_notes(db, &notes)?;
                    }
                }
            }
        }
    }

    // Backup the JSON file
    let backup_path = Database::json_backup_path(&db.vault_path);
    fs::rename(json_path, &backup_path)
        .map_err(|e| DatabaseError::MigrationFailed(format!("Failed to backup JSON: {}", e)))?;

    info!("JSON migration complete, backup at {:?}", backup_path);
    Ok(())
}

/// Import a single paper into the database
fn import_paper(db: &Database, paper: &Paper) -> Result<(), DatabaseError> {
    let status_str = match paper.status {
        PaperStatus::Discovered => "discovered",
        PaperStatus::Wanted => "wanted",
        PaperStatus::Queued => "queued",
        PaperStatus::Downloaded => "downloaded",
        PaperStatus::Summarized => "summarized",
        PaperStatus::Failed => "failed",
    };

    let authors_json = serde_json::to_string(&paper.authors)?;
    let manual_links_json = serde_json::to_string(&paper.manual_download_links)?;

    db.conn.execute(
        "INSERT OR REPLACE INTO papers (
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

    // Import citations
    for citation in &paper.citations {
        db.conn.execute(
            "INSERT INTO citations (source_citekey, citekey, title, authors, year, doi, status)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                paper.citekey,
                citation.citekey,
                citation.title,
                citation.authors,
                citation.year,
                citation.doi,
                citation.status,
            ],
        )?;
    }

    // Import related papers
    for related in &paper.related_papers {
        let authors_json = serde_json::to_string(&related.authors)?;
        db.conn.execute(
            "INSERT INTO related_papers (source_citekey, title, authors_json, year, why_related, vault_citekey)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                paper.citekey,
                related.title,
                authors_json,
                related.year,
                related.why_related,
                related.vault_citekey,
            ],
        )?;
    }

    Ok(())
}

/// Import a connection into the database
fn import_connection(db: &Database, conn: &crate::models::vault::PaperConnection) -> Result<(), DatabaseError> {
    db.conn.execute(
        "INSERT OR IGNORE INTO connections (source, target, reason, created_at)
         VALUES (?, ?, ?, ?)",
        params![
            conn.source,
            conn.target,
            conn.reason,
            conn.created_at.to_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Import notes and highlights into the database
fn import_notes(db: &Database, notes: &PaperNotes) -> Result<(), DatabaseError> {
    db.conn.execute(
        "INSERT OR REPLACE INTO notes (citekey, content, last_modified)
         VALUES (?, ?, ?)",
        params![
            notes.citekey,
            notes.content,
            notes.last_modified.to_rfc3339(),
        ],
    )?;

    // Import highlights
    for highlight in &notes.highlights {
        let rects_json = serde_json::to_string(&highlight.rects)?;
        db.conn.execute(
            "INSERT OR REPLACE INTO highlights (id, citekey, page, rects_json, text, color, note, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                highlight.id,
                notes.citekey,
                highlight.page,
                rects_json,
                highlight.text,
                highlight.color,
                highlight.note,
                highlight.created_at.to_rfc3339(),
            ],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_new_database() {
        let dir = tempdir().unwrap();
        let result = open_database(dir.path());
        assert!(result.is_ok());

        let db_path = Database::db_path(dir.path());
        assert!(db_path.exists());
    }

    #[test]
    fn test_schema_version() {
        let dir = tempdir().unwrap();
        let db = open_database(dir.path()).unwrap();

        let version: i32 = db.conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 1);
    }
}
