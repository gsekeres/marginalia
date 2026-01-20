//! PDF finder commands
//!
//! Uses adapters to search for and download open access PDFs from multiple sources.

use crate::adapters::{ArxivClient, ClaudeCliClient, FileSystemAdapter, SemanticScholarClient, UnpaywallClient};
use crate::models::PaperStatus;
use crate::storage::PaperRepo;
use crate::AppState;
use chrono::Utc;
use reqwest::Client;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tracing::{info, warn};

/// Progress event for PDF search operations
#[derive(Clone, serde::Serialize)]
pub struct PdfSearchProgress {
    /// Citation key being searched
    pub citekey: String,
    /// Current progress percentage (0-100)
    pub progress: i32,
    /// Current source being searched (e.g., "arxiv", "unpaywall")
    pub current_source: Option<String>,
    /// Human-readable status message
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct FindPdfResult {
    pub success: bool,
    pub pdf_path: Option<String>,
    pub source: Option<String>,
    pub manual_links: Vec<String>,
    pub error: Option<String>,
}

/// Emit a progress event to the frontend
fn emit_progress(app: &AppHandle, citekey: &str, progress: i32, source: Option<&str>, message: &str) {
    let event = PdfSearchProgress {
        citekey: citekey.to_string(),
        progress,
        current_source: source.map(|s| s.to_string()),
        message: message.to_string(),
    };
    if let Err(e) = app.emit("pdf:search:progress", &event) {
        warn!("Failed to emit progress event: {}", e);
    }
}

#[tauri::command]
pub async fn find_pdf(
    app: AppHandle,
    vault_path: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<FindPdfResult, String> {
    // Emit initial progress
    emit_progress(&app, &citekey, 0, None, "Starting PDF search...");

    // Get paper from database
    let paper = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("No vault is open")?;
        let paper_repo = PaperRepo::new(&db.conn);
        paper_repo
            .get(&citekey)
            .map_err(|e| format!("Failed to get paper: {}", e))?
            .ok_or_else(|| format!("Paper not found: {}", citekey))?
    };

    // Create shared HTTP client
    let http_client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Initialize adapters with shared client
    let unpaywall = UnpaywallClient::with_client(http_client.clone(), None);
    let semantic_scholar = SemanticScholarClient::with_client(http_client.clone(), None);
    let arxiv = ArxivClient::with_client(http_client.clone());
    let claude_cli = ClaudeCliClient::new();
    let filesystem = FileSystemAdapter::with_client(http_client.clone());

    // Try different sources
    let mut pdf_url: Option<(String, String)> = None;

    // 1. Try arXiv first (if DOI contains arXiv or looks like an arXiv ID)
    emit_progress(&app, &citekey, 10, Some("arxiv"), "Checking arXiv...");
    if let Some(doi) = &paper.doi {
        if let Some(url) = arxiv.find_pdf_by_doi(doi).await {
            pdf_url = Some((url, "arxiv".to_string()));
        }
    }

    // 2. Try Unpaywall (if DOI exists)
    if pdf_url.is_none() {
        emit_progress(&app, &citekey, 25, Some("unpaywall"), "Checking Unpaywall...");
        if let Some(doi) = &paper.doi {
            if let Some(url) = unpaywall.find_pdf_by_doi(doi).await {
                pdf_url = Some((url, "unpaywall".to_string()));
            }
        }
    }

    // 3. Try Semantic Scholar by DOI
    if pdf_url.is_none() {
        emit_progress(&app, &citekey, 40, Some("semantic_scholar"), "Checking Semantic Scholar (DOI)...");
        if let Some(doi) = &paper.doi {
            if let Some(url) = semantic_scholar.find_pdf_by_doi(doi).await {
                pdf_url = Some((url, "semantic_scholar".to_string()));
            }
        }
    }

    // 4. Try Semantic Scholar by title
    if pdf_url.is_none() {
        emit_progress(&app, &citekey, 55, Some("semantic_scholar"), "Checking Semantic Scholar (title)...");
        if let Some(url) = semantic_scholar.find_pdf_by_title(&paper.title).await {
            pdf_url = Some((url, "semantic_scholar".to_string()));
        }
    }

    // 5. Try arXiv by title (for preprints that may not have DOIs)
    if pdf_url.is_none() {
        emit_progress(&app, &citekey, 70, Some("arxiv"), "Checking arXiv (title)...");
        if let Some(url) = arxiv.find_pdf_by_title(&paper.title).await {
            pdf_url = Some((url, "arxiv".to_string()));
        }
    }

    // 6. Try Claude CLI (if available)
    if pdf_url.is_none() && ClaudeCliClient::is_available() {
        emit_progress(&app, &citekey, 85, Some("claude"), "Asking Claude for PDF URL...");
        if let Some(url) = claude_cli.find_pdf_url(&paper).await {
            pdf_url = Some((url, "claude".to_string()));
        }
    }

    // If we found a URL, download the PDF
    if let Some((url, source)) = pdf_url {
        emit_progress(&app, &citekey, 90, Some(&source), &format!("Downloading from {}...", source));
        match filesystem.download_pdf(&vault_path, &citekey, &url).await {
            Ok(path) => {
                // Update paper status in database
                {
                    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
                    let db = db_guard.as_ref().ok_or("No vault is open")?;
                    let paper_repo = PaperRepo::new(&db.conn);

                    let mut updated_paper = paper.clone();
                    updated_paper.status = PaperStatus::Downloaded;
                    updated_paper.pdf_path = Some(path.clone());
                    updated_paper.downloaded_at = Some(Utc::now());

                    paper_repo
                        .update(&updated_paper)
                        .map_err(|e| format!("Failed to update paper: {}", e))?;
                }

                info!("Found PDF for {} from {}", citekey, source);
                emit_progress(&app, &citekey, 100, Some(&source), "PDF downloaded successfully!");

                return Ok(FindPdfResult {
                    success: true,
                    pdf_path: Some(path),
                    source: Some(source),
                    manual_links: Vec::new(),
                    error: None,
                });
            }
            Err(e) => {
                warn!("Failed to download from {}: {}", url, e);
            }
        }
    }

    // Generate manual search links
    emit_progress(&app, &citekey, 95, None, "Generating manual search links...");
    let manual_links = generate_search_links(&paper.title, &paper.authors, paper.doi.as_deref());

    // Update search attempts in database
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("No vault is open")?;
        let paper_repo = PaperRepo::new(&db.conn);

        let mut updated_paper = paper.clone();
        updated_paper.search_attempts += 1;
        updated_paper.manual_download_links = manual_links.clone();

        paper_repo
            .update(&updated_paper)
            .map_err(|e| format!("Failed to update paper: {}", e))?;
    }

