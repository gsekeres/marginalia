//! Filesystem adapter
//!
//! Handles file operations for PDFs, summaries, and other vault files.

use crate::models::Paper;
use crate::utils::http::{is_likely_login_page, is_valid_pdf, with_retry, RetryConfig};
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Default timeout for PDF downloads
const DOWNLOAD_TIMEOUT_SECS: u64 = 60;

/// Adapter for filesystem operations
#[derive(Clone)]
pub struct FileSystemAdapter {
    client: Client,
}

impl FileSystemAdapter {
    /// Create a new filesystem adapter with default HTTP client
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self { client })
    }

    /// Create a new adapter with an existing HTTP client
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Download a PDF from a URL and save it to the vault
    ///
    /// # Arguments
    /// * `vault_path` - Path to the vault directory
    /// * `citekey` - Paper citation key (used for directory name)
    /// * `url` - URL to download from
    ///
    /// # Returns
    /// * `Ok(relative_path)` - Relative path to the saved PDF
    /// * `Err(error)` - If download or save fails
    pub async fn download_pdf(
        &self,
        vault_path: &str,
        citekey: &str,
        url: &str,
    ) -> Result<String, String> {
        debug!("Downloading PDF for {} from {}", citekey, url);

        let retry_config = RetryConfig {
            max_retries: 3,
            initial_backoff: Duration::from_millis(1000),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        };

        let client = self.client.clone();
        let url_owned = url.to_string();

        // Use retry logic for the download
        let bytes = with_retry(
            &retry_config,
            &format!("PDF download from {}", url),
            || {
                let client = client.clone();
                let url = url_owned.clone();
                async move {
                    let resp = client
                        .get(&url)
                        .header("User-Agent", "Marginalia/1.0 (academic literature manager)")
                        .send()
                        .await
                        .map_err(|e| format!("HTTP request failed: {}", e))?;

                    if !resp.status().is_success() {
                        return Err(format!("HTTP status: {}", resp.status()));
                    }

                    // Get content type for validation
                    let content_type = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string());

                    let bytes = resp
                        .bytes()
                        .await
                        .map_err(|e| format!("Failed to read response: {}", e))?;

                    // Check for login page redirect (publisher paywall)
                    if is_likely_login_page(content_type.as_deref(), &bytes) {
                        return Err("Response appears to be a login/paywall page".to_string());
                    }

                    // Validate PDF magic bytes
                    if !is_valid_pdf(&bytes) {
                        return Err("Response is not a valid PDF (invalid magic bytes)".to_string());
                    }

                    Ok(bytes)
                }
            },
            |err| {
                // Retry on network errors, but not on validation failures
                err.contains("HTTP request failed")
                    || err.contains("HTTP status: 5")
                    || err.contains("HTTP status: 429")
            },
        )
        .await?;

        // Create paper directory
        let paper_dir = PathBuf::from(vault_path).join("papers").join(citekey);
        fs::create_dir_all(&paper_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let pdf_path = paper_dir.join("paper.pdf");
        fs::write(&pdf_path, &bytes).map_err(|e| format!("Failed to write PDF: {}", e))?;

        let relative_path = format!("papers/{}/paper.pdf", citekey);
        info!("Downloaded PDF to {}", relative_path);

        Ok(relative_path)
    }

    /// Save a summary file for a paper
    ///
    /// # Arguments
    /// * `vault_path` - Path to the vault directory
    /// * `paper` - Paper metadata (for frontmatter)
    /// * `summary_content` - The summary text from Claude
    ///
    /// # Returns
    /// * `Ok(relative_path)` - Relative path to the saved summary
    /// * `Err(error)` - If save fails
    pub fn save_summary(
        &self,
        vault_path: &str,
        paper: &Paper,
        summary_content: &str,
    ) -> Result<String, String> {
        let paper_dir = PathBuf::from(vault_path).join("papers").join(&paper.citekey);
        fs::create_dir_all(&paper_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let formatted = Self::format_summary_with_frontmatter(paper, summary_content);

        let summary_path = paper_dir.join("summary.md");
        fs::write(&summary_path, &formatted)
            .map_err(|e| format!("Failed to write summary: {}", e))?;

        let relative_path = format!("papers/{}/summary.md", paper.citekey);
        info!("Saved summary to {}", relative_path);

        Ok(relative_path)
    }

    /// Save raw LLM response (for debugging invalid JSON output)
    ///
    /// # Arguments
    /// * `vault_path` - Path to the vault directory
    /// * `citekey` - Paper citation key
    /// * `raw_content` - The raw response text
    ///
    /// # Returns
    /// * `Ok(relative_path)` - Relative path to the saved file
    /// * `Err(error)` - If save fails
    pub fn save_raw_response(
        &self,
        vault_path: &str,
        citekey: &str,
        raw_content: &str,
    ) -> Result<String, String> {
        let paper_dir = PathBuf::from(vault_path).join("papers").join(citekey);
        fs::create_dir_all(&paper_dir)
            .map_err(|e| format!("Failed to create directory: {}", e))?;

        let raw_path = paper_dir.join("raw_response.txt");
        fs::write(&raw_path, raw_content)
            .map_err(|e| format!("Failed to write raw response: {}", e))?;

        let relative_path = format!("papers/{}/raw_response.txt", citekey);
        warn!("Saved raw LLM response to {} (parsing failed)", relative_path);

        Ok(relative_path)
    }

    /// Extract text from a PDF file
    ///
    /// # Arguments
    /// * `pdf_path` - Full path to the PDF file
    ///
    /// # Returns
    /// * `Ok(text)` - Extracted text (truncated if too long)
    /// * `Err(error)` - If extraction fails
    pub fn extract_pdf_text(&self, pdf_path: &PathBuf) -> Result<String, String> {
        debug!("Extracting text from: {:?}", pdf_path);

        let text = pdf_extract::extract_text(pdf_path)
            .map_err(|e| format!("Failed to extract PDF text: {}", e))?;

        // Clean up text - remove empty lines
        let cleaned: String = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        // Truncate if too long (Claude has context limits)
        let max_chars = 100_000;
        if cleaned.len() > max_chars {
            debug!("Truncating PDF text from {} to {} chars", cleaned.len(), max_chars);
            Ok(cleaned[..max_chars].to_string())
        } else {
            Ok(cleaned)
        }
    }

    /// Format summary with YAML frontmatter
    fn format_summary_with_frontmatter(paper: &Paper, response: &str) -> String {
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

{}"#,
            paper.title,
            paper.authors,
            paper.year.unwrap_or(0),
            paper.journal.as_deref().unwrap_or(""),
            paper.citekey,
            paper.doi.as_deref().unwrap_or(""),
            response.trim()
        )
    }

    /// Check if a PDF exists for a paper
    pub fn pdf_exists(&self, vault_path: &str, citekey: &str) -> bool {
        let pdf_path = PathBuf::from(vault_path)
            .join("papers")
            .join(citekey)
            .join("paper.pdf");
        pdf_path.exists()
    }

    /// Check if a summary exists for a paper
    pub fn summary_exists(&self, vault_path: &str, citekey: &str) -> bool {
        let summary_path = PathBuf::from(vault_path)
            .join("papers")
            .join(citekey)
            .join("summary.md");
        summary_path.exists()
    }

    /// Get full path to a paper's PDF
    pub fn get_pdf_path(&self, vault_path: &str, citekey: &str) -> PathBuf {
        PathBuf::from(vault_path)
            .join("papers")
            .join(citekey)
            .join("paper.pdf")
    }
}

impl Default for FileSystemAdapter {
    fn default() -> Self {
        Self::new().expect("Failed to create FileSystemAdapter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_summary() {
        let paper = Paper {
            citekey: "smith2020".to_string(),
            title: "Test Paper".to_string(),
            authors: vec!["Smith, John".to_string()],
            year: Some(2020),
            journal: Some("Test Journal".to_string()),
            doi: Some("10.1234/test".to_string()),
            ..Default::default()
        };

        let formatted = FileSystemAdapter::format_summary_with_frontmatter(&paper, "Summary content");
        assert!(formatted.contains("title: \"Test Paper\""));
        assert!(formatted.contains("citekey: \"smith2020\""));
        assert!(formatted.contains("Summary content"));
    }
}
