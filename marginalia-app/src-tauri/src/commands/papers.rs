//! Paper management commands

use crate::models::{Paper, PaperStatus, VaultStats};
use crate::storage::PaperRepo;
use crate::AppState;
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use chrono::Utc;
use tauri::State;
use tracing::info;

#[derive(serde::Serialize)]
pub struct PapersResponse {
    pub total: usize,
    pub papers: Vec<Paper>,
}

#[tauri::command]
pub async fn get_papers(
    vault_path: String,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: State<'_, AppState>,
) -> Result<PapersResponse, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);

    let limit = limit.unwrap_or(100);
    let offset = offset.unwrap_or(0);

    let papers = paper_repo.list(status.as_deref(), limit, offset)
        .map_err(|e| format!("Failed to get papers: {}", e))?;

    // Get total count
    let total = if let Some(ref status_filter) = status {
        paper_repo.count_by_status(status_filter)
            .map_err(|e| format!("Failed to count papers: {}", e))? as usize
    } else {
        paper_repo.stats()
            .map_err(|e| format!("Failed to get stats: {}", e))?.total
    };

    Ok(PapersResponse { total, papers })
}

#[tauri::command]
pub async fn get_paper(
    vault_path: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<Option<Paper>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    paper_repo.get(&citekey)
        .map_err(|e| format!("Failed to get paper: {}", e))
}

#[tauri::command]
pub async fn get_stats(
    vault_path: String,
    state: State<'_, AppState>,
) -> Result<VaultStats, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    paper_repo.stats()
        .map_err(|e| format!("Failed to get stats: {}", e))
}

#[tauri::command]
pub async fn update_paper_status(
    vault_path: String,
    citekey: String,
    status: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Validate status
    let valid_statuses = ["discovered", "wanted", "queued", "downloaded", "summarized", "failed"];
    if !valid_statuses.contains(&status.as_str()) {
        return Err(format!("Invalid status: {}", status));
    }

    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    paper_repo.update_status(&citekey, &status)
        .map_err(|e| format!("Failed to update status: {}", e))?;

    info!("Updated paper {} status to {}", citekey, status);
    Ok(())
}

#[tauri::command]
pub async fn search_papers(
    vault_path: String,
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<Paper>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    paper_repo.search(&query)
        .map_err(|e| format!("Failed to search papers: {}", e))
}

#[derive(serde::Deserialize)]
pub struct AddRelatedPaperRequest {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub source_citekey: String,
}

#[derive(serde::Serialize)]
pub struct AddRelatedPaperResponse {
    pub status: String,
    pub citekey: String,
}

#[tauri::command]
pub async fn add_related_paper(
    vault_path: String,
    request: AddRelatedPaperRequest,
    state: State<'_, AppState>,
) -> Result<AddRelatedPaperResponse, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);

    // First, check if paper already exists by matching title or authors+year
    if let Some(existing_citekey) = find_existing_paper(&paper_repo, &request)? {
        // Update the existing paper's cited_by if not already there
        if let Ok(Some(mut paper)) = paper_repo.get(&existing_citekey) {
            if !paper.cited_by.contains(&request.source_citekey) {
                paper.cited_by.push(request.source_citekey.clone());
                paper_repo.update(&paper)
                    .map_err(|e| format!("Failed to update paper: {}", e))?;
            }
        }
        return Ok(AddRelatedPaperResponse {
            status: "exists".to_string(),
            citekey: existing_citekey,
        });
    }

    // Generate citekey from first author lastname + year
    let mut citekey = generate_citekey(&request.authors, request.year);

    // If citekey already exists (collision), add a suffix
    let base_citekey = citekey.clone();
    let mut suffix = 'a';
    while paper_repo.exists(&citekey).unwrap_or(false) {
        citekey = format!("{}{}", base_citekey, suffix);
        suffix = (suffix as u8 + 1) as char;
        if suffix > 'z' {
            break;
        }
    }

    // Create new paper entry
    let paper = Paper {
        citekey: citekey.clone(),
        title: request.title.clone(),
        authors: request.authors.clone(),
        year: request.year,
        journal: None,
        volume: None,
        number: None,
        pages: None,
        doi: None,
        url: None,
        r#abstract: None,
        status: PaperStatus::Discovered,
        pdf_path: None,
        summary_path: None,
        notes_path: None,
        added_at: Utc::now(),
        downloaded_at: None,
        summarized_at: None,
        citations: Vec::new(),
        cited_by: vec![request.source_citekey.clone()],
        related_papers: Vec::new(),
        search_attempts: 0,
        last_search_error: None,
        manual_download_links: Vec::new(),
    };

    // Add to database
    paper_repo.insert(&paper)
        .map_err(|e| format!("Failed to insert paper: {}", e))?;

    // Append BibTeX entry to .bib file
    append_bibtex_entry(&vault_path, &citekey, &request)?;

    info!("Added related paper: {}", citekey);

    Ok(AddRelatedPaperResponse {
        status: "added".to_string(),
        citekey,
    })
}

