# LitVault Development Guide

## Project Overview

LitVault is an agent-based academic literature management system. It automates:
- PDF discovery and download from open access sources
- Text extraction and structured summarization using Claude
- Citation graph building with Obsidian-compatible wikilinks

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Dashboard (FastAPI)                │
│                    http://localhost:8000                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     VaultManager                            │
│           (Index management, paper tracking)                │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────────┐  ┌──────────────┐
        │ BibTeX   │   │ PDF Finder   │  │ Summarizer   │
        │ Parser   │   │ Agent        │  │ Agent        │
        └──────────┘   └──────────────┘  └──────────────┘
```

## Key Files

- `agents/api.py` - FastAPI web dashboard and API endpoints
- `agents/cli.py` - Command-line interface
- `agents/pdf_finder.py` - PDF search agent (Unpaywall, Semantic Scholar, NBER)
- `agents/summarizer.py` - Claude-based summarization
- `agents/vault.py` - Vault management and index
- `agents/models.py` - Pydantic data models
- `app/index.html` - Web dashboard frontend (Alpine.js + Tailwind)

## Development Commands

```bash
# Install in dev mode
pip install -e ".[dev]"

# Run the web server
python -m agents.api

# CLI commands
python -m agents.cli status
python -m agents.cli import references.bib
python -m agents.cli search "collusion"
python -m agents.cli want --all
python -m agents.cli find --limit 5
python -m agents.cli summarize --limit 5
```

## Data Flow

1. **Import**: BibTeX → `VaultIndex` (stored in `.litvault_index.json`)
2. **Mark Wanted**: User selects papers → status changes to "wanted"
3. **Find PDF**: PDFFinder searches sources → downloads to `vault/papers/[citekey]/paper.pdf`
4. **Summarize**: Summarizer extracts text → generates `vault/papers/[citekey]/summary.md`
5. **Link**: Wikilinks connect papers → viewable in Obsidian

## Paper Status Flow

```
discovered → wanted → downloaded → summarized
                ↘ (if not found)
                  → manual queue (with search links)
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/stats` | Vault statistics |
| GET | `/api/papers` | List papers (filterable) |
| GET | `/api/papers/{citekey}` | Get single paper |
| POST | `/api/papers/mark-wanted` | Mark papers as wanted |
| POST | `/api/find-pdfs` | Start PDF finding job |
| POST | `/api/summarize` | Start summarization job |
| GET | `/api/jobs` | List active jobs |
| POST | `/api/papers/{citekey}/upload-pdf` | Upload PDF manually |
| GET | `/api/manual-queue` | Papers needing manual download |

## Extending

### Adding a new PDF source

Edit `agents/pdf_finder.py`:

```python
async def _try_new_source(self, paper: Paper) -> Optional[str]:
    # Your logic here
    return pdf_url_or_none
```

Add to the sources list in `find_pdf()`.

### Modifying the summary format

Edit the prompt in `agents/summarizer.py` `_generate_summary()`.

### Adding new CLI commands

Edit `agents/cli.py` and add:
1. A new `cmd_<name>` function
2. A new subparser in `main()`
3. Add to the commands dispatch dict
