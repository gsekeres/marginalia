"""FastAPI web dashboard for Marginalia."""

import asyncio
import os
from pathlib import Path
from typing import Optional

from dotenv import load_dotenv
load_dotenv()  # Load .env file

from fastapi import FastAPI, HTTPException, UploadFile, File, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel

from .models import Paper, PaperStatus, RelatedPaper
from .pdf_finder import PDFFinder
from .summarizer import Summarizer
from .vault import VaultManager, generate_citekey

app = FastAPI(
    title="Marginalia",
    description="Agent-based academic literature management",
    version="0.1.0",
)

# CORS for local development and production
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:8000",
        "http://127.0.0.1:8000",
        "http://localhost:1313",  # Hugo dev server
        "https://gabesekeres.com",
        "https://www.gabesekeres.com",
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Global vault manager
vault_path = Path(os.getenv("VAULT_PATH", "./vault"))
vault = VaultManager(vault_path)

# Job tracking
active_jobs: dict[str, dict] = {}


# Request/Response models
class MarkWantedRequest(BaseModel):
    citekeys: list[str]


class SearchRequest(BaseModel):
    query: str


class RegisterPDFRequest(BaseModel):
    citekey: str


class ImportPathRequest(BaseModel):
    file_path: str


class AddRelatedPaperRequest(BaseModel):
    title: str
    authors: list[str]
    year: Optional[int] = None
    source_citekey: str  # Paper this was related to


def normalize_title(title: str) -> str:
    """Normalize a title for comparison."""
    import re
    return re.sub(r'[^a-z0-9\s]', '', title.lower()).strip()


def match_related_papers_to_vault(paper: Paper) -> Paper:
    """Update related papers with vault_citekey if they exist in vault."""
    for related in paper.related_papers:
        related_title_norm = normalize_title(related.title)
        for vault_paper in vault.index.papers.values():
            vault_title_norm = normalize_title(vault_paper.title)
            # Check for title similarity
            if (related_title_norm in vault_title_norm or
                vault_title_norm in related_title_norm or
                related_title_norm == vault_title_norm):
                related.vault_citekey = vault_paper.citekey
                break
    return paper


# Routes

@app.get("/")
async def root():
    """Serve the dashboard."""
    return FileResponse(Path(__file__).parent.parent / "app" / "index.html")


@app.get("/api/stats")
async def get_stats():
    """Get vault statistics."""
    return vault.index.stats()


@app.get("/api/papers")
async def get_papers(
    status: Optional[str] = None,
    search: Optional[str] = None,
    limit: int = 100,
    offset: int = 0,
):
    """Get papers with optional filtering."""
    if search:
        papers = vault.search_papers(search)
    elif status:
        papers = vault.index.get_papers_by_status(PaperStatus(status))
    else:
        papers = list(vault.index.papers.values())

    # Sort by year descending
    papers.sort(key=lambda p: p.year or 0, reverse=True)

    return {
        "total": len(papers),
        "papers": [p.model_dump() for p in papers[offset:offset + limit]],
    }


@app.get("/api/papers/{citekey}")
async def get_paper(citekey: str):
    """Get a specific paper with related papers matched to vault."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")
    # Match related papers to vault
    paper = match_related_papers_to_vault(paper)
    return paper.model_dump()


@app.post("/api/papers/mark-wanted")
async def mark_wanted(request: MarkWantedRequest):
    """Mark papers as wanted for download."""
    count = vault.mark_wanted(request.citekeys)
    return {"marked": count}


@app.post("/api/papers/add-related")
async def add_related_paper(request: AddRelatedPaperRequest):
    """Add a related paper to the vault as 'discovered'."""
    # Generate citekey from metadata
    citekey = generate_citekey(request.authors, request.year, request.title)

    # Check if already exists
    existing = vault.get_paper(citekey)
    if existing:
        return {"status": "exists", "citekey": citekey}

    # Create new paper
    paper = Paper(
        citekey=citekey,
        title=request.title,
        authors=request.authors,
        year=request.year,
        status=PaperStatus.DISCOVERED,
    )
    vault.add_paper(paper)
    return {"status": "added", "citekey": citekey}


@app.post("/api/papers/{citekey}/want")
async def want_paper(citekey: str):
    """Mark a single paper as wanted."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    paper.status = PaperStatus.WANTED
    vault.save_index()
    return {"status": "wanted"}


