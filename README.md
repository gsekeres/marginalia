# Marginalia

A native macOS academic literature management app that uses AI to organize your research library.

## What It Does

Marginalia automates the tedious parts of building a research library:

1. **Find PDFs** - Searches Unpaywall, Semantic Scholar, and other open-access sources
2. **Summarize Papers** - Extracts text and generates structured summaries using Claude
3. **Build Citation Graphs** - Links papers together through their citations
4. **Obsidian Integration** - All summaries are Obsidian-compatible markdown with wikilinks

**Key Feature**: Uses your existing Claude Pro/Max subscription via Claude Code CLI - no separate API credits needed.

## Installation

### 1. Download

Download the latest release from the [Releases](https://github.com/yourusername/marginalia/releases) page.

### 2. Set Up Claude Code CLI

Summarization requires Claude Code CLI with your subscription:

```bash
# Install Claude Code CLI
brew install anthropics/tap/claude

# Authenticate (opens browser)
claude login
```

### 3. Run

Open Marginalia.app and select or create a vault folder.

## Usage

1. **Import** - Drop a BibTeX file or paste citation keys
2. **Find PDFs** - Click "Find PDFs" to search open-access sources
3. **Summarize** - Click "Summarize" to generate structured summaries
4. **Explore** - Use the graph view to explore citation relationships
5. **Take Notes** - Add highlights and annotations to PDFs
6. **Export** - Open the vault folder in Obsidian for linked notes

## Development

### Prerequisites

- Rust (latest stable)
- Node.js 18+
- Claude Code CLI (for summarization)

### Build from Source

```bash
cd marginalia-app

# Install frontend dependencies
npm install

# Run development server
npm run tauri dev

# Build for production
npm run tauri build
```

### Project Structure

```
marginalia-app/
├── src/                    # TypeScript frontend
│   ├── api/               # Tauri command wrappers
│   ├── state/             # State management
│   ├── views/             # View helpers
│   ├── components/        # UI components
│   └── types.ts           # Type definitions
├── src-tauri/             # Rust backend
│   └── src/
│       ├── adapters/      # External API clients
│       ├── commands/      # Tauri commands
│       ├── models/        # Data structures
│       ├── services/      # Business logic
│       └── storage/       # SQLite database
└── package.json
```

## How PDF Finding Works

The PDF Finder searches these sources in order:
1. **Unpaywall** - Open-access versions via DOI
2. **Semantic Scholar** - Academic search with open-access PDFs
3. **Claude** - AI-powered web search (if available)
4. **Manual Queue** - Generates search links if not found

Expected success rate: ~70-85% for papers with DOIs.

## How Summarization Works

1. Extracts text from PDF
2. Sends to Claude with structured JSON output format
3. Validates and parses response
4. Generates markdown summary with:
   - Overview paragraph
   - Key contributions
   - Methodology
   - Main results
   - Related work with wikilinks

## Configuration

Configuration is managed through the Settings panel in the app:
- Default vault location
- Claude model preferences
- Auto-find/summarize options

## Logs & Diagnostics

Logs are stored in `~/Library/Application Support/com.marginalia.app/logs/`.

Access diagnostics via Settings → Diagnostics to check:
- Database status
- Claude CLI availability
- Network connectivity

## License

MIT License

## Contributing

Contributions welcome! Please open an issue or PR on GitHub.
