# Marginalia - Obsidian Vault

This folder is your Obsidian-compatible literature vault.

## Structure

```
vault/
├── papers/           # One folder per paper
│   └── [citekey]/
│       ├── paper.pdf
│       ├── summary.md
│       └── full_text.md
├── templates/        # Obsidian templates
├── index.md          # Auto-generated index
└── .obsidian/        # Obsidian settings
```

## Opening in Obsidian

1. Open Obsidian
2. Click "Open folder as vault"
3. Select this `vault` folder
4. The papers will appear as linked notes

## Wikilinks

Papers link to each other using `[[citekey]]` syntax:
- `[[calvano2020aicollusion]]` links to that paper's summary
- Obsidian shows a graph of all connections
- Click any link to navigate

## Templates

Use the `paper_summary.md` template when manually creating summaries.
