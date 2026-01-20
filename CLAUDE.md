# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Marginalia is a native macOS academic literature management app built with Tauri. It automates:
- PDF discovery and download from open access sources
- Text extraction and structured summarization using Claude CLI
- Citation graph building with Obsidian-compatible wikilinks
- Notes and highlights on PDFs

**Architecture**: Tauri app with Rust backend and TypeScript/Alpine.js frontend.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│            TypeScript Frontend (Alpine.js + Vite)           │
│                      marginalia-app/src/                      │
└─────────────────────────────────────────────────────────────┘
                              │
                       Tauri IPC
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Rust Backend (Tauri)                      │
│                  marginalia-app/src-tauri/                    │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   Commands   │  │   Services   │  │   Adapters   │       │
│  │ (Tauri IPC)  │  │ (Job Queue)  │  │ (External)   │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
│                            │                                 │
│                   ┌────────┴────────┐                       │
│                   │  SQLite Storage │                       │
│                   └─────────────────┘                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │ Claude Code CLI  │
                    │ (user's login)   │
                    └──────────────────┘
```

## Development Commands

```bash
cd marginalia-app

# Install dependencies
npm install

# Development server (with hot reload)
npm run tauri dev

# Type checking
npm run typecheck

# Build for production
npm run tauri build
```

## Key Directories

### Rust Backend (`src-tauri/src/`)
- `lib.rs` - Tauri app initialization, command registration
- `models/` - Data structures (Paper, VaultIndex, etc.)
- `storage/` - SQLite database layer (schema, repos)
- `services/` - Business logic (job_manager, summarizer_service)
- `commands/` - Tauri command handlers
- `adapters/` - External integrations (Unpaywall, Semantic Scholar, Claude CLI)
- `utils/` - Utility functions

### TypeScript Frontend (`src/`)
- `main.ts` - Entry point, Alpine initialization
- `types.ts` - TypeScript interfaces mirroring Rust models
- `api/client.ts` - Typed Tauri invoke wrappers
- `state/` - State management (vault, papers, jobs)
- `views/` - View helpers (library, paperDetail, network)
- `components/` - UI components (toaster, jobProgress)

## Data Flow

1. **Import**: BibTeX → SQLite database (`.marginalia.sqlite`)
2. **Mark Wanted**: User selects papers → status = "wanted"
3. **Find PDF**: Adapters search sources → `vault/papers/[citekey]/paper.pdf`
4. **Summarize**: Extract text → Claude CLI → JSON → `vault/papers/[citekey]/summary.md`
5. **Link**: Wikilinks connect papers → viewable in Obsidian

## Summarization (Claude Code CLI)

The summarizer calls the Claude CLI with JSON output:
- Requires Claude CLI installed (`brew install anthropics/tap/claude`)
- User must be logged in (`claude login`)
- LLM output is validated with `serde_json` parsing
- On parse failure, raw response is saved to `raw_response.txt`

```rust
// From src-tauri/src/services/summarizer_service.rs
Command::new("claude")
    .args(["--print", "-p", &prompt])
    .output()
```

## Paper Status Flow

```
discovered → wanted → downloaded → summarized
                ↘ (not found)
                  → manual queue (with search links)
```

## Key Commands (Tauri IPC)

| Command | File | Description |
|---------|------|-------------|
| `open_vault` | vault.rs | Open vault, load database |
| `get_papers` | papers.rs | List papers with filters |
| `find_pdf` | pdf_finder.rs | Search and download PDF |
| `summarize_paper` | claude.rs | Summarize via Claude CLI |
| `start_job` | jobs.rs | Start background job |
| `run_diagnostics` | diagnostics.rs | System checks |

## Storage (SQLite)

Database file: `.marginalia.sqlite` in vault directory.

Tables:
- `papers` - Paper metadata, status, paths
- `citations` - Citation references
- `related_papers` - From summarization
- `connections` - Manual graph edges
- `notes` - Paper notes content
- `highlights` - PDF highlights
- `jobs` - Background job queue

## Extending

### Adding a PDF source
1. Create adapter in `src-tauri/src/adapters/`
2. Export from `adapters/mod.rs`
3. Add to source list in `commands/pdf_finder.rs`

### Modifying summary format
1. Update prompt in `services/summarizer_service.rs`
2. Update `ClaudeSummaryOutput` struct
3. Update `format_to_markdown()` function

### Adding a Tauri command
1. Add function in appropriate `commands/*.rs` file
2. Register in `lib.rs` `invoke_handler!` macro
3. Add TypeScript wrapper in `src/api/client.ts`
4. Add types in `src/types.ts`

## Logs

Log files: `~/Library/Application Support/com.marginalia.app/logs/marginalia.log`

Uses `tracing` with daily log rotation.
