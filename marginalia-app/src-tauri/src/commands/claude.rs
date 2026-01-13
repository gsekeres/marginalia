use crate::models::{Paper, PaperStatus, VaultIndex, RelatedPaper, Citation};
use crate::utils::claude::is_claude_available;
use std::path::PathBuf;
use std::fs;
use std::process::Command;
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

#[derive(serde::Serialize)]
pub struct ClaudeStatus {
    pub available: bool,
    pub version: Option<String>,
    pub logged_in: bool,
}

#[tauri::command]
pub async fn check_claude_cli() -> Result<ClaudeStatus, String> {
    let available = is_claude_available();

    if !available {
        return Ok(ClaudeStatus {
            available: false,
            version: None,
            logged_in: false,
        });
    }

    // Get version
    let version = Command::new("claude")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    // Check if logged in by trying a simple command
    let logged_in = Command::new("claude")
        .args(["--print", "-p", "Say hi"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

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
    pub error: Option<String>,
}

#[tauri::command]
pub async fn summarize_paper(vault_path: String, citekey: String) -> Result<SummaryResult, String> {
    // Check Claude availability
    if !is_claude_available() {
        return Ok(SummaryResult {
            success: false,
            summary_path: None,
            error: Some("Claude CLI not installed. Install with: brew install anthropics/tap/claude".to_string()),
        });
    }

    let index = load_index(&vault_path)?;
    let paper = index.papers.get(&citekey)
        .ok_or_else(|| format!("Paper not found: {}", citekey))?
        .clone();

    // Check if PDF exists
    let pdf_path = paper.pdf_path.as_ref()
        .ok_or("Paper has no PDF downloaded")?;

    let full_pdf_path = PathBuf::from(&vault_path).join(pdf_path);
    if !full_pdf_path.exists() {
        return Err("PDF file not found".to_string());
    }

    // Extract text from PDF
    let text = extract_pdf_text(&full_pdf_path)?;

    // Build summarization prompt
    let prompt = build_summary_prompt(&paper, &text);

    // Call Claude CLI
    let output = Command::new("claude")
        .args(["--print", "-p", &prompt])
        .output()
        .map_err(|e| format!("Failed to run Claude CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Ok(SummaryResult {
            success: false,
            summary_path: None,
            error: Some(format!("Claude CLI error: {}", stderr)),
        });
    }

    let response = String::from_utf8_lossy(&output.stdout).to_string();

    // Parse response and save summary
    let summary_content = format_summary(&paper, &response);

    let paper_dir = PathBuf::from(&vault_path).join("papers").join(&citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let summary_path = paper_dir.join("summary.md");
    fs::write(&summary_path, &summary_content)
        .map_err(|e| format!("Failed to write summary: {}", e))?;

    // Update paper status
    let mut index = load_index(&vault_path)?;
    if let Some(p) = index.papers.get_mut(&citekey) {
        p.status = PaperStatus::Summarized;
        p.summary_path = Some(format!("papers/{}/summary.md", citekey));
        p.summarized_at = Some(Utc::now());

        // Extract related papers from response (basic parsing)
        p.related_papers = extract_related_papers(&response);
    }
    save_index(&vault_path, &index)?;

    Ok(SummaryResult {
        success: true,
        summary_path: Some(format!("papers/{}/summary.md", citekey)),
        error: None,
    })
}

fn extract_pdf_text(pdf_path: &PathBuf) -> Result<String, String> {
    // Use pdf-extract crate
    let text = pdf_extract::extract_text(pdf_path)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))?;

    // Clean up text
    let cleaned = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    // Truncate if too long (Claude has context limits)
    let max_chars = 100_000;
    if cleaned.len() > max_chars {
        Ok(cleaned[..max_chars].to_string())
    } else {
        Ok(cleaned)
    }
}

fn build_summary_prompt(paper: &Paper, text: &str) -> String {
    format!(
        r#"You are an academic research assistant. Summarize this paper in a structured format.

Paper: "{}" by {} ({})

Provide your response in this exact format:

## Summary
[1-2 paragraph overview of the paper]

## Key Contributions
- [Bullet point 1]
- [Bullet point 2]
- [etc.]

## Methodology
[Brief description of methods used]

## Main Results
- [Key finding 1]
- [Key finding 2]
- [etc.]

## Related Work
For each related paper mentioned, use this format:
- Title: [paper title]
  Authors: [author names]
  Year: [year]
  Why Related: [brief explanation]

---

Paper text:
{}
"#,
        paper.title,
        paper.authors.join(", "),
        paper.year.map(|y| y.to_string()).unwrap_or_else(|| "n.d.".to_string()),
        text
    )
}

fn format_summary(paper: &Paper, response: &str) -> String {
    format!(
        r#"---
title: "{}"
authors: {:?}
year: {}
journal: "{}"
citekey: "{}"
doi: "{}"
status: "summarized"
---

{}
"#,
        paper.title,
        paper.authors,
        paper.year.unwrap_or(0),
        paper.journal.as_deref().unwrap_or(""),
        paper.citekey,
        paper.doi.as_deref().unwrap_or(""),
        response.trim()
    )
}

fn extract_related_papers(response: &str) -> Vec<RelatedPaper> {
    let mut related = Vec::new();

    // Find Related Work section
    if let Some(start) = response.find("## Related Work") {
        let section = &response[start..];
        // Stop at next section or end
        let end = section[15..].find("\n## ").map(|i| i + 15).unwrap_or(section.len());
        let section = &section[..end];

        let mut current: Option<RelatedPaper> = None;

        for line in section.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("- Title:") {
                // Save previous paper if exists
                if let Some(paper) = current.take() {
                    if !paper.title.is_empty() {
                        related.push(paper);
                    }
                }
                // Start new paper
                current = Some(RelatedPaper {
                    title: trimmed.trim_start_matches("- Title:").trim().to_string(),
                    authors: Vec::new(),
                    year: None,
                    why_related: String::new(),
                    vault_citekey: None,
                });
            } else if let Some(ref mut paper) = current {
                if trimmed.starts_with("Authors:") {
                    let authors_str = trimmed.trim_start_matches("Authors:").trim();
                    paper.authors = authors_str
                        .split(" and ")
                        .flat_map(|s| s.split(", "))
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                } else if trimmed.starts_with("Year:") {
                    let year_str = trimmed.trim_start_matches("Year:").trim();
                    paper.year = year_str.parse().ok();
                } else if trimmed.starts_with("Why Related:") {
                    paper.why_related = trimmed.trim_start_matches("Why Related:").trim().to_string();
                }
            }
        }

        // Don't forget the last paper
        if let Some(paper) = current {
            if !paper.title.is_empty() {
                related.push(paper);
            }
        }
    }

    related
}