fn find_existing_paper(repo: &PaperRepo, request: &AddRelatedPaperRequest) -> Result<Option<String>, String> {
    let request_title_normalized = normalize_title(&request.title);
    let request_first_author = request.authors.first()
        .map(|a| normalize_author(a))
        .unwrap_or_default();

    // Get all papers and check for duplicates
    let papers = repo.get_all()
        .map_err(|e| format!("Failed to get papers: {}", e))?;

    for (citekey, paper) in &papers {
        // Check by normalized title (fuzzy match)
        let paper_title_normalized = normalize_title(&paper.title);
        if !request_title_normalized.is_empty()
            && titles_match(&request_title_normalized, &paper_title_normalized) {
            return Ok(Some(citekey.clone()));
        }

        // Check by first author + year
        if let Some(paper_first_author) = paper.authors.first() {
            let paper_author_normalized = normalize_author(paper_first_author);
            if request.year == paper.year
                && !request_first_author.is_empty()
                && authors_match(&request_first_author, &paper_author_normalized) {
                return Ok(Some(citekey.clone()));
            }
        }
    }

    Ok(None)
}

fn normalize_title(title: &str) -> String {
    title.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_author(author: &str) -> String {
    // Extract last name, handling "Firstname Lastname" and "Lastname, Firstname"
    let name = if author.contains(',') {
        author.split(',').next().unwrap_or(author).trim()
    } else {
        author.split_whitespace().last().unwrap_or(author)
    };
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn titles_match(t1: &str, t2: &str) -> bool {
    // Check if titles are similar enough (one contains significant part of other)
    if t1 == t2 {
        return true;
    }

    // Check if one is a substring of the other (for abbreviated titles)
    if t1.len() > 10 && t2.len() > 10 {
        // Get first N significant words
        let words1: Vec<_> = t1.split_whitespace().take(5).collect();
        let words2: Vec<_> = t2.split_whitespace().take(5).collect();

        // If first 5 words match, consider it the same
        if words1.len() >= 3 && words1 == words2 {
            return true;
        }
    }

    false
}

fn authors_match(a1: &str, a2: &str) -> bool {
    // Simple last name match
    a1 == a2
}

fn generate_citekey(authors: &[String], year: Option<i32>) -> String {
    // Get first author's last name
    let first_author = authors.first()
        .map(|a| {
            // Handle "Lastname, Firstname" or "Firstname Lastname" formats
            if a.contains(',') {
                a.split(',').next().unwrap_or(a).trim()
            } else {
                a.split_whitespace().last().unwrap_or(a)
            }
        })
        .unwrap_or("unknown");

    // Clean up the name - lowercase, remove special chars
    let clean_name: String = first_author
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase();

    let year_str = year.map(|y| y.to_string()).unwrap_or_else(|| "nd".to_string());

    format!("{}{}", clean_name, year_str)
}

fn append_bibtex_entry(vault_path: &str, citekey: &str, request: &AddRelatedPaperRequest) -> Result<(), String> {
    let vault_path = PathBuf::from(vault_path);

    // Find existing .bib file or create references.bib
    let bib_path = find_or_create_bib_file(&vault_path)?;

    // Format authors for BibTeX
    let authors_bibtex = request.authors.join(" and ");

    // Create BibTeX entry
    let entry = format!(
        r#"
@article{{{},
  title = {{{}}},
  author = {{{}}},
  year = {{{}}}
}}
"#,
        citekey,
        request.title,
        authors_bibtex,
        request.year.map(|y| y.to_string()).unwrap_or_else(|| "".to_string())
    );

    // Append to file
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&bib_path)
        .map_err(|e| format!("Failed to open bib file: {}", e))?;

    file.write_all(entry.as_bytes())
        .map_err(|e| format!("Failed to write to bib file: {}", e))?;

    Ok(())
}

fn find_or_create_bib_file(vault_path: &PathBuf) -> Result<PathBuf, String> {
    // Look for existing .bib files
    if let Ok(entries) = fs::read_dir(vault_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "bib" {
                        return Ok(path);
                    }
                }
            }
        }
    }

    // No .bib file found, create references.bib
    Ok(vault_path.join("references.bib"))
}
