use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PaperStatus {
    Discovered,
    Wanted,
    Queued,
    Downloaded,
    Summarized,
    Failed,
}

impl Default for PaperStatus {
    fn default() -> Self {
        PaperStatus::Discovered
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub citekey: String,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub year: Option<i32>,
    pub doi: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedPaper {
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub year: Option<i32>,
    #[serde(default)]
    pub why_related: String,
    pub vault_citekey: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paper {
    pub citekey: String,
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub number: Option<String>,
    pub pages: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub r#abstract: Option<String>,

    #[serde(default)]
    pub status: PaperStatus,

    pub pdf_path: Option<String>,
    pub summary_path: Option<String>,
    pub notes_path: Option<String>,

    #[serde(default = "Utc::now")]
    pub added_at: DateTime<Utc>,
    pub downloaded_at: Option<DateTime<Utc>>,
    pub summarized_at: Option<DateTime<Utc>>,

    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub cited_by: Vec<String>,
    #[serde(default)]
    pub related_papers: Vec<RelatedPaper>,

    #[serde(default)]
    pub search_attempts: i32,
    pub last_search_error: Option<String>,
    #[serde(default)]
    pub manual_download_links: Vec<String>,
}

impl Paper {
    pub fn new(citekey: String, title: String) -> Self {
        Self {
            citekey,
            title,
            authors: Vec::new(),
            year: None,
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
            cited_by: Vec::new(),
            related_papers: Vec::new(),
            search_attempts: 0,
            last_search_error: None,
            manual_download_links: Vec::new(),
        }
    }

    pub fn authors_str(&self) -> String {
        self.authors.join(", ")
    }
}
