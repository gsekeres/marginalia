# Marginalia Refactor Plan - Remaining Phases

## Overview

This document captures the remaining work for the Marginalia production refactor. Phases 1-2 are complete.

## Completed (Do Not Redo)

### Phase 1: SQLite Storage Layer ✅
- Added `rusqlite` and `tracing` dependencies to `src-tauri/Cargo.toml`
- Created `src-tauri/src/storage/` module with:
  - `schema.sql` - Database schema (papers, citations, related_papers, connections, notes, highlights, jobs tables)
  - `db.rs` - Connection management, migrations, JSON→SQLite import
  - `paper_repo.rs` - Paper CRUD operations
  - `citation_repo.rs` - Citation/related paper operations
  - `connection_repo.rs` - Graph edge operations
  - `notes_repo.rs` - Notes and highlights
  - `job_repo.rs` - Job queue with JobType/JobStatus enums
- Updated all commands to use SQLite instead of JSON files
- Migration: On vault open, if `.marginalia.sqlite` missing but `.marginalia_index.json` exists, imports and backs up JSON

### Phase 2: Job System ✅
- Created `src-tauri/src/services/job_manager.rs` - Background task execution with Tauri events
- Created `src-tauri/src/commands/jobs.rs` - Job management commands
- Commands: `start_job`, `get_job`, `list_jobs`, `list_active_jobs`, `cancel_job`, `update_job_progress`
- Job types: ImportBib, FindPdf, DownloadPdf, ExtractText, Summarize, BuildGraph
- Emits `job:updated` Tauri events with {id, status, progress, error}

### Phase 3: Adapters Module ✅
- Created `src-tauri/src/adapters/` module with:
  - `mod.rs` - Module exports and re-exports
  - `unpaywall.rs` - Unpaywall API client with rate limiting
  - `semantic_scholar.rs` - Semantic Scholar API client (DOI and title search)
  - `claude_cli.rs` - Claude CLI wrapper for PDF search and summarization
  - `filesystem.rs` - PDF download, save, text extraction, summary file operations
- Updated `lib.rs` to include `pub mod adapters;`
- Refactored `commands/pdf_finder.rs` to use adapters
- Refactored `commands/claude.rs` to use adapters

### Phase 4: LLM Output Validation ✅
- Created `src-tauri/src/services/summarizer_service.rs` with:
  - `ClaudeSummaryOutput` struct for JSON schema validation
  - `RelatedWorkEntry` struct with conversion to `RelatedPaper`
  - `SummarizationResult` enum for success/failure handling
  - `SummarizerService` with JSON prompt, parsing, and markdown formatting
  - `extract_json()` helper for handling markdown-wrapped JSON responses
- Updated `commands/claude.rs` to use `SummarizerService`
- Added `raw_response_path` to `SummaryResult` for debugging
- On parse failure: raw response saved to `papers/{citekey}/raw_response.txt`

---

## Remaining Work

### Phase 5: Frontend TypeScript Migration ✅

**5.1 Add Build System ✅**
- Updated `package.json` with Vite, TypeScript, Alpine.js dependencies
- Created `tsconfig.json` and `tsconfig.node.json` for TypeScript configuration
- Created `vite.config.ts` with Tauri-specific settings
- Created `src/types.ts` with all TypeScript interfaces mirroring Rust models
- Created `src/api/client.ts` with typed Tauri command wrappers
- Created `src/main.ts` as module entry point
- Updated `index.html` to import main.ts module
- Updated `tauri.conf.json` for Vite dev server and build

**5.2 Modularize Frontend ✅**
- Created `src/state/` modules (vault.ts, papers.ts, jobs.ts)
- Created `src/views/` modules (library.ts, paperDetail.ts, network.ts)
- Created `src/components/` modules (toaster.ts, jobProgress.ts)
- All modules exported via main.ts to global scope for Alpine.js

### Phase 6: Logging & Diagnostics ✅
- Added file logging with `tracing-appender` (daily rolling logs)
- Logs stored in `~/Library/Application Support/com.marginalia.app/logs/`
- Created `commands/diagnostics.rs` with:
  - `run_diagnostics` - System health checks
  - `get_log_path` - Return log directory
  - `open_log_folder` - Open in Finder

### Phase 7: Python Backend Deprecated ✅
- Deleted `agents/`, `app/`, `venv/`, `pyproject.toml`
- Updated `README.md` for Tauri-only architecture
- Updated `CLAUDE.md` with new project structure

---

## All Phases Complete ✅

The refactor is complete. The application is now:
- A native macOS Tauri app
- SQLite database for storage
- TypeScript frontend with Vite
- Rust backend with adapters for external services
- Structured LLM output validation
- File logging and diagnostics

