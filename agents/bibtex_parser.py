"""BibTeX parsing and paper extraction."""

import re
from pathlib import Path
from typing import Optional

import bibtexparser
from bibtexparser.bparser import BibTexParser
from bibtexparser.customization import convert_to_unicode

from .models import Paper, PaperStatus, VaultIndex


def parse_authors(author_str: str) -> list[str]:
    """Parse BibTeX author string into list of names."""
    if not author_str:
        return []

    # Split on " and " (BibTeX convention)
    authors = re.split(r'\s+and\s+', author_str)

    cleaned = []
    for author in authors:
        # Clean up whitespace and formatting
        author = author.strip()
        author = re.sub(r'\s+', ' ', author)

        # Handle "Last, First" format -> "First Last"
        if ',' in author:
            parts = author.split(',', 1)
            if len(parts) == 2:
                author = f"{parts[1].strip()} {parts[0].strip()}"

        if author:
            cleaned.append(author)

    return cleaned


def parse_year(year_str: str) -> Optional[int]:
    """Extract year as integer from various formats."""
    if not year_str:
        return None

    # Handle "forthcoming", "R&R", etc.
    if not any(c.isdigit() for c in year_str):
        return None

    # Extract first 4-digit year
    match = re.search(r'(\d{4})', year_str)
    if match:
        return int(match.group(1))

    return None


def entry_to_paper(entry: dict) -> Paper:
    """Convert a bibtexparser entry to a Paper model."""
    # Get citekey (ID)
    citekey = entry.get('ID', 'unknown')

    # Parse authors
    authors = parse_authors(entry.get('author', ''))

    # Parse year
    year = parse_year(entry.get('year', ''))

    # Clean title (remove braces used for capitalization)
    title = entry.get('title', 'Untitled')
    title = re.sub(r'[{}]', '', title)

    # Extract DOI
    doi = entry.get('doi', '')
    if doi:
        # Clean DOI format
        doi = doi.replace('https://doi.org/', '')
        doi = doi.replace('http://doi.org/', '')

    return Paper(
        citekey=citekey,
        title=title,
        authors=authors,
        year=year,
        journal=entry.get('journal', ''),
        volume=entry.get('volume', ''),
        number=entry.get('number', ''),
        pages=entry.get('pages', ''),
        doi=doi if doi else None,
        url=entry.get('url', '') or None,
        abstract=entry.get('abstract', '') or None,
        status=PaperStatus.DISCOVERED,
    )


def parse_bibtex_file(filepath: Path) -> VaultIndex:
    """Parse a BibTeX file and return a VaultIndex."""
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()

    # Configure parser
    parser = BibTexParser(common_strings=True)
    parser.customization = convert_to_unicode

    # Parse
    bib_database = bibtexparser.loads(content, parser=parser)

    # Convert to papers
    index = VaultIndex()
    for entry in bib_database.entries:
        try:
            paper = entry_to_paper(entry)
            index.add_paper(paper)
        except Exception as e:
            print(f"Warning: Could not parse entry {entry.get('ID', 'unknown')}: {e}")

    return index


def export_to_bibtex(index: VaultIndex, filepath: Path) -> None:
    """Export vault index back to BibTeX format."""
    with open(filepath, 'w', encoding='utf-8') as f:
        for paper in index.papers.values():
            f.write(paper.to_bibtex())
            f.write('\n\n')


if __name__ == "__main__":
    # Test parsing
    import sys
    if len(sys.argv) > 1:
        bib_path = Path(sys.argv[1])
        index = parse_bibtex_file(bib_path)
        stats = index.stats()
        print(f"Parsed {stats['total']} papers")

        # Show first 5
        for i, paper in enumerate(list(index.papers.values())[:5]):
            print(f"\n{paper.citekey}:")
            print(f"  Title: {paper.title}")
            print(f"  Authors: {paper.authors_str}")
            print(f"  Year: {paper.year}")
            print(f"  DOI: {paper.doi}")
