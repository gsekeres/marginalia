//! Claude CLI wrapper
//!
//! Provides integration with the Claude Code CLI for:
//! - PDF search via web browsing
//! - Paper summarization with structured output

use crate::models::{Paper, RelatedPaper};
use std::process::Command;
use tracing::{debug, info, warn};

/// Claude CLI client for LLM-powered operations
pub struct ClaudeCliClient;

impl ClaudeCliClient {
    /// Create a new Claude CLI client
    pub fn new() -> Self {
        Self
    }

    /// Check if Claude CLI is installed and available
    pub fn is_available() -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get Claude CLI version
    pub fn get_version() -> Option<String> {
        Command::new("claude")
            .arg("--version")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    /// Check if user is logged in by testing a simple command
    pub fn is_logged_in() -> bool {
        Command::new("claude")
            .args(["--print", "-p", "Say hi"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Search for a PDF URL using Claude's web browsing capabilities
    ///
    /// # Arguments
    /// * `paper` - Paper to search for
    ///
    /// # Returns
    /// * `Some(url)` - Direct PDF URL if found
    /// * `None` - If no PDF found or Claude is unavailable
    pub async fn find_pdf_url(&self, paper: &Paper) -> Option<String> {
        if !Self::is_available() {
            warn!("Claude CLI not available for PDF search");
            return None;
        }

        let prompt = format!(
            "Find a direct download URL for the open-access PDF of this academic paper. \
             Only respond with a URL that ends in .pdf, nothing else. If you cannot find one, respond with exactly 'NONE'.\n\n\
             Title: {}\nAuthors: {}\nYear: {:?}\nDOI: {:?}",
            paper.title,
            paper.authors.join(", "),
            paper.year,
            paper.doi
        );

        debug!("Claude PDF search for: {}", paper.title);

        let output = match Command::new("claude")
            .args(["--print", "-p", &prompt])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to run Claude CLI: {}", e);
                return None;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Claude CLI error: {}", stderr);
            return None;
        }

        let response = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if response.starts_with("http") && response.contains(".pdf") {
            info!("Claude found PDF URL: {}", response);
            Some(response)
        } else {
            debug!("Claude did not find PDF URL");
            None
        }
    }

    /// Summarize a paper using Claude CLI
    ///
    /// # Arguments
    /// * `paper` - Paper metadata
    /// * `text` - Extracted PDF text
    ///
    /// # Returns
    /// * `Ok((summary, related))` - Summary text and extracted related papers
    /// * `Err(error)` - If summarization fails
    pub async fn summarize_paper(
        &self,
        paper: &Paper,
        text: &str,
    ) -> Result<(String, Vec<RelatedPaper>), String> {
        if !Self::is_available() {
            return Err("Claude CLI not installed. Install with: brew install anthropics/tap/claude".to_string());
        }

        let prompt = Self::build_summary_prompt(paper, text);

        info!("Summarizing paper: {}", paper.title);

        let output = Command::new("claude")
            .args(["--print", "-p", &prompt])
            .output()
            .map_err(|e| format!("Failed to run Claude CLI: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Claude CLI error: {}", stderr));
        }

        let response = String::from_utf8_lossy(&output.stdout).to_string();

        // Extract related papers from the response
        let related_papers = Self::extract_related_papers(&response);

        info!("Successfully summarized paper: {}", paper.citekey);

        Ok((response, related_papers))
    }

    /// Build the summarization prompt
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
{}"#,
            paper.title,
            paper.authors.join(", "),
            paper.year.map(|y| y.to_string()).unwrap_or_else(|| "n.d.".to_string()),
            text
        )
    }

    /// Extract related papers from Claude's response
    fn extract_related_papers(response: &str) -> Vec<RelatedPaper> {
        let mut related = Vec::new();

        // Find Related Work section
        let Some(start) = response.find("## Related Work") else {
            return related;
        };

        let section = &response[start..];
        // Stop at next section or end
        let end = section[15..]
            .find("\n## ")
            .map(|i| i + 15)
            .unwrap_or(section.len());
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

        related
    }
}

impl Default for ClaudeCliClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_related_papers() {
        let response = r#"## Summary
This is a summary.

## Related Work
- Title: Paper One
  Authors: Smith, John and Doe, Jane
  Year: 2020
  Why Related: Foundational work
- Title: Paper Two
  Authors: Brown, Alice
  Year: 2021
  Why Related: Extension of methods

## Conclusions
End of paper.
"#;

        let related = ClaudeCliClient::extract_related_papers(response);
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].title, "Paper One");
        assert_eq!(related[0].year, Some(2020));
        assert_eq!(related[1].title, "Paper Two");
    }
}