@app.post("/api/find-pdfs")
async def find_pdfs(background_tasks: BackgroundTasks, limit: Optional[int] = None):
    """Start a background job to find PDFs for wanted papers."""
    wanted = vault.get_wanted_papers()
    if not wanted:
        return {"message": "No papers marked as wanted", "job_id": None}

    job_id = f"find_{len(active_jobs)}"
    active_jobs[job_id] = {
        "type": "find_pdfs",
        "status": "running",
        "total": len(wanted[:limit] if limit else wanted),
        "completed": 0,
        "success": 0,
    }

    def run_find():
        """Run PDF finding in a sync wrapper."""
        async def async_find():
            finder = PDFFinder(vault_path)
            papers_to_process = wanted[:limit] if limit else wanted

            for paper in papers_to_process:
                result = await finder.find_pdf(paper)
                active_jobs[job_id]["completed"] += 1

                if result.success:
                    paper.status = PaperStatus.DOWNLOADED
                    paper.pdf_path = result.pdf_path
                    active_jobs[job_id]["success"] += 1
                else:
                    paper.manual_download_links = result.manual_links
                    paper.search_attempts += 1

                vault.save_index()

            await finder.close()
            active_jobs[job_id]["status"] = "completed"

        asyncio.run(async_find())

    background_tasks.add_task(run_find)
    return {"job_id": job_id, "total": active_jobs[job_id]["total"]}


@app.post("/api/papers/{citekey}/find-pdf")
async def find_pdf_single(citekey: str):
    """Find PDF for a single paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    # Mark as wanted if discovered
    if paper.status == PaperStatus.DISCOVERED:
        paper.status = PaperStatus.WANTED

    finder = PDFFinder(vault_path)
    result = await finder.find_pdf(paper)
    await finder.close()

    if result.success:
        paper.status = PaperStatus.DOWNLOADED
        paper.pdf_path = result.pdf_path
        vault.save_index()
        return {"status": "success", "pdf_path": result.pdf_path, "source": result.source}
    else:
        paper.manual_download_links = result.manual_links
        paper.search_attempts += 1
        vault.save_index()
        return {
            "status": "not_found",
            "message": "No open access PDF found",
            "manual_links": result.manual_links
        }


@app.post("/api/summarize")
async def summarize_papers(background_tasks: BackgroundTasks, limit: Optional[int] = None):
    """Start a background job to summarize downloaded papers."""
    downloaded = vault.get_downloaded_papers()
    if not downloaded:
        return {"message": "No downloaded papers to summarize", "job_id": None}

    job_id = f"summarize_{len(active_jobs)}"
    active_jobs[job_id] = {
        "type": "summarize",
        "status": "running",
        "total": len(downloaded[:limit] if limit else downloaded),
        "completed": 0,
        "success": 0,
    }

    def run_summarize():
        summarizer = Summarizer(vault_path)
        papers_to_process = downloaded[:limit] if limit else downloaded

        for paper in papers_to_process:
            result = summarizer.summarize(paper)
            active_jobs[job_id]["completed"] += 1

            if result.success:
                paper.status = PaperStatus.SUMMARIZED
                paper.summary_path = result.summary_path
                paper.citations = result.extracted_citations
                active_jobs[job_id]["success"] += 1

            vault.save_index()

        active_jobs[job_id]["status"] = "completed"

    background_tasks.add_task(run_summarize)
    return {"job_id": job_id, "total": active_jobs[job_id]["total"]}


@app.post("/api/papers/{citekey}/summarize")
async def summarize_single_paper(citekey: str):
    """Summarize or re-summarize a single paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")
    if paper.status not in [PaperStatus.DOWNLOADED, PaperStatus.SUMMARIZED]:
        raise HTTPException(status_code=400, detail="Paper must be downloaded first")

    # Run summarization
    summarizer = Summarizer(vault_path)
    result = summarizer.summarize(paper)

    if result.success:
        paper.status = PaperStatus.SUMMARIZED
        paper.summary_path = result.summary_path
        paper.citations = result.extracted_citations
        vault.save_index()
        return {"status": "success", "summary_path": result.summary_path}
    else:
        raise HTTPException(status_code=500, detail=result.error)


