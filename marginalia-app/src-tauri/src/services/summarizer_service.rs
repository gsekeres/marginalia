//! Summarizer service for LLM output validation
//!
//! This service handles:
//! - Requesting structured JSON output from Claude CLI
//! - Validating and parsing LLM responses
//! - Converting validated output to markdown
//! - Saving raw responses on parse failure

use crate::adapters::FileSystemAdapter;
use crate::models::{Paper, RelatedPaper};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Expected JSON output structure from Claude CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeSummaryOutput {
    pub summary: String,
    pub key_contributions: Vec<String>,
    #[serde(default)]
    pub methodology: Option<String>,
    pub main_results: Vec<String>,
    #[serde(default)]
    pub limitations: Option<String>,
    #[serde(default)]
    pub related_work: Vec<RelatedWorkEntry>,
}

/// Related work entry from Claude's structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedWorkEntry {
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub year: Option<i32>,
    pub why_related: String,
}

impl From<RelatedWorkEntry> for RelatedPaper {
    fn from(entry: RelatedWorkEntry) -> Self {
        RelatedPaper {
            title: entry.title,
            authors: entry.authors,
            year: entry.year,
            why_related: entry.why_related,
            vault_citekey: None,
        }
    }
}

/// Result of summarization attempt
pub enum SummarizationResult {
    /// Successfully parsed and validated
    Success {
        markdown: String,
        related_papers: Vec<RelatedPaper>,
    },
    /// Failed to parse - raw response saved
    ParseFailure {
        raw_response_path: String,
        error: String,
    },
    /// Claude CLI error
    CliError(String),
}

/// Summarizer service for handling LLM-based summarization
pub struct SummarizerService {
    filesystem: FileSystemAdapter,
}

impl SummarizerService {
    /// Create a new summarizer service
    pub fn new() -> Result<Self, String> {
        let filesystem = FileSystemAdapter::new()?;
        Ok(Self { filesystem })
    }

    /// Create with existing filesystem adapter
    pub fn with_filesystem(filesystem: FileSystemAdapter) -> Self {
        Self { filesystem }
    }

    /// Maximum number of retry attempts for JSON parse failures
    const MAX_RETRIES: u32 = 3;

    /// Summarize a paper and return structured output
    ///
    /// This method:
    /// 1. Calls Claude CLI with JSON output format
    /// 2. Attempts to parse the response as ClaudeSummaryOutput
    /// 3. On parse failure: retries up to MAX_RETRIES times with increasingly specific prompts
    /// 4. On success: converts to markdown and returns related papers
    /// 5. After all retries fail: saves raw response and returns error
    pub async fn summarize(
        &self,
        vault_path: &str,
        paper: &Paper,
        text: &str,
    ) -> SummarizationResult {
        info!("Summarizing paper with JSON output: {}", paper.citekey);

        let mut last_response = String::new();
        let mut last_error = String::new();

        for attempt in 1..=Self::MAX_RETRIES {
            let prompt = if attempt == 1 {
                Self::build_json_prompt(paper, text)
            } else {
                // On retry, use a more emphatic prompt about JSON format
                Self::build_retry_prompt(paper, text, attempt, &last_error)
            };

            if attempt > 1 {
                info!(
                    "Retry attempt {}/{} for paper {} (previous error: {})",
                    attempt,
                    Self::MAX_RETRIES,
                    paper.citekey,
                    last_error
                );
            }

            // Call Claude CLI
            let output = match Command::new("claude")
                .args(["--print", "-p", &prompt])
                .output()
            {
                Ok(o) => o,
                Err(e) => {
                    return SummarizationResult::CliError(format!(
                        "Failed to run Claude CLI: {}",
                        e
                    ));
                }
            };

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return SummarizationResult::CliError(format!("Claude CLI error: {}", stderr));
            }

            let response = String::from_utf8_lossy(&output.stdout).to_string();
            debug!(
                "Raw Claude response length: {} chars (attempt {})",
                response.len(),
                attempt
            );

            // Try to extract JSON from the response
            let json_str = Self::extract_json(&response);

            // Attempt to parse as structured output
            match serde_json::from_str::<ClaudeSummaryOutput>(&json_str) {
                Ok(parsed) => {
                    if attempt > 1 {
                        info!(
                            "Successfully parsed structured summary for {} on attempt {}",
                            paper.citekey, attempt
                        );
                    } else {
                        info!(
                            "Successfully parsed structured summary for {}",
                            paper.citekey
                        );
                    }

                    let markdown = Self::format_to_markdown(paper, &parsed);
                    let related_papers: Vec<RelatedPaper> =
                        parsed.related_work.into_iter().map(Into::into).collect();

                    return SummarizationResult::Success {
                        markdown,
                        related_papers,
                    };
                }
                Err(parse_error) => {
                    warn!(
                        "Failed to parse JSON response for {} (attempt {}): {}",
                        paper.citekey, attempt, parse_error
                    );
                    last_response = response;
                    last_error = parse_error.to_string();
                    // Continue to next retry attempt
                }
            }
        }

