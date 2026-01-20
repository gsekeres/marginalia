//! Claude CLI commands
//!
//! Commands for checking Claude CLI status and summarizing papers.

use crate::adapters::{ClaudeCliClient, FileSystemAdapter};
use crate::models::{Paper, PaperStatus, RelatedPaper};
use crate::services::{SummarizationResult, SummarizerService};
use crate::storage::PaperRepo;
use crate::AppState;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use tracing::info;

#[derive(serde::Serialize)]
pub struct ClaudeStatus {
    pub available: bool,
    pub version: Option<String>,
    pub logged_in: bool,
}

#[tauri::command]
pub async fn check_claude_cli() -> Result<ClaudeStatus, String> {
    let available = ClaudeCliClient::is_available();

    if !available {
        return Ok(ClaudeStatus {
            available: false,
            version: None,
            logged_in: false,
        });
    }

    let version = ClaudeCliClient::get_version();
    let logged_in = ClaudeCliClient::is_logged_in();

    Ok(ClaudeStatus {
        available,
        version,
        logged_in,
    })
}

#[derive(serde::Serialize)]
pub struct SummaryResult {
    pub success: bool,
    pub summary_path: Option<String>,
    pub raw_response_path: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn summarize_paper(
    vault_path: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<SummaryResult, String> {
    // Check Claude availability
    if !ClaudeCliClient::is_available() {
        return Ok(SummaryResult {
            success: false,
            summary_path: None,
            raw_response_path: None,
            error: Some(
                "Claude CLI not installed. Install with: brew install anthropics/tap/claude"
                    .to_string(),
            ),
        });
    }

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

    // Check if PDF exists
    let pdf_path = paper
        .pdf_path
        .as_ref()
        .ok_or("Paper has no PDF downloaded")?;

    let full_pdf_path = PathBuf::from(&vault_path).join(pdf_path);
    if !full_pdf_path.exists() {
        return Err("PDF file not found".to_string());
    }

    // Initialize services
    let filesystem = FileSystemAdapter::new()?;
    let summarizer = SummarizerService::with_filesystem(filesystem.clone());

    // Extract text from PDF
    let text = filesystem.extract_pdf_text(&full_pdf_path)?;

    // Call summarizer service with JSON output validation
    let result = summarizer.summarize(&vault_path, &paper, &text).await;

    match result {
        SummarizationResult::Success {
            markdown,
            related_papers,
        } => {
            // Save the formatted markdown
            let summary_path = filesystem.save_summary(&vault_path, &paper, &markdown)?;

            // Update paper in database with auto-linked related papers
            {
                let db_guard = state.db.lock().map_err(|e| e.to_string())?;
                let db = db_guard.as_ref().ok_or("No vault is open")?;
                let paper_repo = PaperRepo::new(&db.conn);

                // Fetch all vault papers for auto-linking
                let vault_papers = paper_repo
                    .get_all()
                    .map_err(|e| format!("Failed to get vault papers: {}", e))?;

                // Auto-link related papers to vault papers by title/author matching
                let linked_related_papers = auto_link_related_papers(related_papers, &vault_papers);

                let mut updated_paper = paper.clone();
                updated_paper.status = PaperStatus::Summarized;
                updated_paper.summary_path = Some(summary_path.clone());
                updated_paper.summarized_at = Some(Utc::now());
                updated_paper.related_papers = linked_related_papers;

                paper_repo
                    .update(&updated_paper)
                    .map_err(|e| format!("Failed to update paper: {}", e))?;
            }

            info!("Successfully summarized paper: {}", citekey);

            Ok(SummaryResult {
                success: true,
                summary_path: Some(summary_path),
                raw_response_path: None,
                error: None,
            })
        }
        SummarizationResult::ParseFailure {
            raw_response_path,
            error,
        } => {
            info!(
                "Summarization parse failed for {}, raw saved to {}",
                citekey, raw_response_path
            );

            Ok(SummaryResult {
                success: false,
                summary_path: None,
                raw_response_path: Some(raw_response_path),
                error: Some(error),
            })
        }
        SummarizationResult::CliError(error) => {
            info!("Claude CLI error for {}: {}", citekey, error);

            Ok(SummaryResult {
                success: false,
                summary_path: None,
                raw_response_path: None,
                error: Some(error),
            })
        }
    }
}

/// Auto-link related papers to papers already in the vault by title/author matching
fn auto_link_related_papers(
    related_papers: Vec<RelatedPaper>,
    vault_papers: &HashMap<String, Paper>,
) -> Vec<RelatedPaper> {
    related_papers
        .into_iter()
        .map(|mut related| {
            // Try to find a matching paper in the vault
            if let Some(matched_citekey) = find_vault_match(&related, vault_papers) {
                info!(
                    "Auto-linked related paper '{}' to vault paper '{}'",
                    related.title, matched_citekey
                );
                related.vault_citekey = Some(matched_citekey);
            }
            related
        })
        .collect()
}

/// Find a vault paper that matches the related paper by title or author+year
fn find_vault_match(related: &RelatedPaper, vault_papers: &HashMap<String, Paper>) -> Option<String> {
    let related_title_normalized = normalize_title(&related.title);
    let related_first_author = related
        .authors
        .first()
        .map(|a| normalize_author(a))
        .unwrap_or_default();

    for (citekey, paper) in vault_papers {
        // Check by normalized title
        let paper_title_normalized = normalize_title(&paper.title);
        if !related_title_normalized.is_empty()
            && titles_match(&related_title_normalized, &paper_title_normalized)
        {
            return Some(citekey.clone());
        }

        // Check by first author + year
        if let Some(paper_first_author) = paper.authors.first() {
            let paper_author_normalized = normalize_author(paper_first_author);
            if related.year == paper.year
                && !related_first_author.is_empty()
                && related_first_author == paper_author_normalized
            {
                return Some(citekey.clone());
            }
        }
    }

    None
}

fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
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
    if t1 == t2 {
        return true;
    }

    // Check if first 5 significant words match (for abbreviated titles)
    if t1.len() > 10 && t2.len() > 10 {
        let words1: Vec<_> = t1.split_whitespace().take(5).collect();
        let words2: Vec<_> = t2.split_whitespace().take(5).collect();

        if words1.len() >= 3 && words1 == words2 {
            return true;
        }
    }

    false
}

/// Read the raw response file for a paper (saved when summarization parse fails)
#[tauri::command]
pub async fn read_raw_response(vault_path: String, citekey: String) -> Result<String, String> {
    let raw_response_path = PathBuf::from(&vault_path)
        .join("papers")
        .join(&citekey)
        .join("raw_response.txt");

    if !raw_response_path.exists() {
        return Err(format!(
            "Raw response file not found for paper: {}",
            citekey
        ));
    }

    std::fs::read_to_string(&raw_response_path)
        .map_err(|e| format!("Failed to read raw response: {}", e))
}
