# LitVault

An agent-based academic literature management system that automates the tedious parts of building a research library.

## What It Does

LitVault uses AI agents to:
1. **Find PDFs** - Searches Unpaywall, Semantic Scholar, NBER, and other open-access sources
2. **Summarize Papers** - Extracts text and generates structured summaries using Claude
3. **Build Citation Graphs** - Links papers together through their citations
4. **Create an Obsidian Vault** - All summaries are Obsidian-compatible markdown with wikilinks

## Quick Start

### 1. Install Dependencies

```bash
cd /path/to/LitVault
pip install -e .
```

### 2. Set Up Environment

```bash
cp .env.example .env
# Edit .env and add your ANTHROPIC_API_KEY
```

### 3. Import Your Bibliography

```bash
# Using CLI
python -m agents.cli import references.bib

# Or start the web dashboard
python -m agents.api
# Then open http://localhost:8000 and use the Import button
```

### 4. Find and Summarize Papers

```bash
# Mark papers you want
python -m agents.cli want --all  # or specific citekeys

# Find PDFs (searches open access sources)
python -m agents.cli find

# Summarize downloaded papers
python -m agents.cli summarize
```

## Web Dashboard

The web dashboard provides a visual interface for managing your library:

```bash
python -m agents.api
# Open http://localhost:8000
```

Features:
- Browse papers by status
- Search by title/author
- Mark papers as wanted
- Upload PDFs manually
- View summaries
- Track download jobs

## CLI Commands

```bash
python -m agents.cli <command>

Commands:
  status      Show vault statistics
  import      Import papers from BibTeX file
  search      Search for papers by title/author
  want        Mark papers for download
  find        Find and download PDFs
  summarize   Generate summaries for downloaded papers
  register    Register a manually downloaded PDF
  manual      Show papers needing manual download
  list        List papers with optional filters
  index       Generate Obsidian index page
```

## Obsidian Integration

The `vault/` folder is an Obsidian-compatible vault:

1. Open Obsidian
2. "Open folder as vault" → select `vault/`
3. Papers appear as linked notes with `[[wikilinks]]`

Each paper gets:
- `summary.md` - Structured summary with frontmatter
- `paper.pdf` - The downloaded PDF
- `full_text.md` - Extracted text (optional)

## How PDF Finding Works

The PDF Finder agent searches these sources in order:

1. **Unpaywall** - Free API for finding open-access versions via DOI
2. **Semantic Scholar** - Academic search with open-access PDFs
3. **NBER** - Working papers from National Bureau of Economic Research
4. **Manual Queue** - If no PDF found, generates search links for you

Expected success rate: ~70-85% for economics papers.

Papers that can't be found automatically are added to the "Manual Download Queue" with pre-generated search links. You can then:
- Click the links to search manually
- Download the PDF yourself
- Upload it through the web dashboard

## How Summarization Works

The Summarizer agent:
1. Extracts text from PDF using `pdfplumber` or `PyMuPDF`
2. Sends text to Claude with a structured prompt
3. Generates a summary with:
   - Overview paragraph
   - Key contributions
   - Methodology
   - Main results
   - Related work
4. Extracts citations and creates Obsidian wikilinks

## Directory Structure

```
LitVault/
├── agents/              # Python agents and API
│   ├── api.py          # FastAPI web dashboard
│   ├── cli.py          # Command-line interface
│   ├── pdf_finder.py   # PDF search agent
│   ├── summarizer.py   # Summary generation agent
│   ├── vault.py        # Vault management
│   ├── models.py       # Data models
│   └── bibtex_parser.py
├── app/                 # Web frontend
│   └── index.html      # Dashboard UI
├── vault/              # Obsidian vault
│   ├── papers/         # Paper folders
│   ├── templates/      # Summary templates
│   └── index.md        # Auto-generated index
├── scripts/            # Utility scripts
├── references.bib      # Master bibliography
├── .env               # Environment variables (create from .env.example)
└── pyproject.toml     # Python project config
```

## Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `ANTHROPIC_API_KEY` | Claude API key for summaries | Yes |
| `UNPAYWALL_EMAIL` | Email for Unpaywall API (better rate limits) | No |
| `SEMANTIC_SCHOLAR_API_KEY` | Semantic Scholar API key | No |
| `VAULT_PATH` | Path to vault directory | No (defaults to `./vault`) |

### API Rate Limits

The agents include rate limiting to be respectful to APIs:
- Minimum 1 second between requests
- Automatic retries with backoff

## Costs

- **PDF Finding**: Free (uses open APIs)
- **Summarization**: ~$0.01-0.03 per paper (Claude Sonnet)
- **Storage**: ~1-2MB per paper (PDF + markdown)

For 344 papers: approximately $3-10 for full summarization.

## License

MIT License - see LICENSE file.

## Contributing

Contributions welcome! Please open an issue or PR on GitHub.

## Acknowledgments

Built with:
- [Anthropic Claude](https://www.anthropic.com/) for summarization
- [FastAPI](https://fastapi.tiangolo.com/) for the web dashboard
- [Obsidian](https://obsidian.md/) for the knowledge graph
- [Unpaywall](https://unpaywall.org/) for open access lookup
