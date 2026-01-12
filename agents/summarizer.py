"""Summarizer Agent - Extracts text from PDFs and generates structured summaries."""

import os
import re
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Optional

from rich.console import Console

from .models import Citation, Paper, PaperStatus, SummaryResult

console = Console()


def extract_text_from_pdf(pdf_path: Path) -> str:
    """Extract text from a PDF file using pdfplumber or PyMuPDF."""
    text = ""

    # Try pdfplumber first (better for academic papers)
    try:
        import pdfplumber
        with pdfplumber.open(pdf_path) as pdf:
            for page in pdf.pages:
                page_text = page.extract_text()
                if page_text:
                    text += page_text + "\n\n"
        if text.strip():
            return text
    except Exception as e:
        console.print(f"[yellow]pdfplumber failed: {e}[/yellow]")

    # Fall back to PyMuPDF
    try:
        import fitz  # PyMuPDF
        doc = fitz.open(pdf_path)
        for page in doc:
            text += page.get_text() + "\n\n"
        doc.close()
        if text.strip():
            return text
    except Exception as e:
        console.print(f"[yellow]PyMuPDF failed: {e}[/yellow]")

    return text


def clean_extracted_text(text: str) -> str:
    """Clean up extracted text for better processing."""
    # Remove excessive whitespace
    text = re.sub(r'\n{3,}', '\n\n', text)
    text = re.sub(r' {2,}', ' ', text)

    # Remove page numbers (common patterns)
    text = re.sub(r'\n\d+\n', '\n', text)

    # Remove common headers/footers
    text = re.sub(r'(Electronic copy available at|Downloaded from|https?://\S+)', '', text)

    return text.strip()


