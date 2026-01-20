//! arXiv API client
//!
//! Provides open access PDF lookup via arXiv's OAI-PMH API.
//! See: https://arxiv.org/help/api/

use crate::utils::http::{rate_limiters, with_retry, RetryConfig};
use regex::Regex;
use reqwest::Client;
use std::time::Duration;
use tracing::{debug, warn};

/// Default timeout for arXiv API requests
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Client for the arXiv API
pub struct ArxivClient {
    client: Client,
}

impl ArxivClient {
    /// Create a new arXiv client
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self { client })
    }

    /// Create a new client with an existing reqwest client
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Extract arXiv ID from a DOI or URL
    ///
    /// Handles formats like:
    /// - 2301.12345
    /// - arxiv:2301.12345
    /// - https://arxiv.org/abs/2301.12345
    /// - https://arxiv.org/pdf/2301.12345.pdf
    /// - 10.48550/arXiv.2301.12345 (DOI format)
    pub fn extract_arxiv_id(input: &str) -> Option<String> {
        // Pattern for arXiv IDs (both old and new format)
        // New format: YYMM.NNNNN (e.g., 2301.12345)
        // Old format: category/YYMMNNN (e.g., hep-th/9901001)
        let new_pattern = Regex::new(r"(\d{4}\.\d{4,5}(?:v\d+)?)").ok()?;
        let old_pattern = Regex::new(r"([a-z-]+/\d{7}(?:v\d+)?)").ok()?;

        // Try new format first
        if let Some(cap) = new_pattern.captures(input) {
            return Some(cap[1].to_string());
        }

        // Try old format
        if let Some(cap) = old_pattern.captures(input) {
            return Some(cap[1].to_string());
        }

        None
    }

    /// Look up an open access PDF URL by arXiv ID
    ///
    /// # Arguments
    /// * `arxiv_id` - The arXiv ID (e.g., "2301.12345" or "hep-th/9901001")
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL
    /// * `None` - If the paper is not found
    pub async fn find_pdf_by_id(&self, arxiv_id: &str) -> Option<String> {
        // Wait for rate limit slot (arXiv asks for 3-second delay between requests)
        rate_limiters::ARXIV.wait_for_slot("arxiv").await;

        // Clean the ID (remove any v1, v2 suffixes for lookup)
        let clean_id = arxiv_id.trim();

        // First verify the paper exists via the API
        let api_url = format!(
            "https://export.arxiv.org/api/query?id_list={}",
            urlencoding::encode(clean_id)
        );

        debug!("arXiv lookup for ID: {}", clean_id);

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_secs(3), // arXiv asks for 3-second delays
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        };

        let client = self.client.clone();
        let api_url_owned = api_url.clone();

        let result = with_retry(
            &retry_config,
            &format!("arXiv lookup for {}", clean_id),
            || {
                let client = client.clone();
                let url = api_url_owned.clone();
                async move {
                    let resp = client
                        .get(&url)
                        .header("User-Agent", "Marginalia/1.0 (academic literature manager)")
                        .send()
                        .await
                        .map_err(|e| format!("request failed: {}", e))?;

                    if !resp.status().is_success() {
                        return Err(format!("status: {}", resp.status()));
                    }

                    resp.text()
                        .await
                        .map_err(|e| format!("read failed: {}", e))
                }
            },
            |err| {
                err.contains("request failed")
                    || err.contains("status: 5")
                    || err.contains("status: 429")
            },
        )
        .await;

        let xml_response = match result {
            Ok(r) => r,
            Err(e) => {
                warn!("arXiv API request failed: {}", e);
                return None;
            }
        };

        // Check if the paper exists (look for entry with id)
        // The API returns XML with <entry> tags for found papers
        if !xml_response.contains("<entry>") {
            debug!("Paper not found on arXiv: {}", clean_id);
            return None;
        }

        // arXiv PDF URLs are predictable: https://arxiv.org/pdf/{id}.pdf
        let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", clean_id);
        debug!("Found arXiv PDF: {}", pdf_url);

        Some(pdf_url)
    }

    /// Try to find a PDF by DOI (checks if DOI is an arXiv DOI)
    ///
    /// # Arguments
    /// * `doi` - The DOI to check
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL if DOI is an arXiv paper
    /// * `None` - If DOI is not an arXiv paper
    pub async fn find_pdf_by_doi(&self, doi: &str) -> Option<String> {
        // arXiv DOIs have format: 10.48550/arXiv.XXXX.XXXXX
        if !doi.starts_with("10.48550/arXiv.") && !doi.contains("arXiv") {
            return None;
        }

        if let Some(arxiv_id) = Self::extract_arxiv_id(doi) {
            return self.find_pdf_by_id(&arxiv_id).await;
        }

        None
    }

    /// Search for a paper by title on arXiv
    ///
    /// # Arguments
    /// * `title` - The paper title to search for
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL if a matching paper is found
    /// * `None` - If no matching paper is found
    pub async fn find_pdf_by_title(&self, title: &str) -> Option<String> {
        // Wait for rate limit slot
        rate_limiters::ARXIV.wait_for_slot("arxiv").await;

        let encoded_title = urlencoding::encode(title);
        let api_url = format!(
            "https://export.arxiv.org/api/query?search_query=ti:{}&start=0&max_results=1",
            encoded_title
        );

        debug!("arXiv title search: {}", title);

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_secs(3),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        };

        let client = self.client.clone();
        let api_url_owned = api_url.clone();

        let result = with_retry(
            &retry_config,
            &format!("arXiv title search for {}", title),
            || {
                let client = client.clone();
                let url = api_url_owned.clone();
                async move {
                    let resp = client
                        .get(&url)
                        .header("User-Agent", "Marginalia/1.0 (academic literature manager)")
                        .send()
                        .await
                        .map_err(|e| format!("request failed: {}", e))?;

                    if !resp.status().is_success() {
                        return Err(format!("status: {}", resp.status()));
                    }

                    resp.text()
                        .await
                        .map_err(|e| format!("read failed: {}", e))
                }
            },
            |err| {
                err.contains("request failed")
                    || err.contains("status: 5")
                    || err.contains("status: 429")
            },
        )
        .await;

        let xml_response = match result {
            Ok(r) => r,
            Err(e) => {
                warn!("arXiv title search failed: {}", e);
                return None;
            }
        };

        // Extract arXiv ID from the response
        // Look for <id>http://arxiv.org/abs/XXXX.XXXXX</id>
        let id_pattern = Regex::new(r"<id>https?://arxiv\.org/abs/([^<]+)</id>").ok()?;

        if let Some(cap) = id_pattern.captures(&xml_response) {
            let arxiv_id = &cap[1];
            let pdf_url = format!("https://arxiv.org/pdf/{}.pdf", arxiv_id);
            debug!("Found arXiv PDF via title search: {}", pdf_url);
            return Some(pdf_url);
        }

        debug!("No arXiv paper found for title: {}", title);
        None
    }
}

impl Default for ArxivClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ArxivClient")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_arxiv_id() {
        // New format
        assert_eq!(
            ArxivClient::extract_arxiv_id("2301.12345"),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            ArxivClient::extract_arxiv_id("arxiv:2301.12345"),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            ArxivClient::extract_arxiv_id("https://arxiv.org/abs/2301.12345"),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            ArxivClient::extract_arxiv_id("https://arxiv.org/pdf/2301.12345.pdf"),
            Some("2301.12345".to_string())
        );
        assert_eq!(
            ArxivClient::extract_arxiv_id("10.48550/arXiv.2301.12345"),
            Some("2301.12345".to_string())
        );

        // With version
        assert_eq!(
            ArxivClient::extract_arxiv_id("2301.12345v2"),
            Some("2301.12345v2".to_string())
        );

        // Old format
        assert_eq!(
            ArxivClient::extract_arxiv_id("hep-th/9901001"),
            Some("hep-th/9901001".to_string())
        );

        // Invalid
        assert_eq!(ArxivClient::extract_arxiv_id("not-an-arxiv-id"), None);
    }

    #[tokio::test]
    async fn test_arxiv_client_creation() {
        let client = ArxivClient::new();
        assert!(client.is_ok());
    }
}
