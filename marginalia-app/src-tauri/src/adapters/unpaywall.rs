//! Unpaywall API client
//!
//! Provides open access PDF lookup via the Unpaywall API.
//! See: https://unpaywall.org/products/api

use crate::utils::http::{rate_limiters, with_retry, RetryConfig};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

/// Rate limit: Unpaywall allows 100,000 requests/day
const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize)]
struct UnpaywallResponse {
    best_oa_location: Option<OaLocation>,
    oa_locations: Option<Vec<OaLocation>>,
}

#[derive(Debug, Deserialize)]
struct OaLocation {
    url_for_pdf: Option<String>,
}

/// Client for the Unpaywall API
pub struct UnpaywallClient {
    client: Client,
    email: String,
}

impl UnpaywallClient {
    /// Create a new Unpaywall client
    ///
    /// # Arguments
    /// * `email` - Email for API identification (required by Unpaywall TOS)
    pub fn new(email: Option<String>) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let email = email
            .or_else(|| std::env::var("UNPAYWALL_EMAIL").ok())
            .unwrap_or_else(|| "marginalia@example.com".to_string());

        Ok(Self { client, email })
    }

    /// Create a new client with an existing reqwest client
    pub fn with_client(client: Client, email: Option<String>) -> Self {
        let email = email
            .or_else(|| std::env::var("UNPAYWALL_EMAIL").ok())
            .unwrap_or_else(|| "marginalia@example.com".to_string());

        Self { client, email }
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
        rate_limiters::UNPAYWALL.wait_for_slot("unpaywall").await;

        let url = format!(
            "https://api.unpaywall.org/v2/{}?email={}",
            doi, self.email
        );

        debug!("Unpaywall lookup for DOI: {}", doi);

        let retry_config = RetryConfig {
            max_retries: 2,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
        };

        let client = self.client.clone();
        let url_owned = url.clone();

        let result = with_retry(
            &retry_config,
            &format!("Unpaywall lookup for {}", doi),
            || {
                let client = client.clone();
                let url = url_owned.clone();
                async move {
                    let resp = client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|e| format!("request failed: {}", e))?;

                    if !resp.status().is_success() {
                        return Err(format!("status: {}", resp.status()));
                    }

                    resp.json::<UnpaywallResponse>()
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
        .await;

        let data = match result {
            Ok(d) => d,
            Err(e) => {
                warn!("Unpaywall lookup failed: {}", e);
                return None;
            }
        };

        // Try best_oa_location first
        if let Some(best_loc) = data.best_oa_location {
            if let Some(pdf_url) = best_loc.url_for_pdf {
                if !pdf_url.is_empty() {
                    debug!("Found PDF via Unpaywall best_oa_location: {}", pdf_url);
                    return Some(pdf_url);
                }
            }
        }

        // Try oa_locations array
        if let Some(locations) = data.oa_locations {
            for loc in locations {
                if let Some(pdf_url) = loc.url_for_pdf {
                    if !pdf_url.is_empty() {
                        debug!("Found PDF via Unpaywall oa_locations: {}", pdf_url);
                        return Some(pdf_url);
                    }
                }
            }
        }

        debug!("No open access PDF found via Unpaywall for DOI: {}", doi);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unpaywall_client_creation() {
        let client = UnpaywallClient::new(Some("test@example.com".to_string()));
        assert!(client.is_ok());
    }
}