class Summarizer:
    """Agent that summarizes academic papers using Claude Code CLI."""

    def __init__(self, vault_path: Path, api_key: Optional[str] = None):
        self.vault_path = Path(vault_path)
        # API key no longer required - we use Claude Code CLI instead

    def summarize(self, paper: Paper) -> SummaryResult:
        """Generate a structured summary for a paper."""
        if not paper.pdf_path:
            return SummaryResult(
                success=False,
                citekey=paper.citekey,
                error="No PDF path available",
            )

        pdf_path = self.vault_path / "papers" / paper.citekey / "paper.pdf"
        if not pdf_path.exists():
            # Try the stored path
            pdf_path = Path(paper.pdf_path)
            if not pdf_path.exists():
                return SummaryResult(
                    success=False,
                    citekey=paper.citekey,
                    error=f"PDF not found: {paper.pdf_path}",
                )

        console.print(f"[blue]Extracting text from:[/blue] {pdf_path.name}")

        # Extract text
        raw_text = extract_text_from_pdf(pdf_path)
        if not raw_text:
            return SummaryResult(
                success=False,
                citekey=paper.citekey,
                error="Could not extract text from PDF",
            )

        text = clean_extracted_text(raw_text)
        console.print(f"[green]Extracted {len(text):,} characters[/green]")

        # Truncate if too long (Claude has context limits)
        max_chars = 100000
        if len(text) > max_chars:
            text = text[:max_chars] + "\n\n[TRUNCATED]"

        # Generate summary with Claude Code CLI
        console.print("[blue]Generating summary with Claude Code...[/blue]")

        try:
            # Define summary path
            paper_dir = self.vault_path / "papers" / paper.citekey
            paper_dir.mkdir(parents=True, exist_ok=True)
            summary_path = paper_dir / "summary.md"

            # Generate summary (Claude Code writes directly to file)
            summary_content = self._generate_summary(paper, text, summary_path)

            # Extract citations from the generated summary
            citations = self._extract_citations(summary_content)

            return SummaryResult(
                success=True,
                citekey=paper.citekey,
                summary_path=str(summary_path),
                extracted_citations=citations,
            )

        except Exception as e:
            return SummaryResult(
                success=False,
                citekey=paper.citekey,
                error=str(e),
            )

    def _generate_summary(self, paper: Paper, text: str, summary_path: Path) -> str:
        """Use Claude Code CLI to generate a structured summary."""
        import json as json_module
        paper_dir = self.vault_path / "papers" / paper.citekey

        console.print("[blue]Running Claude Code for summarization...[/blue]")

        try:
            # Ask Claude to output JSON that we'll parse into markdown
            json_prompt = f"""Summarize this academic paper and output a JSON object.

PAPER METADATA:
Title: {paper.title}
Authors: {paper.authors_str}
Year: {paper.year}
Journal: {paper.journal or 'Working Paper'}

EXTRACTED TEXT:
{text[:30000]}

Output a JSON object with these exact keys:
{{
  "summary": "One paragraph summary of what the paper does and finds",
  "key_contributions": ["contribution 1", "contribution 2", ...],
  "methodology": "Description of methodology used",
  "main_results": ["result 1", "result 2", ...],
  "related_work": ["paper 1 - brief description", "paper 2 - brief description", ...]
}}

Output ONLY valid JSON, no other text."""

            result = subprocess.run(
                ["claude", "-p", json_prompt, "--output-format", "json"],
                capture_output=True,
                text=True,
                timeout=180,
                cwd=str(paper_dir),
                env=os.environ.copy(),
            )

            console.print(f"[dim]Claude Code return code: {result.returncode}[/dim]")
            if result.stderr:
                console.print(f"[yellow]Claude Code stderr: {result.stderr[:500]}[/yellow]")

            if result.returncode != 0:
                error_details = result.stderr or result.stdout or "No output"
                raise Exception(f"Claude Code failed (exit {result.returncode}): {error_details[:500]}")

            # Parse the JSON response
            raw_output = result.stdout.strip()
            if not raw_output:
                raise Exception("Claude Code returned empty output")

            # The output format is JSON with a "result" field containing the actual response
            try:
                response_json = json_module.loads(raw_output)
                # Extract the result field which contains Claude's response
                claude_response = response_json.get("result", raw_output)

                # Try to parse Claude's response as JSON
                # It might be a string containing JSON, so we need to parse it
                if isinstance(claude_response, str):
                    # Find JSON in the response (it might have extra text)
                    json_match = re.search(r'\{[\s\S]*\}', claude_response)
                    if json_match:
                        summary_data = json_module.loads(json_match.group())
                    else:
                        raise Exception("Could not find JSON in Claude's response")
                else:
                    summary_data = claude_response
            except json_module.JSONDecodeError as e:
                console.print(f"[yellow]JSON parse error, trying to extract: {e}[/yellow]")
                # Try to find JSON in raw output
                json_match = re.search(r'\{[\s\S]*\}', raw_output)
                if json_match:
                    summary_data = json_module.loads(json_match.group())
                else:
                    raise Exception(f"Could not parse JSON from Claude's response: {raw_output[:500]}")

            # Build markdown from the parsed JSON
            markdown = f"""---
title: "{paper.title}"
authors: {paper.authors}
year: {paper.year}
journal: "{paper.journal or 'Working Paper'}"
citekey: "{paper.citekey}"
doi: "{paper.doi or ''}"
status: "summarized"
pdf_path: "./paper.pdf"
---

## Summary

{summary_data.get('summary', 'No summary available.')}

## Key Contributions

"""
            for contrib in summary_data.get('key_contributions', []):
                markdown += f"- {contrib}\n"

            markdown += f"""
## Methodology

{summary_data.get('methodology', 'No methodology description available.')}

## Main Results

"""
            for result_item in summary_data.get('main_results', []):
                markdown += f"- {result_item}\n"

            markdown += """
## Related Work

"""
            for work in summary_data.get('related_work', []):
                markdown += f"- {work}\n"

            markdown += f"""
---
PDF: [[paper.pdf]]
BibTeX key: `{paper.citekey}`
"""

            # Write the summary file
            with open(summary_path, "w", encoding="utf-8") as f:
                f.write(markdown)

            console.print(f"[green]Summary written to {summary_path}[/green]")
            return markdown

        except subprocess.TimeoutExpired:
            raise Exception("Claude Code timed out after 3 minutes")
        except FileNotFoundError:
            raise Exception(
                "Claude Code CLI not found. Install it with: npm install -g @anthropic-ai/claude-code "
                "(or use the native installer: curl -fsSL https://claude.ai/install.sh | bash). "
                "See: https://code.claude.com/docs/en/setup"
            )

    def _extract_citations(self, text: str) -> list[Citation]:
        """Extract citations from the paper text."""
        citations = []

        # Pattern for common citation formats: (Author, Year) or (Author et al., Year)
        patterns = [
            r'\(([A-Z][a-z]+(?:\s+(?:and|&)\s+[A-Z][a-z]+)?(?:\s+et\s+al\.)?),?\s*(\d{4})\)',
            r'\(([A-Z][a-z]+(?:\s+(?:and|&)\s+[A-Z][a-z]+)*),?\s*(\d{4})\)',
            r'([A-Z][a-z]+(?:\s+(?:and|&)\s+[A-Z][a-z]+)?(?:\s+et\s+al\.)?)\s*\((\d{4})\)',
        ]

        found = set()
        for pattern in patterns:
            matches = re.findall(pattern, text)
            for match in matches:
                author, year = match
                key = f"{author.lower().replace(' ', '')}_{year}"
                if key not in found:
                    found.add(key)
                    # Generate a citekey
                    last_name = author.split()[0].lower()
                    citekey = f"{last_name}{year}"

                    citations.append(Citation(
                        citekey=citekey,
                        authors=author,
                        year=int(year) if year.isdigit() else None,
                        status="unknown",
                    ))

        return citations[:50]  # Limit to top 50

    # _save_summary removed - Claude Code now writes the summary directly

        # Also save the full extracted text
        text_path = paper_dir / "full_text.md"
        # We don't have the text here, but the caller can save it

        return summary_path


def summarize_batch(
    papers: list[Paper],
    vault_path: Path,
    limit: Optional[int] = None,
) -> list[SummaryResult]:
    """Summarize a batch of papers."""
    summarizer = Summarizer(vault_path)

    results = []
    papers_to_process = [p for p in papers if p.status == PaperStatus.DOWNLOADED]
    if limit:
        papers_to_process = papers_to_process[:limit]

    for i, paper in enumerate(papers_to_process):
        console.print(f"\n[bold]({i+1}/{len(papers_to_process)})[/bold] {paper.citekey}")
        result = summarizer.summarize(paper)
        results.append(result)

        if result.success:
            paper.status = PaperStatus.SUMMARIZED
            paper.summary_path = result.summary_path
            paper.summarized_at = datetime.now()
            paper.citations = result.extracted_citations
            console.print(f"[green]Summarized! Found {len(result.extracted_citations)} citations.[/green]")
        else:
            console.print(f"[red]Failed: {result.error}[/red]")

    return results


if __name__ == "__main__":
    # Test summarization
    import sys

    if len(sys.argv) < 2:
        print("Usage: python summarizer.py <vault_path> [citekey]")
        sys.exit(1)

    vault = Path(sys.argv[1])

    # Test with a specific paper
    test_paper = Paper(
        citekey=sys.argv[2] if len(sys.argv) > 2 else "test",
        title="Test Paper",
        authors=["Test Author"],
        year=2024,
        pdf_path="paper.pdf",
        status=PaperStatus.DOWNLOADED,
    )

    summarizer = Summarizer(vault)
    result = summarizer.summarize(test_paper)
    print(f"\nResult: {result}")
