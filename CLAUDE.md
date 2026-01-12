# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Marginalia is an agent-based academic literature management platform. It works with users' existing Claude Code subscriptions to automate:
- PDF discovery and download from open access sources
- Text extraction and structured summarization using Claude
- Citation graph building with Obsidian-compatible wikilinks

**Production Vision**: A hosted platform where researchers can bring their bibliography, and Marginalia handles finding PDFs and generating summaries using their Claude subscription.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 Static Frontend (Hugo site)                 │
│            https://gabesekeres.com/marginalia/                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  FastAPI Backend (Render)                   │
│              https://marginalia-api.onrender.com              │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────────┐  ┌──────────────┐
        │ BibTeX   │   │ PDF Finder   │  │ Summarizer   │
        │ Parser   │   │ Agent        │  │ Agent        │
        └──────────┘   └──────────────┘  └──────────────┘
                                               │
                                               ▼
                                    ┌──────────────────┐
                                    │ Claude Code CLI  │
                                    │ (user's OAuth)   │
                                    └──────────────────┘
```

## Development Commands

```bash
# Install
pip install -e ".[dev]"

# Run locally
python -m agents.api                    # Start server at localhost:8000

# CLI commands
python -m agents.cli status             # Vault statistics
python -m agents.cli import refs.bib    # Import bibliography
python -m agents.cli want --all         # Mark all papers as wanted
python -m agents.cli find --limit 5     # Find PDFs (rate limited)
python -m agents.cli summarize --limit 5 # Summarize papers
```

## Key Files

- `agents/api.py` - FastAPI endpoints, background job management
- `agents/pdf_finder.py` - Multi-source PDF search (Unpaywall, Semantic Scholar, NBER)
- `agents/summarizer.py` - Claude Code CLI integration, JSON parsing to markdown
- `agents/vault.py` - Index management, paper tracking
- `app/index.html` - Local development dashboard (Alpine.js + Tailwind)

## Data Flow

1. **Import**: BibTeX → VaultIndex (`.marginalia_index.json`)
2. **Mark Wanted**: User selects papers → status = "wanted"
3. **Find PDF**: PDFFinder searches sources → `vault/papers/[citekey]/paper.pdf`
4. **Summarize**: Extract text → Claude CLI → JSON → `vault/papers/[citekey]/summary.md`
5. **Link**: Wikilinks connect papers → viewable in Obsidian

## Summarization (Claude Code CLI)

The summarizer shells out to `claude -p` with JSON output:
- Requires `CLAUDE_CODE_OAUTH_TOKEN` in `.env` (from `claude setup-token`)
- **Critical**: Do NOT set `ANTHROPIC_API_KEY` - the CLI prefers it over OAuth
- Pass `env=os.environ.copy()` to subprocess to inherit the token

```python
result = subprocess.run(
    ["claude", "-p", prompt, "--output-format", "json"],
    capture_output=True,
    env=os.environ.copy(),  # Required for OAuth token
)
```

## Paper Status Flow

```
discovered → wanted → downloaded → summarized
                ↘ (not found)
                  → manual queue (with search links)
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/stats` | Vault statistics |
| GET | `/api/papers` | List papers (filter by status, search) |
| POST | `/api/papers/mark-wanted` | Mark papers as wanted |
| POST | `/api/find-pdfs` | Batch PDF finding job |
| POST | `/api/papers/{citekey}/find-pdf` | Find single PDF |
| POST | `/api/summarize` | Batch summarization job |
| POST | `/api/papers/{citekey}/summarize` | Summarize/re-summarize single |
| POST | `/api/papers/{citekey}/upload-pdf` | Upload PDF manually |
| GET | `/api/manual-queue` | Papers needing manual download |

## Deployment

**Current Setup**:
- Frontend: Static HTML on Hugo site (`/static/marginalia/dashboard.html`)
- Backend: Render deployment (`https://marginalia-api.onrender.com`)
- Storage: Persistent disk on Render for vault data

**Environment Variables (Render)**:
- `CLAUDE_CODE_OAUTH_TOKEN` - Required for summarization
- `UNPAYWALL_EMAIL` - For better rate limits
- `VAULT_PATH` - Path to vault directory

## Extending

### Adding a PDF source
Edit `agents/pdf_finder.py`, add method `_try_new_source()`, add to sources list in `find_pdf()`.

### Modifying summary format
Edit prompt in `agents/summarizer.py` `_generate_summary()`.

### Adding CLI commands
Edit `agents/cli.py`: add `cmd_<name>` function, add subparser in `main()`.
