-- Marginalia SQLite Schema
-- Version 1

-- Core papers table
CREATE TABLE IF NOT EXISTS papers (
    citekey TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    authors_json TEXT NOT NULL DEFAULT '[]',
    year INTEGER,
    journal TEXT,
    volume TEXT,
    number TEXT,
    pages TEXT,
    doi TEXT,
    url TEXT,
    abstract TEXT,
    status TEXT NOT NULL DEFAULT 'discovered',
    pdf_path TEXT,
    summary_path TEXT,
    notes_path TEXT,
    added_at TEXT NOT NULL,
    downloaded_at TEXT,
    summarized_at TEXT,
    search_attempts INTEGER DEFAULT 0,
    last_search_error TEXT,
    manual_download_links_json TEXT DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Citations: papers cited BY a paper
CREATE TABLE IF NOT EXISTS citations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_citekey TEXT NOT NULL,
    citekey TEXT,
    title TEXT,
    authors TEXT,
    year INTEGER,
    doi TEXT,
    status TEXT DEFAULT 'unknown',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (source_citekey) REFERENCES papers(citekey) ON DELETE CASCADE
);

-- Related papers suggested by Claude
CREATE TABLE IF NOT EXISTS related_papers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_citekey TEXT NOT NULL,
    title TEXT NOT NULL,
    authors_json TEXT NOT NULL DEFAULT '[]',
    year INTEGER,
    why_related TEXT NOT NULL,
    vault_citekey TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (source_citekey) REFERENCES papers(citekey) ON DELETE CASCADE
);

-- Graph connections between papers
CREATE TABLE IF NOT EXISTS connections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    target TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source, target),
    FOREIGN KEY (source) REFERENCES papers(citekey) ON DELETE CASCADE,
    FOREIGN KEY (target) REFERENCES papers(citekey) ON DELETE CASCADE
);

-- Notes content per paper
CREATE TABLE IF NOT EXISTS notes (
    citekey TEXT PRIMARY KEY,
    content TEXT NOT NULL DEFAULT '',
    last_modified TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (citekey) REFERENCES papers(citekey) ON DELETE CASCADE
);

-- PDF highlights
CREATE TABLE IF NOT EXISTS highlights (
    id TEXT PRIMARY KEY,
    citekey TEXT NOT NULL,
    page INTEGER NOT NULL,
    rects_json TEXT NOT NULL DEFAULT '[]',
    text TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT 'yellow',
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (citekey) REFERENCES papers(citekey) ON DELETE CASCADE
);

-- Background jobs queue
CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    job_type TEXT NOT NULL,
    citekey TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER DEFAULT 0,
    error TEXT,
    log_path TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_papers_status ON papers(status);
CREATE INDEX IF NOT EXISTS idx_papers_year ON papers(year);
CREATE INDEX IF NOT EXISTS idx_citations_source ON citations(source_citekey);
CREATE INDEX IF NOT EXISTS idx_related_source ON related_papers(source_citekey);
CREATE INDEX IF NOT EXISTS idx_connections_source ON connections(source);
CREATE INDEX IF NOT EXISTS idx_connections_target ON connections(target);
CREATE INDEX IF NOT EXISTS idx_highlights_citekey ON highlights(citekey);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
CREATE INDEX IF NOT EXISTS idx_jobs_citekey ON jobs(citekey);

-- Insert initial schema version
INSERT OR IGNORE INTO schema_version (version) VALUES (1);