        // All retries exhausted - save raw response and return failure
        error!(
            "All {} parse attempts failed for {}: {}",
            Self::MAX_RETRIES,
            paper.citekey,
            last_error
        );

        match self
            .filesystem
            .save_raw_response(vault_path, &paper.citekey, &last_response)
        {
            Ok(path) => SummarizationResult::ParseFailure {
                raw_response_path: path,
                error: format!(
                    "JSON parse error after {} attempts: {}",
                    Self::MAX_RETRIES,
                    last_error
                ),
            },
            Err(save_error) => {
                error!("Failed to save raw response: {}", save_error);
                SummarizationResult::CliError(format!(
                    "Parse failed after {} attempts and couldn't save raw response: {}",
                    Self::MAX_RETRIES,
                    last_error
                ))
            }
        }
    }

    /// Build prompt requesting JSON output
    fn build_json_prompt(paper: &Paper, text: &str) -> String {
        format!(
            r#"You are an academic research assistant. Analyze this paper and respond with ONLY valid JSON (no markdown, no explanation, just the JSON object).

Paper: "{}" by {} ({})

Respond with this exact JSON structure:
{{
  "summary": "1-2 paragraph overview of the paper",
  "key_contributions": ["contribution 1", "contribution 2", ...],
  "methodology": "brief description of methods used (or null if not applicable)",
  "main_results": ["key finding 1", "key finding 2", ...],
  "limitations": "any limitations mentioned (or null)",
  "related_work": [
    {{
      "title": "related paper title",
      "authors": ["author 1", "author 2"],
      "year": 2020,
      "why_related": "brief explanation"
    }}
  ]
}}

Important: Return ONLY the JSON object, no other text.

Paper text:
{}"#,
            paper.title,
            paper.authors.join(", "),
            paper
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "n.d.".to_string()),
            text
        )
    }

    /// Build a retry prompt with more emphatic JSON formatting instructions
    fn build_retry_prompt(paper: &Paper, text: &str, attempt: u32, previous_error: &str) -> String {
        // Truncate paper text more aggressively on later retries to give model more room
        let max_text_len = match attempt {
            2 => 50000,
            _ => 30000,
        };
        let truncated_text = if text.len() > max_text_len {
            format!("{}...\n[TEXT TRUNCATED]", &text[..max_text_len])
        } else {
            text.to_string()
        };

        format!(
            r#"CRITICAL: Your previous response failed JSON parsing with error: "{}"

You MUST respond with ONLY a valid JSON object. No markdown code blocks, no explanations, no text before or after the JSON.

Analyze this academic paper and provide a JSON response.

Paper: "{}" by {} ({})

REQUIRED JSON FORMAT (copy this structure exactly):
{{
  "summary": "string - 1-2 paragraph overview",
  "key_contributions": ["string array - list contributions"],
  "methodology": "string or null",
  "main_results": ["string array - list results"],
  "limitations": "string or null",
  "related_work": [
    {{
      "title": "string - paper title",
      "authors": ["string array"],
      "year": 2020,
      "why_related": "string"
    }}
  ]
}}

RULES:
- Start your response with {{ and end with }}
- Use double quotes for all strings
- Arrays can be empty [] but must be valid
- null is valid for optional fields (methodology, limitations)
- Escape special characters in strings (newlines as \n, quotes as \")

Paper text:
{}"#,
            previous_error,
            paper.title,
            paper.authors.join(", "),
            paper
                .year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "n.d.".to_string()),
            truncated_text
        )
    }

    /// Extract JSON from a response that might have extra text
    fn extract_json(response: &str) -> String {
        let trimmed = response.trim();

        // If it starts with {, assume it's pure JSON
        if trimmed.starts_with('{') {
            // Find matching closing brace
            let mut depth = 0;
            let mut end_pos = 0;
            for (i, c) in trimmed.char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end_pos = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end_pos > 0 {
                return trimmed[..end_pos].to_string();
            }
        }

        // Try to find JSON in code blocks
        if let Some(start) = trimmed.find("```json") {
            let json_start = start + 7;
            if let Some(end) = trimmed[json_start..].find("```") {
                return trimmed[json_start..json_start + end].trim().to_string();
            }
        }

        // Try to find JSON between ``` markers
        if let Some(start) = trimmed.find("```") {
            let content_start = start + 3;
            if let Some(end) = trimmed[content_start..].find("```") {
                let content = trimmed[content_start..content_start + end].trim();
                // Skip language identifier if present
                if let Some(newline) = content.find('\n') {
                    let after_lang = &content[newline + 1..];
                    if after_lang.trim().starts_with('{') {
                        return after_lang.trim().to_string();
                    }
                }
                return content.to_string();
            }
        }

        // Return as-is if no special handling needed
        trimmed.to_string()
    }

    /// Format parsed output to markdown with frontmatter
    fn format_to_markdown(paper: &Paper, output: &ClaudeSummaryOutput) -> String {
        let mut md = format!(
            r#"---
title: "{}"
authors: {:?}
year: {}
journal: "{}"
citekey: "{}"
doi: "{}"
status: "summarized"
---

## Summary

{}

## Key Contributions

"#,
            paper.title,
            paper.authors,
            paper.year.unwrap_or(0),
            paper.journal.as_deref().unwrap_or(""),
            paper.citekey,
            paper.doi.as_deref().unwrap_or(""),
            output.summary
        );

        for contrib in &output.key_contributions {
            md.push_str(&format!("- {}\n", contrib));
        }

        if let Some(methodology) = &output.methodology {
            md.push_str(&format!("\n## Methodology\n\n{}\n", methodology));
        }

        md.push_str("\n## Main Results\n\n");
        for result in &output.main_results {
            md.push_str(&format!("- {}\n", result));
        }

        if let Some(limitations) = &output.limitations {
            md.push_str(&format!("\n## Limitations\n\n{}\n", limitations));
        }

        if !output.related_work.is_empty() {
            md.push_str("\n## Related Work\n\n");
            for related in &output.related_work {
                md.push_str(&format!("- **{}**", related.title));
                if !related.authors.is_empty() {
                    md.push_str(&format!(" by {}", related.authors.join(", ")));
                }
                if let Some(year) = related.year {
                    md.push_str(&format!(" ({})", year));
                }
                md.push_str(&format!("\n  - {}\n", related.why_related));
            }
        }

        md
    }
}

