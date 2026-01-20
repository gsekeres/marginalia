-- Migration v2: Add projects support
-- Projects allow users to organize papers into collections

-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6366f1',
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Junction table for many-to-many paper-project relationship
CREATE TABLE IF NOT EXISTS paper_projects (
    paper_citekey TEXT NOT NULL,
    project_id TEXT NOT NULL,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (paper_citekey, project_id),
    FOREIGN KEY (paper_citekey) REFERENCES papers(citekey) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- Index for faster lookups
CREATE INDEX IF NOT EXISTS idx_paper_projects_project ON paper_projects(project_id);
CREATE INDEX IF NOT EXISTS idx_paper_projects_paper ON paper_projects(paper_citekey);

-- Update schema version
INSERT INTO schema_version (version) VALUES (2);
