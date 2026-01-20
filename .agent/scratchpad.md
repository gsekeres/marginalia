# Marginalia Development Scratchpad

## Current Status
- **Phase**: 1 - Fix Core Import/Download/Summarize Pipeline
- **Last completed**: Added retry logic for summarization JSON parse failures
- **Blockers**: None

## Key Findings from Exploration

### BibTeX Import (`src-tauri/src/commands/import.rs`)
- Uses regex parser with CRITICAL nested brace bug at line 131: `\{([^}]*)\}` stops at first `}`
- `biblatex` crate is in Cargo.toml (line 27) but NOT USED
- Two `.unwrap()` calls at lines 64 and 131 (low risk - static regex)
- Entry types captured but ignored (line 67)
- `clean_bibtex_string()` strips ALL braces indiscriminately (lines 142-148)

### PDF Finding (`src-tauri/src/commands/pdf_finder.rs`, `src-tauri/src/adapters/`)
- 4 adapters: Unpaywall, Semantic Scholar, Claude CLI, Filesystem
- NO rate limiting or backoff implemented
- NO arXiv adapter (missing)
- Content-type validation only - no magic byte check
- Timeouts: 30s API, 60s download

### Summarization (`src-tauri/src/services/summarizer_service.rs`)
- JSON extraction handles multiple formats (pure JSON, markdown blocks)
- NO retry logic on parse failure
- Raw response saved to `raw_response.txt` on failure
- Related papers stored but NOT auto-linked to vault papers

### Database (`src-tauri/src/storage/`)
- 8 tables: papers, citations, related_papers, connections, notes, highlights, jobs, schema_version
- NO projects table exists
- Migration system in place (version-based)

### Network Visualization (`src/views/network.ts`)
- Uses vis.js with force-directed (barnesHut/forceAtlas2Based)
- Status-based node coloring
- NO hierarchical tree layout option

---

## Phase 1: Fix Core Import/Download/Summarize Pipeline

### 1.1 BibTeX Import
- [x] Replace regex parser with `biblatex` crate (already in Cargo.toml)
- [x] Handle nested braces properly (biblatex handles this natively)
- [x] Add validation with meaningful error messages (proper Result propagation)
- [x] Support all entry types (book, inproceedings, thesis, etc.) - biblatex handles all types
- [x] Replace `.unwrap()` calls with proper error handling (using .ok() + map)

### 1.2 PDF Download Robustness
- [x] Add exponential backoff in adapters (utils/http.rs with_retry())
- [x] Implement rate limiting for Unpaywall (100k/day) and Semantic Scholar (100/5min) (utils/http.rs RateLimiter)
- [x] Validate PDFs by magic bytes (`%PDF-`) (utils/http.rs is_valid_pdf())
- [x] Handle publisher redirects to login pages (utils/http.rs is_likely_login_page())
- [x] Add arXiv adapter (adapters/arxiv.rs)
- [ ] Show download progress in UI

### 1.3 Summarization Reliability
- [x] Add retry logic (up to 3 attempts) on JSON parse failure
  - Implemented in `summarizer_service.rs` with `MAX_RETRIES = 3`
  - Uses `build_retry_prompt()` with increasingly emphatic JSON instructions
  - Includes error message from previous attempt in retry prompt
  - Truncates paper text on retries (50k→30k chars) to give model more room
- [ ] Show partial results on parse failure
- [ ] Auto-link related papers to vault papers by DOI/title matching

---

## Phase 2: Add Project Organization

### 2.1 Database Schema
- [ ] Add `projects` table: id, name, color, created_at, updated_at
- [ ] Add `paper_projects` junction table
- [ ] Create migration v2

### 2.2 Backend Commands
- [ ] Create `project_repo.rs` with CRUD
- [ ] Create `commands/projects.rs`
- [ ] Register in `lib.rs`
- [ ] Add `get_papers_by_project` filter

### 2.3 Frontend Integration
- [ ] Add project types to `types.ts`
- [ ] Add project API client
- [ ] Create project state
- [ ] Add project sidebar/tabs
- [ ] Implement add-to-project UI

---

## Phase 3: Tree-Based Network Visualization

### 3.1 Hierarchical Layout
- [ ] Add vis.js hierarchical layout option
- [ ] Structure: root paper → Cites/Cited-By/Related branches
- [ ] Add collapsible nodes
- [ ] Add toggle between tree and force-directed views

### 3.2 Paper Linking
- [ ] Auto-match related papers to vault
- [ ] Surface citation relationships
- [ ] Allow manual linking with type selector
- [ ] Show edge labels

---

## Phase 4: Polish & Error Handling

- [ ] Replace all `.unwrap()` and `.expect()` with proper error handling
- [ ] Add user-friendly error messages
- [ ] Add pagination to paper list
- [ ] Test full workflow end-to-end

---

## Notes

- `biblatex` crate already in dependencies - just need to use it
- arXiv API: `https://export.arxiv.org/api/query?id_list={arxiv_id}`
- PDF magic bytes: `%PDF-` (first 5 bytes)
- vis.js hierarchical: `layout: { hierarchical: { direction: 'UD' } }`
