# Marginalia Development Tasks

## Objective

Complete the development of Marginalia, a native macOS academic literature management app built with Tauri. Fix core functionality issues with paper importing, PDF downloading, and summarization. Add project-based organization and improve the network visualization to use a tree structure.

## Context

Marginalia is a Tauri app (Rust backend + TypeScript/Alpine.js frontend) that manages academic papers. Current state:
- BibTeX import works but uses fragile regex parsing
- PDF finding pipeline lacks robustness (no rate limiting, poor error handling)
- Claude CLI summarization has JSON parsing failures
- Network visualization is force-directed but hard to navigate
- No project/folder organization - all papers in one flat list

Key directories:
- `marginalia-app/src-tauri/src/` - Rust backend
- `marginalia-app/src/` - TypeScript frontend
- Database: SQLite at `.marginalia.sqlite` in vault directory

Run with: `cd marginalia-app && npm run tauri dev`

## Current Status

<!-- Ralph will update this section as work progresses -->
- Phase: Not started
- Last completed task: None
- Blockers: None

## Progress Log

<!-- Ralph checks off items as they are completed -->

### Phase 1: Fix Core Import/Download/Summarize Pipeline

#### 1.1 BibTeX Import
- [ ] Replace regex parser in `src-tauri/src/commands/import.rs` with `biblatex` crate or `nom` parser
- [ ] Handle nested braces: `title = {{Nested {Braces}}}`
- [ ] Add validation with meaningful error messages for malformed entries
- [ ] Support entry types beyond `@article` (book, inproceedings, thesis, etc.)
- [ ] Test with complex .bib files containing edge cases

#### 1.2 PDF Download Robustness
- [ ] Add exponential backoff in `src-tauri/src/commands/pdf_finder.rs`
- [ ] Implement rate limiting for Unpaywall and Semantic Scholar APIs
- [ ] Validate downloaded PDFs by checking magic bytes (not just content-type)
- [ ] Handle publisher redirects to login pages gracefully
- [ ] Add arXiv direct download support in `src-tauri/src/adapters/`
- [ ] Show download progress percentage in UI

#### 1.3 Summarization Reliability
- [ ] Improve JSON extraction in `src-tauri/src/services/summarizer_service.rs`
- [ ] Add retry logic when JSON parsing fails (up to 3 attempts)
- [ ] Show partial results on parse failure instead of losing all progress
- [ ] Auto-link extracted related papers to vault papers by DOI/title matching
- [ ] Test summarization end-to-end with 10 different PDFs

### Phase 2: Add Project Organization

#### 2.1 Database Schema
- [ ] Add `projects` table: id, name, color, created_at, updated_at
- [ ] Add `paper_projects` junction table: paper_citekey, project_id
- [ ] Run migration in `src-tauri/src/storage/db.rs`

#### 2.2 Backend Commands
- [ ] Create `src-tauri/src/storage/project_repo.rs` with CRUD operations
- [ ] Create `src-tauri/src/commands/projects.rs` with Tauri commands
- [ ] Register commands in `src-tauri/src/lib.rs`
- [ ] Add `get_papers_by_project` filter to paper queries

#### 2.3 Frontend Integration
- [ ] Add project types to `marginalia-app/src/types.ts`
- [ ] Add project API client in `marginalia-app/src/api/client.ts`
- [ ] Create project state in `marginalia-app/src/state/`
- [ ] Add project sidebar/tabs to main library view in `index.html`
- [ ] Implement add-to-project UI (bulk and single paper)

### Phase 3: Tree-Based Network Visualization

#### 3.1 Replace Force-Directed with Hierarchical Tree
- [ ] Update `marginalia-app/src/views/network.ts` to use vis.js hierarchical layout
- [ ] Structure: Selected paper as root, branches for Cites/Cited-By/Related
- [ ] Add collapsible nodes for deep hierarchies
- [ ] Add toggle to switch between tree and network views

#### 3.2 Improve Paper Linking
- [ ] Auto-match related papers from summaries to vault papers
- [ ] Surface citation relationships (cites, cited-by)
- [ ] Allow manual linking with relationship type selector
- [ ] Show edge labels with relationship reason

### Phase 4: Polish & Error Handling

- [ ] Replace `.unwrap()` calls in `import.rs` (lines 64, 131) with proper error handling
- [ ] Replace `.expect()` in `SummarizerService::default()` with Result return
- [ ] Add user-friendly error messages for all failure modes
- [ ] Add pagination to paper list (currently loads all into memory)
- [ ] Test full workflow: import → find PDF → download → summarize → view network

## Success Criteria

The task is complete when:
- [ ] BibTeX import handles complex entries without crashing
- [ ] PDF download succeeds for >80% of papers with DOIs
- [ ] Summarization produces valid JSON output >95% of the time
- [ ] Papers can be organized into projects from the UI
- [ ] Network view displays papers in a navigable tree structure
- [ ] All `.unwrap()` and `.expect()` calls are replaced with proper error handling
- [ ] Full workflow tested end-to-end with 20 papers

## Completion Marker

<!-- When ALL success criteria checkboxes above are checked, mark this: -->
- [ ] TASK_COMPLETE

## Constraints

- Must maintain compatibility with existing vault format (`.marginalia.sqlite`)
- Must work with user's existing Claude CLI installation
- Do not change the Obsidian-compatible markdown output format
- Keep bundle size reasonable (no heavy JS visualization libraries beyond vis.js)

## Notes

- Run `npm run typecheck` before committing changes
- Test with: `cd marginalia-app && npm run tauri dev`
- Logs at: `~/Library/Application Support/com.marginalia.app/logs/marginalia.log`
- If stuck on a phase, document blockers in "Current Status" section and move to next phase

---
The orchestrator will continue iterations until all success criteria are met or limits are reached.
