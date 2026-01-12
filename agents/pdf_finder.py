"""PDF Finder Agent - Searches for and downloads academic paper PDFs."""

import asyncio
import os
import re
import time
from pathlib import Path
from typing import Optional
from urllib.parse import quote_plus, urlparse

import httpx
from anthropic import Anthropic
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
        self.anthropic_key = os.getenv("ANTHROPIC_API_KEY", "")

        # HTTP client with reasonable timeouts
        self.client = httpx.AsyncClient(
            timeout=30.0,
            follow_redirects=True,
            headers={
                "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
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
            ("google_scholar", self._try_google_scholar),
            ("claude_search", self._try_claude_search),
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

    async def _try_google_scholar(self, paper: Paper) -> Optional[str]:
        """Try to find PDF via Google Scholar search."""
        if not paper.authors:
            return None

        # Search for author's website with PDF
        first_author = paper.authors[0]
        last_name = first_author.split()[-1]

        # Try common academic domains
        queries = [
            f'"{paper.title}" filetype:pdf',
            f'{last_name} "{paper.title[:40]}" pdf',
        ]

        for query in queries:
            try:
                # Use DuckDuckGo HTML search (more permissive than Google)
                search_url = f"https://html.duckduckgo.com/html/?q={quote_plus(query)}"
                response = await self.client.get(search_url)

                if response.status_code == 200:
                    # Look for PDF links in the response
                    text = response.text
                    # Find URLs that end in .pdf
                    pdf_urls = re.findall(r'href="(https?://[^"]+\.pdf)"', text, re.IGNORECASE)
                    pdf_urls += re.findall(r'uddg=([^&]+\.pdf)', text)  # DuckDuckGo redirect format

                    for url in pdf_urls[:5]:  # Check first 5 PDF links
                        try:
                            # URL decode if needed
                            from urllib.parse import unquote
                            url = unquote(url)

                            # Verify it's accessible
                            check = await self.client.head(url, timeout=10)
                            if check.status_code == 200:
                                content_type = check.headers.get("content-type", "")
                                if "pdf" in content_type.lower():
                                    return url
                        except Exception:
                            continue
            except Exception as e:
                console.print(f"[yellow]Google Scholar search error: {e}[/yellow]")

        return None

    async def _try_claude_search(self, paper: Paper) -> Optional[str]:
        """Use Claude to help find PDF by generating smart search strategies."""
        if not self.anthropic_key:
            return None

        try:
            client = Anthropic(api_key=self.anthropic_key)

            # Ask Claude to suggest where to find this paper
            prompt = f"""I need to find a PDF of this academic paper:

Title: {paper.title}
Authors: {', '.join(paper.authors) if paper.authors else 'Unknown'}
Year: {paper.year}
Journal: {paper.journal or 'Unknown'}
DOI: {paper.doi or 'Not available'}

Please suggest 3-5 specific URLs where I might find a free PDF of this paper. Consider:
1. Author personal/academic websites (look up author affiliations)
2. Working paper repositories (NBER, SSRN, arXiv, CEPR, IZA, etc.)
3. University repositories
4. Open access versions

Return ONLY a JSON array of URLs to try, no explanation. Example:
["https://example.edu/~author/paper.pdf", "https://ssrn.com/abstract=123456"]

If you cannot suggest specific URLs, return an empty array: []"""

            response = client.messages.create(
                model="claude-sonnet-4-20250514",
                max_tokens=500,
                messages=[{"role": "user", "content": prompt}]
            )

            # Parse the response
            text = response.content[0].text.strip()
            # Extract JSON array from response
            match = re.search(r'\[.*\]', text, re.DOTALL)
            if match:
                import json
                urls = json.loads(match.group())

                for url in urls:
                    try:
                        console.print(f"[dim]Trying Claude-suggested URL: {url[:60]}...[/dim]")
                        # Check if it's a valid PDF
                        check = await self.client.head(url, timeout=10)
                        if check.status_code == 200:
                            content_type = check.headers.get("content-type", "")
                            if "pdf" in content_type.lower():
                                return url
                        # Also try GET in case HEAD doesn't work
                        response = await self.client.get(url, timeout=15)
                        if response.status_code == 200:
                            if response.content[:4] == b"%PDF":
                                return url
                    except Exception:
                        continue

        except Exception as e:
            console.print(f"[yellow]Claude search error: {e}[/yellow]")

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

            # Return path relative to vault (e.g., "papers/citekey/paper.pdf")
            return pdf_path.relative_to(self.vault_path)

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
