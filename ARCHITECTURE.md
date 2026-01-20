# Marginalia Architecture Documentation

A comprehensive guide to the Marginalia codebase - a native macOS desktop application for academic literature management built with Tauri (Rust) and Alpine.js.

---

## Table of Contents

1. [Overview](#overview)
2. [Directory Structure](#directory-structure)
3. [Backend Architecture (Rust)](#backend-architecture-rust)
4. [Frontend Architecture](#frontend-architecture)
5. [Data Models](#data-models)
6. [API Surface](#api-surface)
7. [External Integrations](#external-integrations)
8. [Data Storage](#data-storage)
9. [Build & Configuration](#build--configuration)

---

## Overview

Marginalia is a desktop application for managing academic papers, inspired by Obsidian's vault-based approach. Users select a local folder as their "vault" and all data lives there.

### Key Design Principles

- **Offline-first**: All data stored locally in user's vault folder
- **Optional AI**: Works fully without Claude CLI; AI features enhance but aren't required
- **Portable vaults**: Vaults can be synced via Dropbox/iCloud
- **Single-file frontend**: All UI in one HTML file using Alpine.js

### Technology Stack

| Layer | Technology |
|-------|------------|
| Framework | Tauri v2 |
| Backend | Rust |
| Frontend | Alpine.js + Tailwind CSS |
| PDF Viewer | PDF.js |
| Graph Viz | vis.js |
| Math Rendering | KaTeX |

---

## Directory Structure

```
Marginalia/
├── marginalia-app/           # Main Tauri application
│   ├── src-tauri/            # Rust backend
│   │   ├── src/
│   │   │   ├── main.rs       # Entry point
│   │   │   ├── lib.rs        # App initialization & command registration
│   │   │   ├── models/       # Data structures
│   │   │   │   ├── mod.rs
│   │   │   │   ├── paper.rs  # Paper, Citation, RelatedPaper
│   │   │   │   ├── vault.rs  # VaultIndex, VaultStats, AppSettings
│   │   │   │   └── notes.rs  # PaperNotes, Highlight
│   │   │   ├── commands/     # Tauri IPC handlers
│   │   │   │   ├── mod.rs
│   │   │   │   ├── vault.rs      # Vault CRUD
│   │   │   │   ├── papers.rs     # Paper CRUD & search
│   │   │   │   ├── import.rs     # BibTeX import/export
│   │   │   │   ├── pdf_finder.rs # Multi-source PDF discovery
│   │   │   │   ├── claude.rs     # AI summarization
│   │   │   │   ├── notes.rs      # Annotations
│   │   │   │   ├── graph.rs      # Knowledge graph
│   │   │   │   └── settings.rs   # App preferences
│   │   │   └── utils/        # Shared utilities
│   │   │       ├── mod.rs
│   │   │       ├── claude.rs     # CLI detection
│   │   │       └── keychain.rs   # macOS Keychain
│   │   ├── Cargo.toml        # Rust dependencies
│   │   ├── tauri.conf.json   # Tauri configuration
│   │   └── build.rs
│   ├── src/                  # Frontend
│   │   └── index.html        # Single-page application (~3700 lines)
│   └── package.json          # Node.js tooling
├── agents/                   # Legacy Python agents (deprecated)
├── app/                      # Legacy web app files
├── vault/                    # Sample vault for testing
├── CLAUDE.md                 # Claude Code instructions
├── ARCHITECTURE.md           # This document
└── README.md
```

---

## Backend Architecture (Rust)

### Entry Points

**`main.rs`** - Minimal entry point
```rust
fn main() {
    marginalia::run()
}
```

**`lib.rs`** - Application bootstrap
- Initializes Tauri plugins (shell, dialog, fs)
- Checks Claude CLI availability on startup
- Registers all 23 IPC command handlers

### Module Organization

#### Models (`models/`)

**`paper.rs`** - Core paper representation
```rust
pub enum PaperStatus {
    Discovered,  // Known but no PDF
    Wanted,      // User wants to find
    Queued,      // In download queue
    Downloaded,  // PDF acquired
    Summarized,  // AI summary generated
    Failed,      // Download/processing failed
}

pub struct Paper {
    // Identity
    pub citekey: String,
    pub title: String,
    pub authors: Vec<String>,

    // Bibliographic
    pub year: Option<i32>,
    pub journal: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub r#abstract: Option<String>,

    // File paths (relative to vault)
    pub pdf_path: Option<String>,
    pub summary_path: Option<String>,
    pub notes_path: Option<String>,

    // Status tracking
    pub status: PaperStatus,
    pub added_at: DateTime<Utc>,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub summarized_at: Option<DateTime<Utc>>,

    // Relationships
    pub citations: Vec<Citation>,
    pub cited_by: Vec<String>,
    pub related_papers: Vec<RelatedPaper>,

    // Search metadata
    pub search_attempts: u32,
    pub last_search_error: Option<String>,
    pub manual_download_links: Vec<String>,
}
```

**`vault.rs`** - Collection management
```rust
pub struct VaultIndex {
    pub papers: HashMap<String, Paper>,
    pub connections: Vec<PaperConnection>,
    pub last_updated: DateTime<Utc>,
    pub source_bib_path: Option<String>,
}

pub struct PaperConnection {
    pub source: String,
    pub target: String,
    pub connection_type: String,
    pub created_at: DateTime<Utc>,
}

pub struct VaultStats {
    pub total: usize,
    pub by_status: HashMap<String, usize>,
}
```

**`notes.rs`** - Annotation system
```rust
pub struct Highlight {
    pub id: String,           // UUID
    pub page: u32,
    pub rects: Vec<Rect>,     // Position coordinates
    pub text: String,         // Extracted text
    pub color: String,        // yellow, green, blue, pink
    pub note: Option<String>, // Optional annotation
    pub created_at: DateTime<Utc>,
}

pub struct PaperNotes {
    pub citekey: String,
    pub content: String,      // Markdown notes
    pub highlights: Vec<Highlight>,
    pub updated_at: DateTime<Utc>,
}
```

#### Commands (`commands/`)

Each module exposes Tauri commands via `#[tauri::command]` macro:

**`vault.rs`** - Vault lifecycle
| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `open_vault` | `path: String` | `VaultIndex` | Opens vault, loads index |
| `create_vault` | `path: String` | `VaultIndex` | Creates new vault structure |
| `get_recent_vaults` | - | `Vec<RecentVault>` | Lists recent vaults |
| `scan_vault_files` | `path: String` | `ScanResult` | Finds existing PDFs/summaries |
| `find_bib_files` | `path: String` | `Vec<String>` | Lists .bib files in vault |

**`papers.rs`** - Paper management
| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `get_papers` | `vault_path, status?, limit?, offset?` | `PapersResponse` | List with pagination |
| `get_paper` | `vault_path, citekey` | `Option<Paper>` | Single paper lookup |
| `get_stats` | `vault_path` | `VaultStats` | Status counts |
| `update_paper_status` | `vault_path, citekey, status` | `()` | Change status |
| `search_papers` | `vault_path, query` | `Vec<Paper>` | Full-text search |
| `add_related_paper` | `vault_path, request` | `AddRelatedPaperResponse` | Add cited paper |

**`pdf_finder.rs`** - PDF acquisition
| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `find_pdf` | `vault_path, citekey` | `FindPdfResult` | Multi-source search |
| `download_pdf` | `vault_path, citekey, url` | `String` | Manual download |

**PDF Search Order:**
1. Unpaywall API (by DOI)
2. Semantic Scholar API (by DOI)
3. Semantic Scholar API (by title)
4. Claude CLI (if available)
5. Returns manual search links if all fail

**`claude.rs`** - AI summarization
| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `check_claude_cli` | - | `ClaudeStatus` | Check CLI availability |
| `summarize_paper` | `vault_path, citekey` | `SummaryResult` | Generate summary |

Summarization workflow:
1. Extract text from PDF (max 100k chars)
2. Build structured prompt
3. Call `claude --print -p "..."`
4. Parse response for sections
5. Extract related papers
6. Save as `summary.md`

**`graph.rs`** - Knowledge graph
| Command | Parameters | Returns | Description |
|---------|------------|---------|-------------|
| `get_graph` | `vault_path` | `GraphData` | Nodes & edges |
| `connect_papers` | `vault_path, source, target, reason` | `String` | Create edge |
| `disconnect_papers` | `vault_path, source, target` | `()` | Remove edge |

#### Utils (`utils/`)

**`claude.rs`** - CLI detection
```rust
pub fn is_claude_available() -> bool {
    Command::new("which")
        .arg("claude")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

**`keychain.rs`** - Credential storage
```rust
pub fn store_keychain(account: &str, value: &str) -> Result<(), String>
pub fn get_keychain(account: &str) -> Result<Option<String>, String>
pub fn delete_keychain(account: &str) -> Result<(), String>
```

---

## Frontend Architecture

### Single-File Application

All frontend code lives in `marginalia-app/src/index.html` (~3700 lines):

```
index.html
├── <head>
│   ├── Google Fonts (Inter, JetBrains Mono, Source Serif 4)
│   ├── Tailwind CSS (CDN)
│   ├── marked.js (Markdown)
│   ├── PDF.js
│   ├── KaTeX
│   └── vis.js (network graphs)
│   └── <style> CSS variables & components
├── <body x-data="app()">
│   ├── Sidebar (fixed, 220px)
│   ├── Main Area
│   │   ├── Topbar (search, refresh)
│   │   └── Content (view-dependent)
│   ├── Paper Detail Modal (full-page overlay)
│   ├── Import Modal
│   ├── Sync Modal
│   ├── Vault Selector Modal
│   └── Browse Modal
└── <script>
    └── Alpine.js app() component
```

### Alpine.js Component Structure

```javascript
function app() {
    return {
        // Vault state
        vaultPath: null,
        recentVaults: [],
        showVaultModal: false,
        claudeStatus: { available: false, version: null, logged_in: false },

        // Data state
        stats: { by_status: {} },
        papers: [],
        totalPapers: 0,
        offset: 0,
        searchQuery: '',
        currentFilter: null,

        // UI state
        currentView: 'library',  // 'library' | 'network'
        showPaperDetail: false,
        paperDetail: null,
        paperSummary: null,
        detailView: 'summary',   // 'summary' | 'pdf'

        // PDF viewer state
        pdfDoc: null,
        pdfCurrentPage: 1,
        pdfTotalPages: 0,
        pdfScale: 1.2,
        highlightColor: 'yellow',

        // Notes state
        notesTab: 'notes',       // 'notes' | 'highlights'
        paperNotesContent: '',
        paperHighlights: [],

        // Network graph state
        networkGraph: null,
        connectSource: '',
        connectTarget: '',

        // Methods...
        async init() { ... },
        async refreshData() { ... },
        async viewPaperDetail(citekey) { ... },
        async findPDFSingle(citekey) { ... },
        async summarizeSinglePaper(citekey) { ... },
        // ... 50+ more methods
    }
}
```

### Key UI Components

**Sidebar**
- Navigation (Library, Network)
- Stats display (Discovered, Downloaded, Summarized)
- Import/Sync actions
- User info (when authenticated)

**Library View**
- Status filter cards (clickable stats)
- Papers table with:
  - Status dot
  - Title, authors, citekey
  - Year
  - Status badge
  - Actions (PDF, Find PDF)
- Pagination

**Paper Detail View**
- Back navigation
- View toggle (Summary / PDF)
- Action buttons (Find PDF, Summarize)
- Left margin: metadata (year, journal, DOI, dates)
- Main content: BibTeX, abstract, summary, related papers

**PDF Viewer**
- Toolbar: page info, zoom controls, highlight colors
- Scrollable page container (all pages rendered)
- Notes panel (collapsible):
  - Notes tab: Markdown editor with preview
  - Highlights tab: List of annotations

**Network Graph**
- vis.js canvas
- Search bar
- Connection panel (source, target, reason)

### CSS Design System

```css
:root {
    /* Paper & Ink palette */
    --bg: #FBF7EF;
    --surface: #F4EEE3;
    --text: #1F2328;
    --text-muted: #57606A;
    --accent: #22324A;

    /* Status colors */
    --status-discovered: #8B949E;
    --status-downloaded: #2D6A4F;
    --status-summarized: #5B4B8A;
    --status-failed: #9B2C2C;

    /* Typography */
    --font-ui: 'Inter', sans-serif;
    --font-reading: 'Source Serif 4', serif;
    --font-mono: 'JetBrains Mono', monospace;
}
```

---

## Data Models

### Entity Relationships

```
┌─────────────────────────────────────────────────────────┐
│                     VaultIndex                          │
│  (stored in .marginalia_index.json)                     │
├─────────────────────────────────────────────────────────┤
│  papers: HashMap<citekey, Paper>                        │
│  connections: Vec<PaperConnection>                      │
│  last_updated: DateTime                                 │
│  source_bib_path: Option<String>                        │
└───────────────────────┬─────────────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        ▼               ▼               ▼
┌───────────────┐ ┌───────────────┐ ┌───────────────┐
│    Paper      │ │    Paper      │ │    Paper      │
├───────────────┤ ├───────────────┤ ├───────────────┤
│ citekey       │ │ citekey       │ │ citekey       │
│ title         │ │ title         │ │ title         │
│ authors[]     │ │ authors[]     │ │ authors[]     │
│ status        │ │ status        │ │ status        │
│ pdf_path ─────┼─┼───────────────┼─┼─► papers/{citekey}/
│ summary_path  │ │ summary_path  │ │     ├── paper.pdf
│ related_papers│ │ cited_by[]    │ │     ├── summary.md
│ citations[]   │ │               │ │     └── notes.json
└───────────────┘ └───────────────┘ └───────────────┘
        │                 ▲
        │  cited_by       │
        └─────────────────┘

┌─────────────────────────┐
│   PaperConnection       │
├─────────────────────────┤
│ source: citekey         │──────┐
│ target: citekey         │──────┤
│ connection_type: String │      │
│ created_at: DateTime    │      │
└─────────────────────────┘      │
        Used for graph edges ◄───┘
```

### Paper Lifecycle

```
                    ┌──────────────┐
                    │  BibTeX      │
                    │  Import      │
                    └──────┬───────┘
                           │
                           ▼
┌──────────────────────────────────────────────────────────┐
│                     DISCOVERED                           │
│  Paper exists in index, no PDF yet                       │
└──────────────────────────┬───────────────────────────────┘
                           │ find_pdf()
                           ▼
        ┌──────────────────┴──────────────────┐
        │                                     │
        ▼                                     ▼
┌───────────────┐                    ┌───────────────┐
│  DOWNLOADED   │                    │    FAILED     │
│  PDF acquired │                    │  Search failed│
└───────┬───────┘                    └───────────────┘
        │ summarize_paper()                  │
        ▼                                    │ retry
┌───────────────┐                            │
│  SUMMARIZED   │◄───────────────────────────┘
│ AI summary    │
│ generated     │
└───────────────┘
```

---

## API Surface

### Complete Command Reference

**Total: 23 commands**

#### Vault Management (8 commands)
```typescript
// Open existing vault
invoke('open_vault', { path: '/path/to/vault' }) → VaultIndex

// Create new vault with structure
invoke('create_vault', { path: '/path/to/vault' }) → VaultIndex

// Get recently opened vaults
invoke('get_recent_vaults') → RecentVault[]

// Save index to disk
invoke('save_index', { vaultPath, index }) → void

// Add to recent vaults list
invoke('add_recent_vault', { path, paperCount }) → void

// Get vault statistics
invoke('get_vault_stats', { path }) → VaultStats

// Scan for existing PDFs/summaries
invoke('scan_vault_files', { path }) → ScanResult

// Find .bib files in vault
invoke('find_bib_files', { path }) → string[]
```

#### Paper Operations (6 commands)
```typescript
// List papers with filtering
invoke('get_papers', {
    vaultPath,
    status?: string,    // 'discovered' | 'downloaded' | etc.
    limit?: number,     // default 100
    offset?: number     // for pagination
}) → { total: number, papers: Paper[] }

// Get single paper
invoke('get_paper', { vaultPath, citekey }) → Paper | null

// Get status counts
invoke('get_stats', { vaultPath }) → VaultStats

// Update paper status
invoke('update_paper_status', { vaultPath, citekey, status }) → void

// Search papers
invoke('search_papers', { vaultPath, query }) → Paper[]

// Add related paper from summary
invoke('add_related_paper', {
    vaultPath,
    request: { title, authors, year, source_citekey }
}) → { status: 'added' | 'exists', citekey }
```

#### Import/Export (2 commands)
```typescript
// Import BibTeX file
invoke('import_bibtex', { vaultPath, bibPath }) → ImportResult

// Export to BibTeX
invoke('export_bibtex', { vaultPath, outputPath }) → void
```

#### PDF Operations (2 commands)
```typescript
// Search and download PDF
invoke('find_pdf', { vaultPath, citekey }) → {
    success: boolean,
    source?: string,
    manual_links?: string[]
}

// Download from specific URL
invoke('download_pdf', { vaultPath, citekey, url }) → string
```

#### AI/Claude (2 commands)
```typescript
// Check CLI status
invoke('check_claude_cli') → {
    available: boolean,
    version?: string,
    logged_in: boolean
}

// Generate summary
invoke('summarize_paper', { vaultPath, citekey }) → {
    success: boolean,
    summary_path?: string,
    error?: string
}
```

#### Notes & Highlights (4 commands)
```typescript
// Get notes and highlights
invoke('get_notes', { vaultPath, citekey }) → PaperNotes

// Save markdown notes
invoke('save_notes', { vaultPath, citekey, content }) → void

// Add highlight
invoke('add_highlight', { vaultPath, citekey, highlight }) → string

// Delete highlight
invoke('delete_highlight', { vaultPath, citekey, highlightId }) → void
```

#### Knowledge Graph (3 commands)
```typescript
// Get graph data
invoke('get_graph', { vaultPath }) → {
    nodes: GraphNode[],
    edges: GraphEdge[]
}

// Connect papers
invoke('connect_papers', {
    vaultPath, source, target, reason
}) → 'connected' | 'exists'

// Remove connection
invoke('disconnect_papers', { vaultPath, source, target }) → void
```

#### Settings (2 commands)
```typescript
// Load settings
invoke('get_settings') → AppSettings

// Save settings
invoke('save_settings', { settings }) → void
```

---

## External Integrations

### PDF Discovery Sources

| Source | Method | Data Required | Rate Limit |
|--------|--------|---------------|------------|
| **Unpaywall** | `api.unpaywall.org/v2/{doi}` | DOI | Unlimited (with email) |
| **Semantic Scholar** | `api.semanticscholar.org/graph/v1/paper/DOI:{doi}` | DOI | 100/5min |
| **Semantic Scholar** | Title search endpoint | Title | 100/5min |
| **Claude CLI** | `claude --print -p "Find PDF..."` | Title, authors | Depends on plan |

### Search Fallback Links

When automated search fails, these manual search URLs are generated:
- Google Scholar: `scholar.google.com/scholar?q={encoded_title}`
- SSRN: `papers.ssrn.com/sol3/results.cfm?txtKey_Words={encoded_title}`
- NBER: `nber.org/papers?q={encoded_title}`
- arXiv: `arxiv.org/search/?searchtype=all&query={encoded_title}`

### Claude CLI Integration

**Detection:**
```rust
Command::new("which").arg("claude").output()
```

**Summarization prompt structure:**
```
You are an academic research assistant. Summarize this paper...

## Summary
[1-2 paragraph overview]

## Key Contributions
- [Bullet points]

## Methodology
[Brief description]

## Main Results
- [Key findings]

## Related Work
- Title: [paper title]
  Authors: [names]
  Year: [year]
  Why Related: [explanation]
```

---

## Data Storage

### Vault Structure

```
User's Vault (e.g., ~/Documents/MyResearch/)
├── .marginalia_index.json     # Main database
├── papers/
│   ├── author2020/
│   │   ├── paper.pdf          # Downloaded PDF
│   │   ├── summary.md         # AI-generated summary
│   │   └── notes.json         # Highlights & annotations
│   ├── smith2019/
│   │   └── ...
│   └── ...
└── references.bib             # Optional: original BibTeX
```

### App Settings Location

```
~/Library/Application Support/com.marginalia/
├── settings.json              # App preferences
│   {
│     "last_vault_path": "/Users/.../vault",
│     "unpaywall_email": "user@example.com",
│     "theme": "light"
│   }
└── recent_vaults.json         # Recently opened vaults
    [
      { "path": "/path/to/vault", "name": "MyVault", "paper_count": 42 }
    ]
```

### Index File Format

`.marginalia_index.json`:
```json
{
  "papers": {
    "author2020": {
      "citekey": "author2020",
      "title": "Paper Title",
      "authors": ["First Author", "Second Author"],
      "year": 2020,
      "journal": "Journal Name",
      "doi": "10.1234/example",
      "status": "summarized",
      "pdf_path": "papers/author2020/paper.pdf",
      "summary_path": "papers/author2020/summary.md",
      "added_at": "2024-01-15T10:30:00Z",
      "downloaded_at": "2024-01-15T10:35:00Z",
      "summarized_at": "2024-01-15T10:40:00Z",
      "citations": [],
      "cited_by": ["other2021"],
      "related_papers": [
        {
          "title": "Related Paper",
          "authors": ["Someone"],
          "year": 2019,
          "why_related": "Extends methodology"
        }
      ]
    }
  },
  "connections": [
    {
      "source": "author2020",
      "target": "other2021",
      "connection_type": "cites",
      "created_at": "2024-01-15T10:45:00Z"
    }
  ],
  "last_updated": "2024-01-15T10:45:00Z",
  "source_bib_path": "/path/to/references.bib"
}
```

---

## Build & Configuration

### Rust Dependencies (`Cargo.toml`)

```toml
[dependencies]
# Framework
tauri = { version = "2", features = ["macos-private-api"] }
tauri-plugin-shell = "2"
tauri-plugin-dialog = "2"
tauri-plugin-fs = "2"

# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# HTTP client
reqwest = { version = "0.12", features = ["json"] }

# PDF processing
pdf-extract = "0.8"

# BibTeX parsing
biblatex = "0.10"

# Text processing
regex = "1"
urlencoding = "2"

# System integration
security-framework = "3"  # macOS Keychain
dirs = "6"                 # Standard directories
notify = "8"               # File watching

# Utilities
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
```

### Tauri Configuration (`tauri.conf.json`)

```json
{
  "productName": "Marginalia",
  "version": "0.1.0",
  "identifier": "com.marginalia.app",
  "build": {
    "frontendDist": "../src"
  },
  "app": {
    "windows": [{
      "title": "Marginalia",
      "width": 1400,
      "height": 900,
      "minWidth": 800,
      "minHeight": 600
    }],
    "security": {
      "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval' ...",
      "assetProtocol": {
        "enable": true,
        "scope": ["$HOME/**", "/**"]
      }
    }
  },
  "plugins": {
    "shell": { "open": true }
  },
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "12.0"
    }
  }
}
```

### Development Commands

```bash
# Install dependencies
cd marginalia-app
npm install

# Development (hot reload)
npm run tauri dev

# Build release
npm run tauri build

# Build release (universal binary)
npm run tauri build -- --target universal-apple-darwin
```

### Environment Variables

Create `.env` in project root for development:
```
UNPAYWALL_EMAIL=your.email@example.com
```

---

## Appendix: File Quick Reference

| Purpose | File Path |
|---------|-----------|
| App entry | `marginalia-app/src-tauri/src/main.rs` |
| Command registration | `marginalia-app/src-tauri/src/lib.rs` |
| Paper model | `marginalia-app/src-tauri/src/models/paper.rs` |
| Vault model | `marginalia-app/src-tauri/src/models/vault.rs` |
| PDF finder | `marginalia-app/src-tauri/src/commands/pdf_finder.rs` |
| Claude integration | `marginalia-app/src-tauri/src/commands/claude.rs` |
| BibTeX import | `marginalia-app/src-tauri/src/commands/import.rs` |
| Frontend UI | `marginalia-app/src/index.html` |
| Rust deps | `marginalia-app/src-tauri/Cargo.toml` |
| Tauri config | `marginalia-app/src-tauri/tauri.conf.json` |
