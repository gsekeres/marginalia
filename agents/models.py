"""Data models for Marginalia papers and metadata."""

from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Optional

from pydantic import BaseModel, Field


class PaperStatus(str, Enum):
    """Status of a paper in the vault."""
    DISCOVERED = "discovered"  # Metadata only, from bib file
    WANTED = "wanted"          # User wants this paper
    QUEUED = "queued"          # In download queue
    DOWNLOADED = "downloaded"  # PDF obtained
    SUMMARIZED = "summarized"  # Summary generated
    FAILED = "failed"          # Download failed, needs manual intervention


class Citation(BaseModel):
    """A citation reference to another paper."""
    citekey: str
    title: Optional[str] = None
    authors: Optional[str] = None
    year: Optional[int] = None
    doi: Optional[str] = None
    status: str = "unknown"  # in_vault, discovered, unknown


class RelatedPaper(BaseModel):
    """A related paper suggested by Claude."""
    title: str
    authors: list[str] = Field(default_factory=list)
    year: Optional[int] = None
    why_related: str = ""
    vault_citekey: Optional[str] = None  # Set if paper exists in vault


class Paper(BaseModel):
    """A paper in the vault."""
    citekey: str
    title: str
    authors: list[str] = Field(default_factory=list)
    year: Optional[int] = None
    journal: Optional[str] = None
    volume: Optional[str] = None
    number: Optional[str] = None
    pages: Optional[str] = None
    doi: Optional[str] = None
    url: Optional[str] = None
    abstract: Optional[str] = None

    # Status tracking
    status: PaperStatus = PaperStatus.DISCOVERED

    # File paths (relative to vault)
    pdf_path: Optional[str] = None
    summary_path: Optional[str] = None

    # Timestamps
    added_at: datetime = Field(default_factory=datetime.now)
    downloaded_at: Optional[datetime] = None
    summarized_at: Optional[datetime] = None

    # Citations (populated after summarization)
    citations: list[Citation] = Field(default_factory=list)
    cited_by: list[str] = Field(default_factory=list)  # citekeys

    # Related papers (Claude-suggested, populated after summarization)
    related_papers: list[RelatedPaper] = Field(default_factory=list)

    # Search metadata
    search_attempts: int = 0
    last_search_error: Optional[str] = None
    manual_download_links: list[str] = Field(default_factory=list)

    @property
    def authors_str(self) -> str:
        """Authors as a comma-separated string."""
        return ", ".join(self.authors)

    @property
    def folder_name(self) -> str:
        """Folder name for this paper in the vault."""
        return self.citekey

    def to_bibtex(self) -> str:
        """Generate BibTeX entry for this paper."""
        lines = [f"@article{{{self.citekey},"]
        if self.title:
            lines.append(f"  title = {{{self.title}}},")
        if self.authors:
            lines.append(f"  author = {{{' and '.join(self.authors)}}},")
        if self.year:
            lines.append(f"  year = {{{self.year}}},")
        if self.journal:
            lines.append(f"  journal = {{{self.journal}}},")
        if self.volume:
            lines.append(f"  volume = {{{self.volume}}},")
        if self.number:
            lines.append(f"  number = {{{self.number}}},")
        if self.pages:
            lines.append(f"  pages = {{{self.pages}}},")
        if self.doi:
            lines.append(f"  doi = {{{self.doi}}},")
        if self.url:
            lines.append(f"  url = {{{self.url}}},")
        lines.append("}")
        return "\n".join(lines)


class VaultIndex(BaseModel):
    """Index of all papers in the vault."""
    papers: dict[str, Paper] = Field(default_factory=dict)  # citekey -> Paper
    last_updated: datetime = Field(default_factory=datetime.now)
    source_bib_path: Optional[str] = None  # Path to the original .bib file

    def add_paper(self, paper: Paper) -> None:
        """Add or update a paper in the index."""
        self.papers[paper.citekey] = paper
        self.last_updated = datetime.now()

    def get_paper(self, citekey: str) -> Optional[Paper]:
        """Get a paper by citekey."""
        return self.papers.get(citekey)

    def get_papers_by_status(self, status: PaperStatus) -> list[Paper]:
        """Get all papers with a given status."""
        return [p for p in self.papers.values() if p.status == status]

    def stats(self) -> dict:
        """Get statistics about the vault."""
        status_counts = {}
        for status in PaperStatus:
            status_counts[status.value] = len(self.get_papers_by_status(status))
        return {
            "total": len(self.papers),
            "by_status": status_counts,
            "last_updated": self.last_updated.isoformat(),
        }


class DownloadResult(BaseModel):
    """Result of a PDF download attempt."""
    success: bool
    citekey: str
    source: Optional[str] = None  # unpaywall, semantic_scholar, nber, etc.
    pdf_path: Optional[str] = None
    error: Optional[str] = None
    manual_links: list[str] = Field(default_factory=list)


class SummaryResult(BaseModel):
    """Result of summarizing a paper."""
    success: bool
    citekey: str
    summary_path: Optional[str] = None
    extracted_citations: list[Citation] = Field(default_factory=list)
    related_papers: list[RelatedPaper] = Field(default_factory=list)
    error: Optional[str] = None