    emit_progress(&app, &citekey, 100, None, "No open access PDF found");

    Ok(FindPdfResult {
        success: false,
        pdf_path: None,
        source: None,
        manual_links,
        error: Some("No open access PDF found".to_string()),
    })
}

/// Generate manual search links for a paper
fn generate_search_links(title: &str, authors: &[String], doi: Option<&str>) -> Vec<String> {
    let mut links = Vec::new();

    let encoded_title = urlencoding::encode(title);
    let first_author = authors.first().map(|s| s.as_str()).unwrap_or("");
    let encoded_author = urlencoding::encode(first_author);

    // Google Scholar
    links.push(format!(
        "https://scholar.google.com/scholar?q={}",
        encoded_title
    ));

    // Semantic Scholar
    links.push(format!(
        "https://www.semanticscholar.org/search?q={}",
        encoded_title
    ));

    // DOI link
    if let Some(doi) = doi {
        links.push(format!("https://doi.org/{}", doi));
    }

    // SSRN search
    links.push(format!(
        "https://papers.ssrn.com/sol3/results.cfm?txtKey_Words={}",
        encoded_title
    ));

    // Author search
    if !first_author.is_empty() {
        links.push(format!(
            "https://scholar.google.com/scholar?q=author:{}+{}",
            encoded_author, encoded_title
        ));
    }

    links
}

#[tauri::command]
pub async fn download_pdf(
    vault_path: String,
    citekey: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let filesystem = FileSystemAdapter::new()?;

    let path = filesystem.download_pdf(&vault_path, &citekey, &url).await?;

    // Update paper in database
    {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        let db = db_guard.as_ref().ok_or("No vault is open")?;
        let paper_repo = PaperRepo::new(&db.conn);

        if let Some(mut paper) = paper_repo.get(&citekey).map_err(|e| e.to_string())? {
            paper.status = PaperStatus::Downloaded;
            paper.pdf_path = Some(path.clone());
            paper.downloaded_at = Some(Utc::now());
            paper_repo.update(&paper).map_err(|e| e.to_string())?;
        }
    }

    info!("Downloaded PDF for {} from URL", citekey);
    Ok(path)
}