impl Default for SummarizerService {
    fn default() -> Self {
        Self::new().expect("Failed to create SummarizerService")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_pure() {
        let input = r#"{"summary": "test", "key_contributions": []}"#;
        let result = SummarizerService::extract_json(input);
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
    }

    #[test]
    fn test_extract_json_with_markdown() {
        let input = r#"Here's the JSON:
```json
{"summary": "test", "key_contributions": []}
```"#;
        let result = SummarizerService::extract_json(input);
        assert!(result.starts_with('{'));
    }

    #[test]
    fn test_parse_valid_output() {
        let json = r#"{
            "summary": "This paper explores...",
            "key_contributions": ["contrib 1", "contrib 2"],
            "methodology": "Survey method",
            "main_results": ["finding 1"],
            "limitations": null,
            "related_work": []
        }"#;

        let parsed: Result<ClaudeSummaryOutput, _> = serde_json::from_str(json);
        assert!(parsed.is_ok());
        let output = parsed.unwrap();
        assert_eq!(output.key_contributions.len(), 2);
    }

    #[test]
    fn test_related_work_conversion() {
        let entry = RelatedWorkEntry {
            title: "Test Paper".to_string(),
            authors: vec!["Smith".to_string()],
            year: Some(2020),
            why_related: "Foundation".to_string(),
        };

        let related: RelatedPaper = entry.into();
        assert_eq!(related.title, "Test Paper");
        assert_eq!(related.year, Some(2020));
    }

    #[test]
    fn test_retry_prompt_includes_error() {
        let mut paper = Paper::new("test2024".to_string(), "Test Paper".to_string());
        paper.authors = vec!["Author".to_string()];
        paper.year = Some(2024);

        let error = "missing field `summary`";
        let prompt = SummarizerService::build_retry_prompt(&paper, "paper text", 2, error);

        // Check that the error is included in the retry prompt
        assert!(prompt.contains(error));
        assert!(prompt.contains("CRITICAL"));
        assert!(prompt.contains("Test Paper"));
    }

    #[test]
    fn test_retry_prompt_truncates_long_text() {
        let mut paper = Paper::new("test2024".to_string(), "Test Paper".to_string());
        paper.authors = vec!["Author".to_string()];
        paper.year = Some(2024);

        // Create a very long text
        let long_text = "a".repeat(100000);
        let prompt = SummarizerService::build_retry_prompt(&paper, &long_text, 3, "error");

        // On attempt 3, should truncate to 30000 chars + truncation message
        assert!(prompt.contains("[TEXT TRUNCATED]"));
        assert!(prompt.len() < 35000); // Some overhead for the prompt template
    }
}