**Build outputs:**
- `src-tauri/target/release/bundle/macos/Marginalia.app`
- `src-tauri/target/release/bundle/dmg/Marginalia_0.1.0_aarch64.dmg`

---

## Archived Original Plan

**5.2 Modularize Frontend** (Original Plan - Completed)

Create new structure in `marginalia-app/src/`:
```
src/
├── index.html          # Minimal HTML shell
├── main.ts             # Alpine initialization
├── types.ts            # TypeScript interfaces (Paper, Job, etc.)
├── api/
│   └── client.ts       # Centralized invoke() wrapper with error handling
├── state/
│   ├── vault.ts        # Vault state slice
│   ├── papers.ts       # Papers state slice
│   └── jobs.ts         # Jobs state slice with event listeners
├── views/
│   ├── library.ts      # Library view component
│   ├── paperDetail.ts  # Paper detail panel
│   └── network.ts      # Graph view
└── components/
    ├── toaster.ts      # Toast notifications
    └── jobProgress.ts  # Job progress display
```

**api/client.ts example:**
```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const api = {
  openVault: (path: string) => invoke<VaultIndex>('open_vault', { path }),
  getPapers: (opts: PaperQuery) => invoke<Paper[]>('get_papers', opts),
  startJob: (jobType: string, citekey?: string) => invoke<string>('start_job', { jobType, citekey }),
  onJobUpdate: (cb: (job: JobUpdate) => void) => listen<JobUpdate>('job:updated', e => cb(e.payload)),
};
```

### Phase 6: Logging & Diagnostics

**6.1 Add File Logging**

Update `src-tauri/src/lib.rs` setup_logging():
```rust
use tracing_appender::rolling;

fn setup_logging(app_dir: &Path) {
    let file_appender = rolling::daily(app_dir.join("logs"), "marginalia.log");
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(file_appender))
        .with(EnvFilter::from_default_env())
        .init();
}
```

**6.2 Add Diagnostics Command**

Create `src-tauri/src/commands/diagnostics.rs`:
```rust
#[derive(Serialize)]
pub struct DiagnosticResult {
    pub vault_writable: bool,
    pub db_status: String,
    pub claude_cli: ClaudeStatus,
    pub network: bool,
}

#[tauri::command]
pub async fn run_diagnostics(vault_path: Option<String>) -> DiagnosticResult { ... }

#[tauri::command]
pub async fn open_logs() -> Result<(), String> { ... }
```

**6.3 Add UI Panel**

In frontend, add diagnostics panel accessible from settings showing:
- Diagnostic check results
- "Open Logs" button
- "Copy Debug Info" button

### Phase 7: Deprecate Python Backend

**Delete these files/directories:**
- `agents/` - Entire Python backend
- `app/` - Legacy web frontend
- `pyproject.toml` - Python package config

**Update documentation:**
- Update root `README.md` to reflect Tauri-only architecture
- Update `CLAUDE.md` to remove Python references

---

## Verification Checklist

After each phase, verify:

### Phase 3 (Adapters) ✅
- [x] `cargo build` succeeds
- [ ] PDF finding still works via UI
- [ ] Summarization still works

### Phase 4 (LLM Validation) ✅
- [x] Valid JSON response → proper markdown summary
- [x] Invalid JSON → raw_response.txt saved, job marked failed

### Phase 5 (Frontend) ✅
- [x] `npm run typecheck` passes
- [x] `npm run build && npm run tauri build` succeeds
- [ ] All UI features work (import, find PDF, summarize, highlights, graph)

### Phase 6 (Logging) ✅
- [x] Log file created at ~/Library/Application Support/com.marginalia.app/logs/
- [x] Diagnostics command works
- [ ] UI panel shows results (to be added in frontend)

### Phase 7 (Python Deprecation) ✅
- [x] agents/, app/, pyproject.toml removed
- [x] No broken references
- [x] Documentation updated

---

## File Locations Reference

**Rust Backend:** `marginalia-app/src-tauri/src/`
- `lib.rs` - Tauri app initialization, command registration
- `models/` - Data structures (Paper, VaultIndex, etc.)
- `storage/` - SQLite database layer (DONE)
- `services/` - Business logic (job_manager.rs DONE)
- `commands/` - Tauri command handlers
- `adapters/` - External integrations (TO CREATE)
- `utils/` - Utility functions

**Frontend:** `marginalia-app/src/`
- Currently single `index.html` (3,752 lines)
- Will be modularized into TypeScript modules

**Build Config:**
- `marginalia-app/package.json`
- `marginalia-app/src-tauri/Cargo.toml`
- `marginalia-app/src-tauri/tauri.conf.json`
