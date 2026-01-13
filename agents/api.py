"""FastAPI web dashboard for Marginalia."""

import asyncio
import os
import secrets
from pathlib import Path
from typing import Optional

from dotenv import load_dotenv
load_dotenv()  # Load .env file

from fastapi import FastAPI, HTTPException, UploadFile, File, BackgroundTasks, Request, Depends
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, HTMLResponse, RedirectResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel
from starlette.middleware.sessions import SessionMiddleware

from .models import Paper, PaperStatus, RelatedPaper, PaperNotes, Highlight, PaperConnection
from .pdf_finder import PDFFinder
from .summarizer import Summarizer
from .vault import VaultManager, generate_citekey
from .auth import (
    oauth, configure_oauth, get_user_store, create_access_token,
    get_current_user, require_auth, is_oauth_configured, User
)

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
        "https://marginalia.site",
        "https://www.marginalia.site",
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Session middleware for OAuth state
app.add_middleware(
    SessionMiddleware,
    secret_key=os.getenv("SECRET_KEY", secrets.token_hex(32))
)

# Configure OAuth providers
configure_oauth()

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


class SyncRequest(BaseModel):
    folder_path: str
    include_pdfs: bool = True
    include_summaries: bool = True
    include_bibtex: bool = True


class SetSourcePathRequest(BaseModel):
    source_path: str


class BrowseRequest(BaseModel):
    path: Optional[str] = None


class SetupRequest(BaseModel):
    claude_oauth_key: Optional[str] = None
    storage_path: str


class NotesRequest(BaseModel):
    content: str


class HighlightRequest(BaseModel):
    page: int
    rects: list[dict]
    text: str = ""
    color: str = "yellow"
    note: Optional[str] = None


class ConnectPapersRequest(BaseModel):
    source: str  # source citekey
    target: str  # target citekey
    reason: str = ""  # why they're related


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
async def root(request: Request, user: Optional[User] = Depends(get_current_user)):
    """Serve landing page or dashboard based on auth status."""
    app_dir = Path(__file__).parent.parent / "app"

    # If OAuth is not configured, skip auth and show dashboard
    if not is_oauth_configured():
        return FileResponse(app_dir / "index.html")

    # If user is authenticated
    if user:
        # Check if setup is complete
        if not user.setup_complete:
            return FileResponse(app_dir / "setup.html")
        return FileResponse(app_dir / "index.html")

    # Not authenticated - show landing page
    return FileResponse(app_dir / "landing.html")


@app.get("/setup")
async def setup_page(user: Optional[User] = Depends(get_current_user)):
    """Serve setup page."""
    app_dir = Path(__file__).parent.parent / "app"
    return FileResponse(app_dir / "setup.html")


@app.get("/demo")
async def demo_page():
    """Serve dashboard in demo mode (no auth required)."""
    return FileResponse(Path(__file__).parent.parent / "app" / "index.html")


# OAuth routes

@app.get("/auth/login/{provider}")
async def auth_login(provider: str, request: Request):
    """Initiate OAuth flow."""
    if provider not in ['google', 'github']:
        raise HTTPException(status_code=400, detail="Invalid provider")

    if not is_oauth_configured():
        raise HTTPException(status_code=400, detail="OAuth not configured")

    client = oauth.create_client(provider)
    if not client:
        raise HTTPException(status_code=400, detail=f"{provider} OAuth not configured")

    redirect_uri = str(request.url_for('auth_callback', provider=provider))
    return await client.authorize_redirect(request, redirect_uri)


@app.get("/auth/callback/{provider}")
async def auth_callback(provider: str, request: Request):
    """Handle OAuth callback."""
    if provider not in ['google', 'github']:
        raise HTTPException(status_code=400, detail="Invalid provider")

    client = oauth.create_client(provider)
    token = await client.authorize_access_token(request)

    if provider == 'google':
        user_info = token.get('userinfo')
        if not user_info:
            user_info = await client.parse_id_token(token)
        email = user_info['email']
        name = user_info.get('name')
        picture = user_info.get('picture')
        provider_id = user_info['sub']
    else:  # github
        resp = await client.get('user', token=token)
        user_data = resp.json()
        # Get email separately for GitHub
        email_resp = await client.get('user/emails', token=token)
        emails = email_resp.json()
        primary_email = next((e['email'] for e in emails if e.get('primary')), emails[0]['email'] if emails else None)
        email = primary_email or f"{user_data['login']}@github.local"
        name = user_data.get('name') or user_data.get('login')
        picture = user_data.get('avatar_url')
        provider_id = str(user_data['id'])

    # Get or create user
    user_store = get_user_store()
    user = user_store.get_or_create_user(provider, provider_id, email, name, picture)

    # Create JWT token
    access_token = create_access_token(user.id)

    # Redirect based on setup status
    redirect_url = "/setup" if not user.setup_complete else "/"

    response = RedirectResponse(url=redirect_url)
    response.set_cookie(
        key="marginalia_token",
        value=access_token,
        httponly=True,
        samesite="lax",
        max_age=7 * 24 * 60 * 60,  # 1 week
    )
    return response


