use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Highlight {
    #[serde(default = "generate_id")]
    pub id: String,
    pub page: i32,
    #[serde(default)]
    pub rects: Vec<HighlightRect>,
    #[serde(default)]
    pub text: String,
    #[serde(default = "default_color")]
    pub color: String,
    pub note: Option<String>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

fn generate_id() -> String {
    Uuid::new_v4().to_string()[..16].to_string()
}

fn default_color() -> String {
    "yellow".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperNotes {
    pub citekey: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub highlights: Vec<Highlight>,
    #[serde(default = "Utc::now")]
    pub last_modified: DateTime<Utc>,
}

impl PaperNotes {
    pub fn new(citekey: String) -> Self {
        Self {
            citekey,
            content: String::new(),
            highlights: Vec::new(),
            last_modified: Utc::now(),
        }
    }
}
