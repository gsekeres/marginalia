#!/bin/bash
# Summarize a paper using Claude Code CLI
#
# Usage: ./summarize_paper.sh <citekey>
# Example: ./summarize_paper.sh carnehl2025quest

set -e

CITEKEY="$1"
VAULT_PATH="${VAULT_PATH:-/Users/gabesekeres/Dropbox/Papers/LitVault/vault}"
PAPER_DIR="$VAULT_PATH/papers/$CITEKEY"
PDF_PATH="$PAPER_DIR/paper.pdf"
SUMMARY_PATH="$PAPER_DIR/summary.md"
TEXT_FILE="$PAPER_DIR/_extracted_text.txt"

if [ -z "$CITEKEY" ]; then
    echo "Usage: $0 <citekey>"
    echo "Example: $0 carnehl2025quest"
    exit 1
fi

if [ ! -f "$PDF_PATH" ]; then
    echo "Error: PDF not found at $PDF_PATH"
    exit 1
fi

echo "Extracting text from PDF..."

# Extract text from PDF (first 30 pages, max 50k chars)
python3 -c "
import pdfplumber
with pdfplumber.open('$PDF_PATH') as pdf:
    text = '\n'.join(page.extract_text() or '' for page in pdf.pages[:30])
    print(text[:50000])
" > "$TEXT_FILE"

echo "Extracted text saved to $TEXT_FILE"
echo "Running Claude Code to generate summary..."

# Use Claude Code to summarize
cd "$PAPER_DIR"
claude -p "Read the file $TEXT_FILE which contains extracted text from an academic paper.

Write a structured summary to $SUMMARY_PATH with:
1. A YAML frontmatter block with title, authors, year, and tags (extract from the paper)
2. A one-paragraph abstract summary (## Summary)
3. Key contributions - 3-5 bullet points (## Key Contributions)
4. Methodology - 1-2 paragraphs (## Methodology)
5. Main findings - 3-5 bullet points (## Main Results)
6. Related work - 3-5 important papers referenced (## Related Work)
7. Extracted citations from the text as bullet points (## Extracted Citations)

End with:
---
PDF: [[paper.pdf]]
BibTeX key: \`$CITEKEY\`

Use markdown formatting. Be concise but thorough." \
    --allowedTools "Read,Write" \
    --max-turns 5

# Clean up
rm -f "$TEXT_FILE"

if [ -f "$SUMMARY_PATH" ]; then
    echo "Summary written to $SUMMARY_PATH"
else
    echo "Error: Summary was not created"
    exit 1
fi
