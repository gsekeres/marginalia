//! HTTP utilities with retry logic and rate limiting
//!
//! Provides exponential backoff, rate limiting, and PDF validation for HTTP requests.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// PDF magic bytes: "%PDF-"
const PDF_MAGIC: &[u8] = b"%PDF-";

/// Rate limiter for API endpoints
pub struct RateLimiter {
    /// Window size in seconds
    window_secs: u64,
    /// Maximum requests per window
    max_requests: u32,
    /// Request timestamps per endpoint
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `window_secs` - Time window in seconds
    /// * `max_requests` - Maximum requests allowed per window
    pub fn new(window_secs: u64, max_requests: u32) -> Self {
        Self {
            window_secs,
            max_requests,
            requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a request can be made and record it
    ///
    /// # Arguments
    /// * `endpoint` - The endpoint identifier (e.g., "unpaywall", "semantic_scholar")
    ///
    /// # Returns
    /// * `true` if request is allowed
    /// * `false` if rate limit exceeded
    pub async fn check_and_record(&self, endpoint: &str) -> bool {
        let mut requests = self.requests.lock().await;
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        let timestamps = requests.entry(endpoint.to_string()).or_insert_with(Vec::new);

        // Remove timestamps outside the window
        timestamps.retain(|t| now.duration_since(*t) < window);

        if timestamps.len() >= self.max_requests as usize {
            debug!(
                "Rate limit hit for {}: {} requests in {} seconds",
                endpoint,
                timestamps.len(),
                self.window_secs
            );
            return false;
        }

        timestamps.push(now);
        true
    }

    /// Wait until a request can be made (blocking rate limiter)
    ///
    /// # Arguments
    /// * `endpoint` - The endpoint identifier
    pub async fn wait_for_slot(&self, endpoint: &str) {
        loop {
            if self.check_and_record(endpoint).await {
                return;
            }
            // Wait a bit before checking again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Get the time until next available slot
    ///
    /// # Arguments
    /// * `endpoint` - The endpoint identifier
    ///
    /// # Returns
    /// * Duration until next slot is available, or None if slot available now
    pub async fn time_until_slot(&self, endpoint: &str) -> Option<Duration> {
        let requests = self.requests.lock().await;
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        if let Some(timestamps) = requests.get(endpoint) {
            let valid_timestamps: Vec<_> = timestamps
                .iter()
                .filter(|t| now.duration_since(**t) < window)
                .collect();

            if valid_timestamps.len() >= self.max_requests as usize {
                // Find oldest timestamp and calculate when it will expire
                if let Some(oldest) = valid_timestamps.iter().min() {
                    let elapsed = now.duration_since(**oldest);
                    if elapsed < window {
                        return Some(window - elapsed);
                    }
                }
            }
        }
        None
    }
}

/// Configuration for retry behavior
#[derive(Clone)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier (exponential factor)
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration for a given attempt
    ///
    /// # Arguments
    /// * `attempt` - The attempt number (0-indexed)
    ///
    /// # Returns
    /// Duration to wait before next retry
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let backoff_ms =
            self.initial_backoff.as_millis() as f64 * self.multiplier.powi(attempt as i32);
        let backoff = Duration::from_millis(backoff_ms as u64);
        backoff.min(self.max_backoff)
    }
}

/// Execute an async function with exponential backoff retry
///
/// # Arguments
/// * `config` - Retry configuration
/// * `operation` - The async operation to retry
/// * `should_retry` - Function to determine if error is retryable
///
/// # Type Parameters
/// * `T` - Success type
/// * `E` - Error type (must implement Display)
/// * `F` - Future type
/// * `Fut` - Future that returns Result<T, E>
/// * `R` - Should-retry predicate
pub async fn with_retry<T, E, F, Fut, R>(
    config: &RetryConfig,
    operation_name: &str,
    mut operation: F,
    should_retry: R,
) -> Result<T, E>
where
    E: std::fmt::Display,
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    R: Fn(&E) -> bool,
{
    let mut last_error: Option<E> = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!("{} succeeded on attempt {}", operation_name, attempt + 1);
                }
                return Ok(result);
            }
            Err(e) => {
                if attempt < config.max_retries && should_retry(&e) {
                    let backoff = config.backoff_for_attempt(attempt);
                    warn!(
                        "{} failed (attempt {}): {}. Retrying in {:?}",
                        operation_name,
                        attempt + 1,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Should never reach here, but handle it gracefully
    Err(last_error.expect("retry loop should have returned"))
}

/// Validate that bytes represent a PDF file
///
/// # Arguments
/// * `bytes` - The bytes to validate
///
/// # Returns
/// * `true` if bytes start with PDF magic bytes
/// * `false` otherwise
pub fn is_valid_pdf(bytes: &[u8]) -> bool {
    bytes.len() >= PDF_MAGIC.len() && &bytes[..PDF_MAGIC.len()] == PDF_MAGIC
}

/// Check if a response might be a login/paywall redirect
///
/// # Arguments
/// * `content_type` - The Content-Type header value
/// * `bytes` - The response bytes (first few KB is enough)
///
/// # Returns
/// * `true` if response appears to be HTML (likely login page)
/// * `false` if response appears to be a PDF
pub fn is_likely_login_page(content_type: Option<&str>, bytes: &[u8]) -> bool {
    // Check content type
    if let Some(ct) = content_type {
        if ct.contains("text/html") {
            return true;
        }
    }

    // Check for HTML content in bytes
    if bytes.len() >= 15 {
        let start = String::from_utf8_lossy(&bytes[..15.min(bytes.len())]).to_lowercase();
        if start.contains("<!doctype") || start.contains("<html") {
            return true;
        }
    }

    false
}

/// Default rate limiters for common APIs
pub mod rate_limiters {
    use super::RateLimiter;
    use once_cell::sync::Lazy;

    /// Unpaywall: 100,000 requests per day ≈ 70 per minute to be safe
    pub static UNPAYWALL: Lazy<RateLimiter> = Lazy::new(|| RateLimiter::new(60, 70));

    /// Semantic Scholar: 100 requests per 5 minutes (without API key)
    pub static SEMANTIC_SCHOLAR: Lazy<RateLimiter> = Lazy::new(|| RateLimiter::new(300, 100));

    /// arXiv: Be conservative, 1 request per 3 seconds
    pub static ARXIV: Lazy<RateLimiter> = Lazy::new(|| RateLimiter::new(3, 1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_pdf() {
        assert!(is_valid_pdf(b"%PDF-1.4"));
        assert!(is_valid_pdf(b"%PDF-2.0 some content"));
        assert!(!is_valid_pdf(b"<!DOCTYPE html>"));
        assert!(!is_valid_pdf(b"<html>"));
        assert!(!is_valid_pdf(b""));
        assert!(!is_valid_pdf(b"%PD")); // Too short
    }

    #[test]
    fn test_is_likely_login_page() {
        assert!(is_likely_login_page(Some("text/html"), b""));
        assert!(is_likely_login_page(None, b"<!DOCTYPE html><html>"));
        assert!(is_likely_login_page(None, b"<html><head>"));
        assert!(!is_likely_login_page(Some("application/pdf"), b"%PDF-1.4"));
        assert!(!is_likely_login_page(None, b"%PDF-1.4"));
    }

    #[test]
    fn test_retry_config_backoff() {
        let config = RetryConfig::default();
        assert_eq!(config.backoff_for_attempt(0), Duration::from_millis(500));
        assert_eq!(config.backoff_for_attempt(1), Duration::from_millis(1000));
        assert_eq!(config.backoff_for_attempt(2), Duration::from_millis(2000));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(1, 2);

        // First two should succeed
        assert!(limiter.check_and_record("test").await);
        assert!(limiter.check_and_record("test").await);

        // Third should fail
        assert!(!limiter.check_and_record("test").await);

        // Wait for window to expire
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Should succeed again
        assert!(limiter.check_and_record("test").await);
    }
}
