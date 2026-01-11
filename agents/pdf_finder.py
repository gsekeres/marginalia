"""PDF Finder Agent - Searches for and downloads academic paper PDFs."""

import asyncio
import os
import re
import time
from pathlib import Path
from typing import Optional
from urllib.parse import quote_plus

import httpx
from rich.console import Console

from .models import DownloadResult, Paper, PaperStatus

console = Console()

# Rate limiting
LAST_REQUEST_TIME = 0
MIN_REQUEST_INTERVAL = 1.0  # seconds between requests


async def rate_limit():
    """Ensure we don't make requests too quickly."""
    global LAST_REQUEST_TIME
    elapsed = time.time() - LAST_REQUEST_TIME
    if elapsed < MIN_REQUEST_INTERVAL:
        await asyncio.sleep(MIN_REQUEST_INTERVAL - elapsed)
    LAST_REQUEST_TIME = time.time()


class PDFFinder:
    """Agent that finds and downloads PDFs for academic papers."""

    def __init__(
        self,
        vault_path: Path,
        unpaywall_email: Optional[str] = None,
        semantic_scholar_key: Optional[str] = None,
    ):
        self.vault_path = Path(vault_path)
        self.unpaywall_email = unpaywall_email or os.getenv("UNPAYWALL_EMAIL", "")
        self.semantic_scholar_key = semantic_scholar_key or os.getenv("SEMANTIC_SCHOLAR_API_KEY", "")

        # HTTP client with reasonable timeouts
        self.client = httpx.AsyncClient(
            timeout=30.0,
            follow_redirects=True,
            headers={
                "User-Agent": "LitVault/0.1 (Academic Research Tool; mailto:gsekeres@github.com)"
            }
        )

    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()

    async def find_pdf(self, paper: Paper) -> DownloadResult:
        """
        Try to find and download a PDF for the given paper.

        Searches sources in order:
        1. Unpaywall (if DOI available)
        2. Semantic Scholar
        3. NBER (for working papers)
        4. Generate manual search links as fallback
        """
        console.print(f"[blue]Searching for:[/blue] {paper.title[:60]}...")

        # Try each source
        sources = [
            ("unpaywall", self._try_unpaywall),
            ("semantic_scholar", self._try_semantic_scholar),
            ("nber", self._try_nber),
        ]

        for source_name, source_func in sources:
            try:
                await rate_limit()
                pdf_url = await source_func(paper)
                if pdf_url:
                    console.print(f"[green]Found on {source_name}![/green]")
                    # Download the PDF
                    pdf_path = await self._download_pdf(paper, pdf_url, source_name)
                    if pdf_path:
                        return DownloadResult(
                            success=True,
                            citekey=paper.citekey,
                            source=source_name,
                            pdf_path=str(pdf_path),
                        )
            except Exception as e:
                console.print(f"[yellow]Error with {source_name}: {e}[/yellow]")

        # No PDF found - generate manual search links
        manual_links = self._generate_search_links(paper)
        console.print(f"[red]Not found automatically. Generated {len(manual_links)} search links.[/red]")

        return DownloadResult(
            success=False,
            citekey=paper.citekey,
            error="No open access PDF found",
            manual_links=manual_links,
        )

    async def _try_unpaywall(self, paper: Paper) -> Optional[str]:
        """Try to find PDF via Unpaywall API."""
        if not paper.doi:
            return None

        email = self.unpaywall_email or "test@example.com"
        url = f"https://api.unpaywall.org/v2/{paper.doi}?email={email}"

        try:
            response = await self.client.get(url)
            if response.status_code == 200:
                data = response.json()
                # Look for best open access location
                best_oa = data.get("best_oa_location")
                if best_oa:
                    pdf_url = best_oa.get("url_for_pdf") or best_oa.get("url")
                    if pdf_url and pdf_url.endswith(".pdf"):
                        return pdf_url
                    # Sometimes the URL is to a landing page, not direct PDF
                    if pdf_url:
                        return pdf_url
        except Exception:
            pass

        return None

    async def _try_semantic_scholar(self, paper: Paper) -> Optional[str]:
        """Try to find PDF via Semantic Scholar API."""
        # Search by DOI first, then by title
        if paper.doi:
            url = f"https://api.semanticscholar.org/graph/v1/paper/DOI:{paper.doi}?fields=openAccessPdf"
        else:
            # Search by title
            query = quote_plus(paper.title)
            url = f"https://api.semanticscholar.org/graph/v1/paper/search?query={query}&limit=1&fields=openAccessPdf"

        headers = {}
        if self.semantic_scholar_key:
            headers["x-api-key"] = self.semantic_scholar_key

        try:
            response = await self.client.get(url, headers=headers)
            if response.status_code == 200:
                data = response.json()

                # Handle search results
                if "data" in data and data["data"]:
                    data = data["data"][0]

                oa_pdf = data.get("openAccessPdf")
                if oa_pdf and oa_pdf.get("url"):
                    return oa_pdf["url"]
        except Exception:
            pass

        return None

    async def _try_nber(self, paper: Paper) -> Optional[str]:
        """Try to find PDF on NBER (for working papers)."""
        if not paper.authors:
            return None

        # Get first author's last name
        first_author = paper.authors[0]
        last_name = first_author.split()[-1].lower()

        # Search NBER
        query = quote_plus(f"{last_name} {paper.title[:50]}")
        search_url = f"https://www.nber.org/api/search?q={query}"

        try:
            response = await self.client.get(search_url)
            if response.status_code == 200:
                data = response.json()
                results = data.get("results", [])
                for result in results[:3]:  # Check first 3 results
                    if "working_paper" in result.get("type", "").lower():
                        paper_id = result.get("id")
                        if paper_id:
                            # NBER PDF URL pattern
                            pdf_url = f"https://www.nber.org/system/files/working_papers/w{paper_id}/w{paper_id}.pdf"
                            # Verify it exists
                            check = await self.client.head(pdf_url)
                            if check.status_code == 200:
                                return pdf_url
        except Exception:
            pass

        return None

    async def _download_pdf(self, paper: Paper, url: str, source: str) -> Optional[Path]:
        """Download a PDF to the vault."""
        try:
            response = await self.client.get(url)
            if response.status_code != 200:
                return None

            # Verify it's actually a PDF
            content_type = response.headers.get("content-type", "")
            if "pdf" not in content_type.lower() and not response.content[:4] == b"%PDF":
                return None

            # Create paper directory
            paper_dir = self.vault_path / "papers" / paper.citekey
            paper_dir.mkdir(parents=True, exist_ok=True)

            # Save PDF
            pdf_path = paper_dir / "paper.pdf"
            with open(pdf_path, "wb") as f:
                f.write(response.content)

            return pdf_path

        except Exception as e:
            console.print(f"[red]Download error: {e}[/red]")
            return None

    def _generate_search_links(self, paper: Paper) -> list[str]:
        """Generate manual search links for the user."""
        links = []

        # Title search
        title_query = quote_plus(paper.title)

        # Google Scholar
        links.append(f"https://scholar.google.com/scholar?q={title_query}")

        # Author + title search
        if paper.authors:
            author_query = quote_plus(f"{paper.authors[0]} {paper.title[:50]}")
            links.append(f"https://scholar.google.com/scholar?q={author_query}")

        # SSRN
        links.append(f"https://papers.ssrn.com/sol3/results.cfm?txtKey_Words={title_query}")

        # NBER (if economics-related)
        if paper.authors:
            nber_query = quote_plus(paper.authors[0].split()[-1])
            links.append(f"https://www.nber.org/search?q={nber_query}")

        # DOI link if available
        if paper.doi:
            links.append(f"https://doi.org/{paper.doi}")

        return links


