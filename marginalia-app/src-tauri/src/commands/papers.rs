use crate::models::{Paper, PaperStatus, VaultIndex, VaultStats};
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use chrono::Utc;

const INDEX_FILENAME: &str = ".marginalia_index.json";

fn load_index(vault_path: &str) -> Result<VaultIndex, String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    if !path.exists() {
        return Ok(VaultIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read index: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse index: {}", e))
}

fn save_index(vault_path: &str, index: &VaultIndex) -> Result<(), String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write index: {}", e))
}

#[tauri::command]
pub async fn get_papers(
    vault_path: String,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<PapersResponse, String> {
    let index = load_index(&vault_path)?;

    let mut papers: Vec<Paper> = index.papers.values().cloned().collect();

    // Filter by status if provided
    if let Some(status_filter) = status {
        papers.retain(|p| {
            let status_str = match p.status {
                PaperStatus::Discovered => "discovered",
                PaperStatus::Wanted => "wanted",
                PaperStatus::Queued => "queued",
                PaperStatus::Downloaded => "downloaded",
                PaperStatus::Summarized => "summarized",
                PaperStatus::Failed => "failed",
            };
            status_str == status_filter
        });
    }

    // Sort by year descending
    papers.sort_by(|a, b| b.year.cmp(&a.year));

    let total = papers.len();

    // Apply pagination
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(100);
    let papers: Vec<Paper> = papers.into_iter().skip(offset).take(limit).collect();

    Ok(PapersResponse { total, papers })
}

#[derive(serde::Serialize)]
pub struct PapersResponse {
    pub total: usize,
    pub papers: Vec<Paper>,
}

#[tauri::command]
pub async fn get_paper(vault_path: String, citekey: String) -> Result<Option<Paper>, String> {
    let index = load_index(&vault_path)?;
    Ok(index.papers.get(&citekey).cloned())
}

#[tauri::command]
pub async fn get_stats(vault_path: String) -> Result<VaultStats, String> {
    let index = load_index(&vault_path)?;
    Ok(index.stats())
}

#[tauri::command]
pub async fn update_paper_status(
    vault_path: String,
    citekey: String,
    status: String,
) -> Result<(), String> {
    let mut index = load_index(&vault_path)?;

    let paper = index.papers.get_mut(&citekey)
        .ok_or_else(|| format!("Paper not found: {}", citekey))?;

    paper.status = match status.as_str() {
        "discovered" => PaperStatus::Discovered,
        "wanted" => PaperStatus::Wanted,
        "queued" => PaperStatus::Queued,
        "downloaded" => PaperStatus::Downloaded,
        "summarized" => PaperStatus::Summarized,
        "failed" => PaperStatus::Failed,
        _ => return Err(format!("Invalid status: {}", status)),
    };

    save_index(&vault_path, &index)?;
    Ok(())
}

#[tauri::command]
pub async fn search_papers(vault_path: String, query: String) -> Result<Vec<Paper>, String> {
    let index = load_index(&vault_path)?;
    let query = query.to_lowercase();

    let results: Vec<Paper> = index.papers.values()
        .filter(|p| {
            p.title.to_lowercase().contains(&query) ||
            p.citekey.to_lowercase().contains(&query) ||
            p.authors.iter().any(|a| a.to_lowercase().contains(&query))
        })
        .cloned()
        .collect();

    Ok(results)
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
) -> Result<AddRelatedPaperResponse, String> {
    let mut index = load_index(&vault_path)?;

    // First, check if paper already exists by matching title or authors+year
    if let Some(existing_citekey) = find_existing_paper(&index, &request) {
        // Update the existing paper's cited_by if not already there
        if let Some(paper) = index.papers.get_mut(&existing_citekey) {
            if !paper.cited_by.contains(&request.source_citekey) {
                paper.cited_by.push(request.source_citekey.clone());
                save_index(&vault_path, &index)?;
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
    while index.papers.contains_key(&citekey) {
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

    // Add to index
    index.papers.insert(citekey.clone(), paper);
    save_index(&vault_path, &index)?;

    // Append BibTeX entry to .bib file
    append_bibtex_entry(&vault_path, &citekey, &request)?;

    Ok(AddRelatedPaperResponse {
        status: "added".to_string(),
        citekey,
    })
}

fn find_existing_paper(index: &VaultIndex, request: &AddRelatedPaperRequest) -> Option<String> {
    let request_title_normalized = normalize_title(&request.title);
    let request_first_author = request.authors.first()
        .map(|a| normalize_author(a))
        .unwrap_or_default();

    for (citekey, paper) in &index.papers {
        // Check by normalized title (fuzzy match)
        let paper_title_normalized = normalize_title(&paper.title);
        if !request_title_normalized.is_empty()
            && titles_match(&request_title_normalized, &paper_title_normalized) {
            return Some(citekey.clone());
        }

        // Check by first author + year
        if let Some(paper_first_author) = paper.authors.first() {
            let paper_author_normalized = normalize_author(paper_first_author);
            if request.year == paper.year
                && !request_first_author.is_empty()
                && authors_match(&request_first_author, &paper_author_normalized) {
                return Some(citekey.clone());
            }
        }
    }

    None
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