@app.get("/auth/logout")
async def auth_logout():
    """Clear auth cookie and redirect to landing."""
    response = RedirectResponse(url="/")
    response.delete_cookie("marginalia_token")
    return response


@app.get("/api/me")
async def get_me(user: Optional[User] = Depends(get_current_user)):
    """Get current user info."""
    if not user:
        return {"authenticated": False}
    return {
        "authenticated": True,
        "user": {
            "id": user.id,
            "email": user.email,
            "name": user.name,
            "picture": user.picture,
            "setup_complete": user.setup_complete,
        }
    }


@app.post("/api/setup")
async def complete_setup(request_data: SetupRequest, user: User = Depends(require_auth)):
    """Complete user setup."""
    user.claude_oauth_key = request_data.claude_oauth_key
    user.storage_path = request_data.storage_path
    user.setup_complete = True

    user_store = get_user_store()
    user_store.update_user(user)

    return {"status": "setup_complete"}


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


@app.get("/api/papers/{citekey}/notes")
async def get_notes(citekey: str):
    """Get notes and highlights for a paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    notes = vault.get_paper_notes(citekey)
    return notes.model_dump()


@app.put("/api/papers/{citekey}/notes")
async def save_notes(citekey: str, request: NotesRequest):
    """Save notes content for a paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    notes = vault.get_paper_notes(citekey)
    notes.content = request.content
    vault.save_paper_notes(notes)

    # Update paper to reference notes
    if not paper.notes_path:
        paper.notes_path = f"papers/{citekey}/notes.json"
        vault.save_index()

    return {"status": "saved"}


