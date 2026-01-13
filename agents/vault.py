"""Vault Manager - Handles the Obsidian-compatible vault structure for Marginalia."""

import json
from datetime import datetime
from pathlib import Path
from typing import Optional

from rich.console import Console
from rich.table import Table

from .bibtex_parser import parse_bibtex_file, export_to_bibtex
from .models import Paper, PaperStatus, VaultIndex, PaperNotes

console = Console()

INDEX_FILENAME = ".marginalia_index.json"


def generate_citekey(authors: list[str], year: Optional[int], title: str) -> str:
    """Generate a citekey from paper metadata.

    Format: firstauthorlastname + year + firstwordoftitle
    Example: "smith2023algorithmic"
    """
    import re

    # Get first author's last name
    if authors and len(authors) > 0:
        first_author = authors[0]
        # Take last word as last name (handles "John Smith" and "Smith, John")
        parts = first_author.replace(",", "").split()
        last_name = parts[-1] if parts else "unknown"
    else:
        last_name = "unknown"

    # Clean last name (lowercase, alphanumeric only)
    last_name = re.sub(r'[^a-z]', '', last_name.lower())

    # Get year or use 0000
    year_str = str(year) if year else "0000"

    # Get first significant word of title (skip articles)
    skip_words = {"a", "an", "the", "on", "in", "of", "and", "for", "to", "with"}
    title_words = re.sub(r'[^a-z\s]', '', title.lower()).split()
    first_word = "paper"
    for word in title_words:
        if word not in skip_words and len(word) > 2:
            first_word = word
            break

    return f"{last_name}{year_str}{first_word}"


