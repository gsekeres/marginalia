use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use super::paper::Paper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperConnection {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultIndex {
    #[serde(default)]
    pub papers: HashMap<String, Paper>,
    #[serde(default)]
    pub connections: Vec<PaperConnection>,
    #[serde(default = "Utc::now")]
    pub last_updated: DateTime<Utc>,
    pub source_bib_path: Option<String>,
}

impl VaultIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_paper(&mut self, paper: Paper) {
        self.papers.insert(paper.citekey.clone(), paper);
        self.last_updated = Utc::now();
    }

    pub fn get_paper(&self, citekey: &str) -> Option<&Paper> {
        self.papers.get(citekey)
    }

    pub fn get_paper_mut(&mut self, citekey: &str) -> Option<&mut Paper> {
        self.papers.get_mut(citekey)
    }

    pub fn stats(&self) -> VaultStats {
        let mut by_status = HashMap::new();
        for paper in self.papers.values() {
            let status_str = match paper.status {
                super::paper::PaperStatus::Discovered => "discovered",
                super::paper::PaperStatus::Wanted => "wanted",
                super::paper::PaperStatus::Queued => "queued",
                super::paper::PaperStatus::Downloaded => "downloaded",
                super::paper::PaperStatus::Summarized => "summarized",
                super::paper::PaperStatus::Failed => "failed",
            };
            *by_status.entry(status_str.to_string()).or_insert(0) += 1;
        }

        VaultStats {
            total: self.papers.len(),
            by_status,
            last_updated: self.last_updated.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub total: usize,
    pub by_status: HashMap<String, i32>,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentVault {
    pub path: PathBuf,
    pub name: String,
    pub last_opened: DateTime<Utc>,
    pub paper_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub recent_vaults: Vec<RecentVault>,
    pub last_vault_path: Option<PathBuf>,
    pub unpaywall_email: Option<String>,
    pub theme: Option<String>,
}
