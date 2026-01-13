use crate::models::{PaperNotes, Highlight, HighlightRect};
use std::path::PathBuf;
use std::fs;
use chrono::Utc;
use uuid::Uuid;

#[tauri::command]
pub async fn get_notes(vault_path: String, citekey: String) -> Result<PaperNotes, String> {
    let notes_path = PathBuf::from(&vault_path)
        .join("papers")
        .join(&citekey)
        .join("notes.json");

    if !notes_path.exists() {
        return Ok(PaperNotes::new(citekey));
    }

    let content = fs::read_to_string(&notes_path)
        .map_err(|e| format!("Failed to read notes: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse notes: {}", e))
}

#[tauri::command]
pub async fn save_notes(
    vault_path: String,
    citekey: String,
    content: String,
) -> Result<(), String> {
    let paper_dir = PathBuf::from(&vault_path).join("papers").join(&citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let notes_path = paper_dir.join("notes.json");

    // Load existing notes or create new
    let mut notes = if notes_path.exists() {
        let existing = fs::read_to_string(&notes_path)
            .map_err(|e| format!("Failed to read notes: {}", e))?;
        serde_json::from_str(&existing).unwrap_or_else(|_| PaperNotes::new(citekey.clone()))
    } else {
        PaperNotes::new(citekey.clone())
    };

    notes.content = content;
    notes.last_modified = Utc::now();

    let json = serde_json::to_string_pretty(&notes)
        .map_err(|e| format!("Failed to serialize notes: {}", e))?;

    fs::write(&notes_path, json)
        .map_err(|e| format!("Failed to write notes: {}", e))?;

    Ok(())
}

#[derive(serde::Deserialize)]
pub struct AddHighlightRequest {
    pub page: i32,
    pub rects: Vec<HighlightRect>,
    pub text: String,
    pub color: String,
    pub note: Option<String>,
}

#[tauri::command]
pub async fn add_highlight(
    vault_path: String,
    citekey: String,
    highlight: AddHighlightRequest,
) -> Result<String, String> {
    let paper_dir = PathBuf::from(&vault_path).join("papers").join(&citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let notes_path = paper_dir.join("notes.json");

    // Load existing notes or create new
    let mut notes = if notes_path.exists() {
        let existing = fs::read_to_string(&notes_path)
            .map_err(|e| format!("Failed to read notes: {}", e))?;
        serde_json::from_str(&existing).unwrap_or_else(|_| PaperNotes::new(citekey.clone()))
    } else {
        PaperNotes::new(citekey.clone())
    };

    let highlight_id = Uuid::new_v4().to_string()[..16].to_string();

    let new_highlight = Highlight {
        id: highlight_id.clone(),
        page: highlight.page,
        rects: highlight.rects,
        text: highlight.text,
        color: highlight.color,
        note: highlight.note,
        created_at: Utc::now(),
    };

    notes.highlights.push(new_highlight);
    notes.last_modified = Utc::now();

    let json = serde_json::to_string_pretty(&notes)
        .map_err(|e| format!("Failed to serialize notes: {}", e))?;

    fs::write(&notes_path, json)
        .map_err(|e| format!("Failed to write notes: {}", e))?;

    Ok(highlight_id)
}

#[tauri::command]
pub async fn delete_highlight(
    vault_path: String,
    citekey: String,
    highlight_id: String,
) -> Result<(), String> {
    let notes_path = PathBuf::from(&vault_path)
        .join("papers")
        .join(&citekey)
        .join("notes.json");

    if !notes_path.exists() {
        return Err("Notes file not found".to_string());
    }

    let content = fs::read_to_string(&notes_path)
        .map_err(|e| format!("Failed to read notes: {}", e))?;

    let mut notes: PaperNotes = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse notes: {}", e))?;

    let original_len = notes.highlights.len();
    notes.highlights.retain(|h| h.id != highlight_id);

    if notes.highlights.len() == original_len {
        return Err("Highlight not found".to_string());
    }

    notes.last_modified = Utc::now();

    let json = serde_json::to_string_pretty(&notes)
        .map_err(|e| format!("Failed to serialize notes: {}", e))?;

    fs::write(&notes_path, json)
        .map_err(|e| format!("Failed to write notes: {}", e))?;

    Ok(())
}