class VaultManager:
    """Manages the paper vault and its index for Marginalia."""

    def __init__(self, vault_path: Path):
        self.vault_path = Path(vault_path)
        self.papers_path = self.vault_path / "papers"
        self.index_path = self.vault_path / INDEX_FILENAME
        self.index: VaultIndex = VaultIndex()

        # Ensure directories exist
        self.vault_path.mkdir(parents=True, exist_ok=True)
        self.papers_path.mkdir(parents=True, exist_ok=True)

        # Load existing index if it exists
        self.load_index()

    def load_index(self) -> None:
        """Load the vault index from disk."""
        if self.index_path.exists():
            with open(self.index_path, "r") as f:
                data = json.load(f)
                self.index = VaultIndex.model_validate(data)
            console.print(f"[green]Loaded {len(self.index.papers)} papers from index[/green]")
        else:
            console.print("[yellow]No existing index found, starting fresh[/yellow]")

    def save_index(self) -> None:
        """Save the vault index to disk."""
        self.index.last_updated = datetime.now()
        with open(self.index_path, "w") as f:
            json.dump(self.index.model_dump(mode="json"), f, indent=2, default=str)

    def import_bibtex(self, bib_path: Path) -> int:
        """Import papers from a BibTeX file."""
        console.print(f"[blue]Importing from {bib_path}...[/blue]")

        imported_index = parse_bibtex_file(bib_path)

        added = 0
        for citekey, paper in imported_index.papers.items():
            if citekey not in self.index.papers:
                self.index.add_paper(paper)
                added += 1
            else:
                # Update existing paper's metadata if it was only discovered
                existing = self.index.papers[citekey]
                if existing.status == PaperStatus.DISCOVERED:
                    # Keep status but update other fields
                    paper.status = existing.status
                    paper.pdf_path = existing.pdf_path
                    paper.summary_path = existing.summary_path
                    self.index.add_paper(paper)

        self.save_index()
        console.print(f"[green]Added {added} new papers[/green]")
        return added

    def get_paper(self, citekey: str) -> Optional[Paper]:
        """Get a paper by citekey."""
        return self.index.get_paper(citekey)

    def add_paper(self, paper: Paper) -> None:
        """Add a paper to the vault and save."""
        self.index.add_paper(paper)
        self.save_index()

    def set_paper_status(self, citekey: str, status: PaperStatus) -> bool:
        """Update a paper's status."""
        paper = self.get_paper(citekey)
        if paper:
            paper.status = status
            self.save_index()
            return True
        return False

    def mark_wanted(self, citekeys: list[str]) -> int:
        """Mark papers as wanted (to be downloaded)."""
        count = 0
        for citekey in citekeys:
            paper = self.get_paper(citekey)
            if paper and paper.status == PaperStatus.DISCOVERED:
                paper.status = PaperStatus.WANTED
                count += 1
        self.save_index()
        return count

    def get_wanted_papers(self) -> list[Paper]:
        """Get all papers marked as wanted."""
        return self.index.get_papers_by_status(PaperStatus.WANTED)

    def get_downloaded_papers(self) -> list[Paper]:
        """Get all papers that have been downloaded."""
        return self.index.get_papers_by_status(PaperStatus.DOWNLOADED)

    def get_papers_needing_manual_download(self) -> list[Paper]:
        """Get papers that need manual download intervention."""
        return [
            p for p in self.index.papers.values()
            if p.status == PaperStatus.WANTED and p.search_attempts > 0
        ]

    def search_papers(self, query: str) -> list[Paper]:
        """Search papers by title, author, or citekey."""
        query = query.lower()
        results = []
        for paper in self.index.papers.values():
            if (query in paper.title.lower() or
                query in paper.citekey.lower() or
                any(query in a.lower() for a in paper.authors)):
                results.append(paper)
        return results

    def create_paper_folder(self, paper: Paper) -> Path:
        """Create the folder structure for a paper."""
        paper_dir = self.papers_path / paper.citekey
        paper_dir.mkdir(parents=True, exist_ok=True)
        return paper_dir

    def register_pdf(self, citekey: str, pdf_path: Path) -> bool:
        """Register a manually downloaded PDF."""
        paper = self.get_paper(citekey)
        if not paper:
            return False

        # Create paper folder and copy/move PDF
        paper_dir = self.create_paper_folder(paper)
        dest_path = paper_dir / "paper.pdf"

        if pdf_path != dest_path:
            import shutil
            shutil.copy2(pdf_path, dest_path)

        paper.pdf_path = str(dest_path.relative_to(self.vault_path))
        paper.status = PaperStatus.DOWNLOADED
        paper.downloaded_at = datetime.now()
        self.save_index()

        return True

    def generate_index_page(self) -> Path:
        """Generate a main index.md page for Obsidian."""
        index_md = self.vault_path / "index.md"

        content = """---
title: Marginalia Index
---

# Literature Vault

## Statistics
"""
        stats = self.index.stats()
        content += f"- **Total papers:** {stats['total']}\n"
        for status, count in stats['by_status'].items():
            content += f"- **{status}:** {count}\n"

        content += f"\n*Last updated: {stats['last_updated']}*\n"

        # Papers by status
        content += "\n## By Status\n"

        for status in [PaperStatus.SUMMARIZED, PaperStatus.DOWNLOADED, PaperStatus.WANTED, PaperStatus.DISCOVERED]:
            papers = self.index.get_papers_by_status(status)
            if papers:
                content += f"\n### {status.value.title()} ({len(papers)})\n"
                for paper in sorted(papers, key=lambda p: p.year or 0, reverse=True)[:20]:
                    content += f"- [[{paper.citekey}|{paper.title[:60]}]] ({paper.year})\n"
                if len(papers) > 20:
                    content += f"- *... and {len(papers) - 20} more*\n"

        # Manual download queue
        manual = self.get_papers_needing_manual_download()
        if manual:
            content += "\n## Manual Download Queue\n"
            content += "These papers need manual downloading:\n\n"
            for paper in manual:
                content += f"### {paper.citekey}\n"
                content += f"**{paper.title}** ({paper.authors_str}, {paper.year})\n\n"
                if paper.manual_download_links:
                    content += "Search links:\n"
                    for link in paper.manual_download_links[:3]:
                        content += f"- [{link[:50]}...]({link})\n"
                content += "\n"

        with open(index_md, "w") as f:
            f.write(content)

        return index_md

    def export_bibtex(self, output_path: Optional[Path] = None) -> Path:
        """Export all papers to BibTeX format."""
        output_path = output_path or (self.vault_path / "references.bib")
        export_to_bibtex(self.index, output_path)
        return output_path

    def get_paper_notes(self, citekey: str) -> PaperNotes:
        """Get notes and highlights for a paper, creating if needed."""
        notes_path = self.papers_path / citekey / "notes.json"
        if notes_path.exists():
            with open(notes_path, "r") as f:
                data = json.load(f)
                return PaperNotes.model_validate(data)
        # Return empty notes object
        return PaperNotes(citekey=citekey)

    def save_paper_notes(self, notes: PaperNotes) -> None:
        """Save notes and highlights for a paper."""
        paper_dir = self.papers_path / notes.citekey
        paper_dir.mkdir(parents=True, exist_ok=True)
        notes_path = paper_dir / "notes.json"
        notes.last_modified = datetime.now()
        with open(notes_path, "w") as f:
            json.dump(notes.model_dump(mode="json"), f, indent=2, default=str)

    def print_stats(self) -> None:
        """Print vault statistics."""
        stats = self.index.stats()

        table = Table(title="Marginalia Statistics")
        table.add_column("Status", style="cyan")
        table.add_column("Count", justify="right", style="green")

        for status, count in stats['by_status'].items():
            table.add_row(status, str(count))

        table.add_row("─" * 15, "─" * 5, style="dim")
        table.add_row("Total", str(stats['total']), style="bold")

        console.print(table)


if __name__ == "__main__":
    import sys

    vault_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("./vault")
    manager = VaultManager(vault_path)
    manager.print_stats()