@app.post("/api/papers/{citekey}/highlights")
async def add_highlight(citekey: str, request: HighlightRequest):
    """Add a highlight to a paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    notes = vault.get_paper_notes(citekey)
    highlight = Highlight(
        page=request.page,
        rects=request.rects,
        text=request.text,
        color=request.color,
        note=request.note,
    )
    notes.highlights.append(highlight)
    vault.save_paper_notes(notes)

    # Update paper to reference notes
    if not paper.notes_path:
        paper.notes_path = f"papers/{citekey}/notes.json"
        vault.save_index()

    return {"status": "added", "highlight_id": highlight.id}


@app.delete("/api/papers/{citekey}/highlights/{highlight_id}")
async def delete_highlight(citekey: str, highlight_id: str):
    """Remove a highlight from a paper."""
    paper = vault.get_paper(citekey)
    if not paper:
        raise HTTPException(status_code=404, detail="Paper not found")

    notes = vault.get_paper_notes(citekey)
    original_count = len(notes.highlights)
    notes.highlights = [h for h in notes.highlights if h.id != highlight_id]

    if len(notes.highlights) == original_count:
        raise HTTPException(status_code=404, detail="Highlight not found")

    vault.save_paper_notes(notes)
    return {"status": "deleted"}


# Graph API endpoints

@app.get("/api/graph")
async def get_graph():
    """Get all nodes (papers) and edges (connections) for the network graph."""
    nodes = []
    for paper in vault.index.papers.values():
        nodes.append({
            "id": paper.citekey,
            "label": paper.citekey,
            "title": paper.title,
            "group": paper.status.value,
            "year": paper.year,
        })

    edges = []
    for conn in vault.index.connections:
        edges.append({
            "from": conn.source,
            "to": conn.target,
            "title": conn.reason,
        })

    return {"nodes": nodes, "edges": edges}


@app.post("/api/graph/connect")
async def connect_papers(request: ConnectPapersRequest):
    """Connect two papers in the graph (bidirectional)."""
    # Verify both papers exist
    source_paper = vault.get_paper(request.source)
    target_paper = vault.get_paper(request.target)

    if not source_paper:
        raise HTTPException(status_code=404, detail=f"Paper not found: {request.source}")
    if not target_paper:
        raise HTTPException(status_code=404, detail=f"Paper not found: {request.target}")

    # Check if connection already exists
    for conn in vault.index.connections:
        if (conn.source == request.source and conn.target == request.target) or \
           (conn.source == request.target and conn.target == request.source):
            return {"status": "exists", "message": "Connection already exists"}

    # Add the connection
    connection = PaperConnection(
        source=request.source,
        target=request.target,
        reason=request.reason,
    )
    vault.index.connections.append(connection)
    vault.save_index()

    return {"status": "connected"}


@app.delete("/api/graph/connection/{source}/{target}")
async def delete_connection(source: str, target: str):
    """Remove a connection between two papers."""
    original_count = len(vault.index.connections)
    vault.index.connections = [
        c for c in vault.index.connections
        if not ((c.source == source and c.target == target) or
                (c.source == target and c.target == source))
    ]

    if len(vault.index.connections) == original_count:
        raise HTTPException(status_code=404, detail="Connection not found")

    vault.save_index()
    return {"status": "deleted"}


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

    # Store the source path for sync defaults
    vault.index.source_bib_path = str(path.resolve())
    vault.save_index()

    return {"added": added, "source_path": str(path.resolve())}


@app.get("/api/config")
async def get_config():
    """Get configuration including default sync path."""
    source_path = vault.index.source_bib_path
    default_sync_path = None

    if source_path:
        # Use the parent directory of the .bib file as default sync path
        default_sync_path = str(Path(source_path).parent)

    return {
        "source_bib_path": source_path,
        "default_sync_path": default_sync_path,
    }


@app.post("/api/config/source-path")
async def set_source_path(request: SetSourcePathRequest):
    """Set the source .bib file path."""
    path = Path(request.source_path).expanduser()
    vault.index.source_bib_path = str(path.resolve()) if path.exists() else str(path)
    vault.save_index()
    return {"source_bib_path": vault.index.source_bib_path}


@app.post("/api/browse")
async def browse_directory(request: BrowseRequest):
    """Browse filesystem directories for folder selection."""
    import os

    # Start from home or provided path
    if request.path:
        base_path = Path(request.path).expanduser()
    else:
        base_path = Path.home()

    if not base_path.exists():
        base_path = Path.home()

    # Get parent path
    parent = str(base_path.parent) if base_path.parent != base_path else None

    # List directories only
    directories = []
    try:
        for entry in sorted(base_path.iterdir()):
            if entry.is_dir() and not entry.name.startswith('.'):
                directories.append({
                    "name": entry.name,
                    "path": str(entry),
                })
    except PermissionError:
        pass

    # Also list .bib files for source selection
    bib_files = []
    try:
        for entry in sorted(base_path.iterdir()):
            if entry.is_file() and entry.suffix == '.bib':
                bib_files.append({
                    "name": entry.name,
                    "path": str(entry),
                })
    except PermissionError:
        pass

    return {
        "current": str(base_path),
        "parent": parent,
        "directories": directories,
        "bib_files": bib_files,
    }


@app.post("/api/generate-index")
async def generate_index():
    """Generate the Obsidian index page."""
    index_path = vault.generate_index_page()
    return {"path": str(index_path)}


@app.post("/api/sync")
async def sync_to_folder(request: SyncRequest):
    """Sync vault contents to a local folder.

    Structure:
    - folder/papers/{citekey}/paper.pdf
    - folder/papers/{citekey}/summary.md
    - folder/references.bib
    """
    import shutil

    folder = Path(request.folder_path).expanduser()
    if not folder.exists():
        folder.mkdir(parents=True, exist_ok=True)

    papers_folder = folder / "papers"
    papers_folder.mkdir(exist_ok=True)

    results = {
        "folder": str(folder),
        "pdfs_copied": 0,
        "summaries_copied": 0,
        "bibtex_exported": False,
    }

    # Copy PDFs and summaries into papers/{citekey}/ folders
    for paper in vault.index.papers.values():
        paper_dir = papers_folder / paper.citekey
        has_content = False

        # Copy PDF
        if request.include_pdfs and paper.pdf_path:
            src_path = vault.vault_path / paper.pdf_path
            if src_path.exists():
                paper_dir.mkdir(exist_ok=True)
                dest_path = paper_dir / "paper.pdf"
                shutil.copy2(src_path, dest_path)
                results["pdfs_copied"] += 1
                has_content = True

        # Copy summary
        if request.include_summaries:
            if paper.summary_path or paper.status == PaperStatus.SUMMARIZED:
                summary_path = vault.vault_path / "papers" / paper.citekey / "summary.md"
                if summary_path.exists():
                    paper_dir.mkdir(exist_ok=True)
                    dest_path = paper_dir / "summary.md"
                    shutil.copy2(summary_path, dest_path)
                    results["summaries_copied"] += 1
                    has_content = True

    # Export BibTeX
    if request.include_bibtex:
        bibtex_path = folder / "references.bib"
        vault.export_bibtex(bibtex_path)
        results["bibtex_exported"] = True
        results["bibtex_path"] = str(bibtex_path)

    return results


@app.get("/api/export-bibtex")
async def export_bibtex_api():
    """Export all papers as BibTeX."""
    bibtex_content = ""
    for paper in vault.index.papers.values():
        bibtex_content += paper.to_bibtex() + "\n\n"

    from fastapi.responses import PlainTextResponse
    return PlainTextResponse(
        content=bibtex_content,
        media_type="text/plain",
        headers={"Content-Disposition": "attachment; filename=references.bib"}
    )


def run_server(host: str = "127.0.0.1", port: int = 8000):
    """Run the FastAPI server."""
    import uvicorn
    uvicorn.run(app, host=host, port=port)


if __name__ == "__main__":
    run_server()
