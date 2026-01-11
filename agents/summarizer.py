"""Summarizer Agent - Extracts text from PDFs and generates structured summaries."""

import os
import re
from datetime import datetime
from pathlib import Path
from typing import Optional

from anthropic import Anthropic
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
    """Agent that summarizes academic papers using Claude."""

    def __init__(self, vault_path: Path, api_key: Optional[str] = None):
        self.vault_path = Path(vault_path)
        self.api_key = api_key or os.getenv("ANTHROPIC_API_KEY")
        if not self.api_key:
            raise ValueError("ANTHROPIC_API_KEY required for summarization")

        self.client = Anthropic(api_key=self.api_key)

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

        # Generate summary with Claude
        console.print("[blue]Generating summary with Claude...[/blue]")

        try:
            summary_content = self._generate_summary(paper, text)
            citations = self._extract_citations(text)

            # Save summary to vault
            summary_path = self._save_summary(paper, summary_content, citations)

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

    def _generate_summary(self, paper: Paper, text: str) -> str:
        """Use Claude to generate a structured summary."""
        prompt = f"""You are an academic research assistant. Summarize this economics/political science paper.

PAPER METADATA:
Title: {paper.title}
Authors: {paper.authors_str}
Year: {paper.year}
Journal: {paper.journal or 'Working Paper'}

FULL TEXT:
{text}

Please provide a structured summary with these sections:

## Summary
One paragraph (3-5 sentences) explaining what this paper does and finds.

## Key Contributions
3-5 bullet points listing the main contributions of this paper.

## Methodology
Describe the methodology used (theoretical model, empirical analysis, experimental, computational, etc.). 1-2 paragraphs.

## Main Results
3-5 bullet points summarizing the key findings.

## Related Work
List 3-5 of the most important papers this work builds on or relates to, with brief explanations of the connection.

Be concise but thorough. Focus on the economic/scientific contribution, not administrative details."""

        response = self.client.messages.create(
            model="claude-sonnet-4-20250514",
            max_tokens=4000,
            messages=[{"role": "user", "content": prompt}]
        )

        return response.content[0].text

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

    def _save_summary(self, paper: Paper, summary_content: str, citations: list[Citation]) -> Path:
        """Save the summary as a markdown file in the vault."""
        paper_dir = self.vault_path / "papers" / paper.citekey
        paper_dir.mkdir(parents=True, exist_ok=True)

        summary_path = paper_dir / "summary.md"

        # Build frontmatter
        frontmatter = f"""---
title: "{paper.title}"
authors: {paper.authors}
year: {paper.year}
journal: "{paper.journal or 'Working Paper'}"
citekey: "{paper.citekey}"
doi: "{paper.doi or ''}"
status: "summarized"
summarized_at: "{datetime.now().isoformat()}"
pdf_path: "./paper.pdf"
---

"""

        # Add wikilinks to citations for Obsidian
        content = summary_content

        # Add citations section if we extracted any
        if citations:
            content += "\n\n## Extracted Citations\n"
            for cite in citations[:20]:  # Top 20
                content += f"- [[{cite.citekey}]] ({cite.authors}, {cite.year})\n"

        # Add navigation links
        content += "\n\n---\n"
        content += f"PDF: [[paper.pdf]]\n"
        content += f"BibTeX key: `{paper.citekey}`\n"

        with open(summary_path, "w", encoding="utf-8") as f:
            f.write(frontmatter + content)

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