@app.get("/api/jobs/{job_id}")
async def get_job_status(job_id: str):
    """Get the status of a background job."""
    if job_id not in active_jobs:
        raise HTTPException(status_code=404, detail="Job not found")
    return active_jobs[job_id]


@app.get("/api/jobs")
async def get_all_jobs():
    """Get all job statuses."""
    return active_jobs


@app.post("/api/papers/{citekey}/upload-pdf")
async def upload_pdf(citekey: str, file: UploadFile = File(...)):
    """Upload a PDF for a paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    # Save the uploaded file
    paper_dir = vault.papers_path / citekey
    paper_dir.mkdir(parents=True, exist_ok=True)
    pdf_path = paper_dir / "paper.pdf"

    with open(pdf_path, "wb") as f:
        content = await file.read()
        f.write(content)

    # Update paper status
    paper.pdf_path = str(pdf_path.relative_to(vault.vault_path))
    paper.status = PaperStatus.DOWNLOADED
    vault.save_index()

    return {"status": "uploaded", "path": str(pdf_path)}


@app.get("/api/papers/{citekey}/pdf")
async def get_pdf(citekey: str):
    """Download a paper's PDF."""
    paper = vault.get_paper(citekey)
    if not paper or not paper.pdf_path:
        raise HTTPException(status_code=404, detail="PDF not found")

    pdf_path = vault.vault_path / paper.pdf_path
    if not pdf_path.exists():
        raise HTTPException(status_code=404, detail="PDF file not found")

    return FileResponse(pdf_path, media_type="application/pdf")


@app.get("/api/papers/{citekey}/summary")
async def get_summary(citekey: str):
    """Get a paper's summary."""
    paper = vault.get_paper(citekey)
    if not paper or not paper.summary_path:
        raise HTTPException(status_code=404, detail="Summary not found")

    summary_path = vault.vault_path / "papers" / citekey / "summary.md"
    if not summary_path.exists():
        raise HTTPException(status_code=404, detail="Summary file not found")

    with open(summary_path, "r") as f:
        return {"content": f.read()}


@app.get("/api/manual-queue")
async def get_manual_queue():
    """Get papers that need manual download."""
    papers = vault.get_papers_needing_manual_download()
    return {
        "count": len(papers),
        "papers": [
            {
                "citekey": p.citekey,
                "title": p.title,
                "authors": p.authors,
                "year": p.year,
                "search_links": p.manual_download_links,
            }
            for p in papers
        ],
    }


@app.post("/api/import-bibtex")
async def import_bibtex(file: UploadFile = File(...)):
    """Import papers from an uploaded BibTeX file."""
    # Save to temp file
    temp_path = vault_path / "temp_import.bib"
    with open(temp_path, "wb") as f:
        content = await file.read()
        f.write(content)

    # Import
    added = vault.import_bibtex(temp_path)

    # Clean up
    temp_path.unlink()

    return {"added": added}


@app.post("/api/import-bibtex-path")
async def import_bibtex_from_path(request: ImportPathRequest):
    """Import papers from a BibTeX file at a specified path."""
    path = Path(request.file_path).expanduser()
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"File not found: {path}")
    if not path.suffix == ".bib":
        raise HTTPException(status_code=400, detail="File must be a .bib file")

    added = vault.import_bibtex(path)
    return {"added": added}


@app.post("/api/generate-index")
async def generate_index():
    """Generate the Obsidian index page."""
    index_path = vault.generate_index_page()
    return {"path": str(index_path)}


def run_server(host: str = "127.0.0.1", port: int = 8000):
    """Run the FastAPI server."""
    import uvicorn
    uvicorn.run(app, host=host, port=port)


if __name__ == "__main__":
    run_server()
