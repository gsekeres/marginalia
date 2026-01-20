//! Semantic Scholar API client
//!
//! Provides academic paper search and open access PDF lookup.
//! See: https://api.semanticscholar.org/

use crate::utils::http::{rate_limiters, with_retry, RetryConfig};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

/// Rate limit: Semantic Scholar allows 100 requests/5 minutes without API key
const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize)]
struct PaperResponse {
    #[serde(rename = "openAccessPdf")]
    open_access_pdf: Option<OpenAccessPdf>,
}

#[derive(Debug, Deserialize)]
struct OpenAccessPdf {
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: Option<Vec<PaperResponse>>,
}

/// Client for the Semantic Scholar API
pub struct SemanticScholarClient {
    client: Client,
    api_key: Option<String>,
}

impl SemanticScholarClient {
    /// Create a new Semantic Scholar client
    ///
    /// # Arguments
    /// * `api_key` - Optional API key for higher rate limits
    pub fn new(api_key: Option<String>) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let api_key = api_key.or_else(|| std::env::var("SEMANTIC_SCHOLAR_API_KEY").ok());

        Ok(Self { client, api_key })
    }

    /// Create a new client with an existing reqwest client
    pub fn with_client(client: Client, api_key: Option<String>) -> Self {
        let api_key = api_key.or_else(|| std::env::var("SEMANTIC_SCHOLAR_API_KEY").ok());
        Self { client, api_key }
    }

    /// Build a request with optional API key header
    fn build_request(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.get(url);
        if let Some(ref key) = self.api_key {
            req = req.header("x-api-key", key);
        }
        req
    }

    /// Look up an open access PDF URL by DOI
    ///
    /// # Arguments
    /// * `doi` - The DOI to look up
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL if found
    /// * `None` - If no open access PDF is available
    pub async fn find_pdf_by_doi(&self, doi: &str) -> Option<String> {
        // Wait for rate limit slot
        rate_limiters::SEMANTIC_SCHOLAR
            .wait_for_slot("semantic_scholar")
            .await;

        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/DOI:{}?fields=openAccessPdf",
            doi
        );

        debug!("Semantic Scholar DOI lookup: {}", doi);

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        };

        let result = self.fetch_with_retry(&url, &retry_config).await;

        let data: PaperResponse = match result {
            Ok(d) => d,
            Err(e) => {
                warn!("Semantic Scholar DOI lookup failed: {}", e);
                return None;
            }
        };

        if let Some(pdf) = data.open_access_pdf {
            if let Some(url) = pdf.url {
                debug!("Found PDF via Semantic Scholar DOI: {}", url);
                return Some(url);
            }
        }

        debug!("No open access PDF found via Semantic Scholar for DOI: {}", doi);
        None
    }

    /// Search for a paper by title and return open access PDF URL
    ///
    /// # Arguments
    /// * `title` - The paper title to search for
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL if found
    /// * `None` - If no matching paper or PDF is available
    pub async fn find_pdf_by_title(&self, title: &str) -> Option<String> {
        // Wait for rate limit slot
        rate_limiters::SEMANTIC_SCHOLAR
            .wait_for_slot("semantic_scholar")
            .await;

        let encoded_title = urlencoding::encode(title);
        let url = format!(
            "https://api.semanticscholar.org/graph/v1/paper/search?query={}&fields=openAccessPdf&limit=1",
            encoded_title
        );

        debug!("Semantic Scholar title search: {}", title);

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        };

        let data: SearchResponse = match self.fetch_with_retry(&url, &retry_config).await {
            Ok(d) => d,
            Err(e) => {
                warn!("Semantic Scholar title search failed: {}", e);
                return None;
            }
        };

        if let Some(papers) = data.data {
            if let Some(first) = papers.first() {
                if let Some(ref pdf) = first.open_access_pdf {
                    if let Some(ref url) = pdf.url {
                        debug!("Found PDF via Semantic Scholar title search: {}", url);
                        return Some(url.clone());
                    }
                }
            }
        }

        debug!("No open access PDF found via Semantic Scholar for title: {}", title);
        None
    }

    /// Fetch JSON from a URL with retry logic
    async fn fetch_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        config: &RetryConfig,
    ) -> Result<T, String> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let url_owned = url.to_string();

        with_retry(
            config,
            &format!("Semantic Scholar request to {}", url),
            || {
                let client = client.clone();
                let api_key = api_key.clone();
                let url = url_owned.clone();
                async move {
                    let mut req = client.get(&url);
                    if let Some(ref key) = api_key {
                        req = req.header("x-api-key", key);
                    }

                    let resp = req
                        .send()
                        .await
                        .map_err(|e| format!("request failed: {}", e))?;

                    if !resp.status().is_success() {
                        return Err(format!("status: {}", resp.status()));
                    }

                    resp.json::<T>()
                        .await
                        .map_err(|e| format!("parse failed: {}", e))
                }
            },
            |err| {
                // Retry on network errors and 5xx/429 status codes
                err.contains("request failed")
                    || err.contains("status: 5")
                    || err.contains("status: 429")
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_semantic_scholar_client_creation() {
        let client = SemanticScholarClient::new(None);
        assert!(client.is_ok());
    }
}
