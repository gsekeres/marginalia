//! Notes and highlights commands

use crate::models::{PaperNotes, Highlight, HighlightRect};
use crate::storage::NotesRepo;
use crate::AppState;
use std::path::PathBuf;
use std::fs;
use chrono::Utc;
use uuid::Uuid;
use tauri::State;
use tracing::info;

#[tauri::command]
pub async fn get_notes(
    vault_path: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<PaperNotes, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let notes_repo = NotesRepo::new(&db.conn);

    // Try to get from database first
    if let Ok(Some(notes)) = notes_repo.get(&citekey) {
        return Ok(notes);
    }

    // Fall back to file if not in database (for migration)
    let notes_path = PathBuf::from(&vault_path)
        .join("papers")
        .join(&citekey)
        .join("notes.json");

    if notes_path.exists() {
        let content = fs::read_to_string(&notes_path)
            .map_err(|e| format!("Failed to read notes: {}", e))?;

        let notes: PaperNotes = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse notes: {}", e))?;

        // Migrate to database
        notes_repo.save(&notes).ok();
        for highlight in &notes.highlights {
            notes_repo.add_highlight(&citekey, highlight).ok();
        }

        return Ok(notes);
    }

    // Return empty notes
    Ok(PaperNotes::new(citekey))
}

#[tauri::command]
pub async fn save_notes(
    vault_path: String,
    citekey: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let notes_repo = NotesRepo::new(&db.conn);

    // Save to database
    notes_repo.update_content(&citekey, &content)
        .map_err(|e| format!("Failed to save notes: {}", e))?;

    // Also save to file for portability
    let paper_dir = PathBuf::from(&vault_path).join("papers").join(&citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let notes_path = paper_dir.join("notes.json");

    // Get full notes from database to write to file
    let notes = notes_repo.get_or_create(&citekey)
        .map_err(|e| format!("Failed to get notes: {}", e))?;

    let json = serde_json::to_string_pretty(&notes)
        .map_err(|e| format!("Failed to serialize notes: {}", e))?;

    fs::write(&notes_path, json)
        .map_err(|e| format!("Failed to write notes: {}", e))?;

    info!("Saved notes for {}", citekey);
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
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let notes_repo = NotesRepo::new(&db.conn);

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

    // Save to database
    notes_repo.add_highlight(&citekey, &new_highlight)
        .map_err(|e| format!("Failed to add highlight: {}", e))?;

    // Also update the file
    let paper_dir = PathBuf::from(&vault_path).join("papers").join(&citekey);
    fs::create_dir_all(&paper_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let notes_path = paper_dir.join("notes.json");

    // Get full notes from database to write to file
    let notes = notes_repo.get_or_create(&citekey)
        .map_err(|e| format!("Failed to get notes: {}", e))?;

    let json = serde_json::to_string_pretty(&notes)
        .map_err(|e| format!("Failed to serialize notes: {}", e))?;

    fs::write(&notes_path, json)
        .map_err(|e| format!("Failed to write notes: {}", e))?;

    info!("Added highlight {} to {}", highlight_id, citekey);
    Ok(highlight_id)
}

#[tauri::command]
pub async fn delete_highlight(
    vault_path: String,
    citekey: String,
    highlight_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let notes_repo = NotesRepo::new(&db.conn);

    // Delete from database
    let deleted = notes_repo.delete_highlight(&citekey, &highlight_id)
        .map_err(|e| format!("Failed to delete highlight: {}", e))?;

    if !deleted {
        return Err("Highlight not found".to_string());
    }

    // Update the file
    let notes_path = PathBuf::from(&vault_path)
        .join("papers")
        .join(&citekey)
        .join("notes.json");

    if notes_path.exists() {
        // Get updated notes from database to write to file
        let notes = notes_repo.get_or_create(&citekey)
            .map_err(|e| format!("Failed to get notes: {}", e))?;

        let json = serde_json::to_string_pretty(&notes)
            .map_err(|e| format!("Failed to serialize notes: {}", e))?;

        fs::write(&notes_path, json)
            .map_err(|e| format!("Failed to write notes: {}", e))?;
    }

    info!("Deleted highlight {} from {}", highlight_id, citekey);
    Ok(())
}