async def find_pdfs_batch(
    papers: list[Paper],
    vault_path: Path,
    limit: Optional[int] = None,
) -> list[DownloadResult]:
    """Find PDFs for a batch of papers."""
    finder = PDFFinder(vault_path)

    results = []
    papers_to_process = papers[:limit] if limit else papers

    for i, paper in enumerate(papers_to_process):
        console.print(f"\n[bold]({i+1}/{len(papers_to_process)})[/bold]")
        result = await finder.find_pdf(paper)
        results.append(result)

        # Update paper status
        if result.success:
            paper.status = PaperStatus.DOWNLOADED
            paper.pdf_path = result.pdf_path
        else:
            paper.manual_download_links = result.manual_links
            paper.search_attempts += 1

    await finder.close()
    return results


if __name__ == "__main__":
    # Test the finder
    import sys

    test_paper = Paper(
        citekey="test2024",
        title="Artificial Intelligence, Algorithmic Pricing, and Collusion",
        authors=["Emilio Calvano", "Giacomo Calzolari"],
        year=2020,
        doi="10.1257/aer.20190623",
    )

    async def test():
        vault = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("./vault")
        finder = PDFFinder(vault)
        result = await finder.find_pdf(test_paper)
        print(f"\nResult: {result}")
        await finder.close()

    asyncio.run(test())
