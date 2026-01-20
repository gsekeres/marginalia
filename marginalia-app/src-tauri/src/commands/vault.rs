//! Vault management commands
//!
//! Handles opening, creating, and managing vaults with SQLite storage.

use std::path::PathBuf;
use std::fs;
use tauri::State;
use tracing::{info, warn};

use crate::models::{VaultIndex, VaultStats, RecentVault, AppSettings, PaperStatus};
use crate::storage::{open_database, Database, PaperRepo};
use crate::AppState;

use chrono::Utc;

/// Open an existing vault, migrating from JSON if needed
#[tauri::command]
pub async fn open_vault(
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultIndex, String> {
    let vault_path = PathBuf::from(&path);

    if !vault_path.exists() {
        return Err(format!("Vault path does not exist: {}", path));
    }

    info!("Opening vault at {:?}", vault_path);

    // Open or create the database, which handles migration from JSON
    let db = open_database(&vault_path)
        .map_err(|e| format!("Failed to open database: {}", e))?;

    // Get all papers and connections from the database
    let paper_repo = PaperRepo::new(&db.conn);
    let papers = paper_repo.get_all()
        .map_err(|e| format!("Failed to load papers: {}", e))?;

    let conn_repo = crate::storage::ConnectionRepo::new(&db.conn);
    let connections = conn_repo.get_all()
        .map_err(|e| format!("Failed to load connections: {}", e))?;

    // Build VaultIndex for compatibility with frontend
    let index = VaultIndex {
        papers,
        connections,
        last_updated: Utc::now(),
        source_bib_path: None,
    };

    // Store database in app state
    {
        let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
        *db_guard = Some(db);
    }
    {
        let mut path_guard = state.vault_path.lock().map_err(|e| e.to_string())?;
        *path_guard = Some(vault_path);
    }

    info!("Vault opened with {} papers", index.papers.len());
    Ok(index)
}

/// Create a new vault
#[tauri::command]
pub async fn create_vault(
    path: String,
    state: State<'_, AppState>,
) -> Result<VaultIndex, String> {
    let vault_path = PathBuf::from(&path);

    // Create vault directory structure
    fs::create_dir_all(&vault_path)
        .map_err(|e| format!("Failed to create vault directory: {}", e))?;

    let papers_path = vault_path.join("papers");
    fs::create_dir_all(&papers_path)
        .map_err(|e| format!("Failed to create papers directory: {}", e))?;

    info!("Created vault at {:?}", vault_path);

    // Open the database (this will create it and run migrations)
    let db = open_database(&vault_path)
        .map_err(|e| format!("Failed to create database: {}", e))?;

    // Store in app state
    {
        let mut db_guard = state.db.lock().map_err(|e| e.to_string())?;
        *db_guard = Some(db);
    }
    {
        let mut path_guard = state.vault_path.lock().map_err(|e| e.to_string())?;
        *path_guard = Some(vault_path);
    }

    Ok(VaultIndex::new())
}

/// Get list of recently opened vaults
#[tauri::command]
pub async fn get_recent_vaults() -> Result<Vec<RecentVault>, String> {
    let app_support = dirs::data_dir()
        .ok_or("Could not find app support directory")?
        .join("com.marginalia");

    let settings_path = app_support.join("settings.json");

    if !settings_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;
    let settings: AppSettings = serde_json::from_str(&content)
        .unwrap_or_default();

    Ok(settings.recent_vaults)
}

/// Save the index (for backward compatibility - now syncs to database)
#[tauri::command]
pub async fn save_index(
    _path: String,
    index: VaultIndex,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Get the database from state
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);

    // Update each paper in the database
    for (_, paper) in &index.papers {
        if paper_repo.exists(&paper.citekey).unwrap_or(false) {
            paper_repo.update(paper)
                .map_err(|e| format!("Failed to update paper {}: {}", paper.citekey, e))?;
        } else {
            paper_repo.insert(paper)
                .map_err(|e| format!("Failed to insert paper {}: {}", paper.citekey, e))?;
        }
    }

    // Update connections
    let conn_repo = crate::storage::ConnectionRepo::new(&db.conn);
    for connection in &index.connections {
        conn_repo.add(&connection.source, &connection.target, &connection.reason)
            .map_err(|e| format!("Failed to save connection: {}", e))?;
    }

    info!("Saved index with {} papers to database", index.papers.len());
    Ok(())
}

/// Add a vault to the recent vaults list
#[tauri::command]
pub async fn add_recent_vault(path: String, paper_count: usize) -> Result<(), String> {
    let app_support = dirs::data_dir()
        .ok_or("Could not find app support directory")?
        .join("com.marginalia");

    fs::create_dir_all(&app_support)
        .map_err(|e| format!("Failed to create app support directory: {}", e))?;

    let settings_path = app_support.join("settings.json");

    let mut settings = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Failed to read settings: {}", e))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        AppSettings::default()
    };

    let vault_path = PathBuf::from(&path);
    let name = vault_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Vault".to_string());

    // Remove existing entry for this path
    settings.recent_vaults.retain(|v| v.path != vault_path);

    // Add new entry at the front
    settings.recent_vaults.insert(
        0,
        RecentVault {
            path: vault_path.clone(),
            name,
            last_opened: Utc::now(),
            paper_count,
        },
    );

    // Keep only last 10 recent vaults
    settings.recent_vaults.truncate(10);

    // Update last vault path
    settings.last_vault_path = Some(vault_path);

    // Save settings
    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    fs::write(&settings_path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    Ok(())
}

/// Get vault statistics
#[tauri::command]
pub async fn get_vault_stats(
    _vault_path: String,
    state: State<'_, AppState>,
) -> Result<VaultStats, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    paper_repo.stats()
        .map_err(|e| format!("Failed to get stats: {}", e))
}

/// Scan vault folder for existing PDFs and summaries, updating the database
#[tauri::command]
pub async fn scan_vault_files(
    path: String,
    state: State<'_, AppState>,
) -> Result<ScanResult, String> {
    let vault_path = PathBuf::from(&path);
    let papers_path = vault_path.join("papers");

    // Get the database
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    let mut updated = 0;

    // Scan papers directory
    if papers_path.exists() {
        if let Ok(entries) = fs::read_dir(&papers_path) {
            for entry in entries.flatten() {
                let citekey = entry.file_name().to_string_lossy().to_string();
                let paper_dir = entry.path();

                if !paper_dir.is_dir() {
                    continue;
                }

                // Check if paper exists in database
                if let Ok(Some(mut paper)) = paper_repo.get(&citekey) {
                    let pdf_path = paper_dir.join("paper.pdf");
                    let summary_path = paper_dir.join("summary.md");

                    let mut changed = false;

                    // Check for PDF
                    if pdf_path.exists() && paper.pdf_path.is_none() {
                        paper.pdf_path = Some(format!("papers/{}/paper.pdf", citekey));
                        if paper.status == PaperStatus::Discovered {
                            paper.status = PaperStatus::Downloaded;
                        }
                        changed = true;
                    }

                    // Check for summary
                    if summary_path.exists() && paper.summary_path.is_none() {
                        paper.summary_path = Some(format!("papers/{}/summary.md", citekey));
                        paper.status = PaperStatus::Summarized;
                        changed = true;
                    }

                    if changed {
                        paper_repo.update(&paper)
                            .map_err(|e| format!("Failed to update paper: {}", e))?;
                        updated += 1;
                    }
                }
            }
        }
    }

    // Get updated index
    let papers = paper_repo.get_all()
        .map_err(|e| format!("Failed to get papers: {}", e))?;

    let conn_repo = crate::storage::ConnectionRepo::new(&db.conn);
    let connections = conn_repo.get_all()
        .map_err(|e| format!("Failed to get connections: {}", e))?;

    let index = VaultIndex {
        papers,
        connections,
        last_updated: Utc::now(),
        source_bib_path: None,
    };

    info!("Scanned vault files, updated {} papers", updated);

    Ok(ScanResult {
        updated,
        index,
    })
}

#[derive(serde::Serialize)]
pub struct ScanResult {
    pub updated: usize,
    pub index: VaultIndex,
}

/// Find .bib files in the vault root directory
#[tauri::command]
pub async fn find_bib_files(path: String) -> Result<Vec<String>, String> {
    let vault_path = PathBuf::from(&path);

    let mut bib_files = Vec::new();

    if let Ok(entries) = fs::read_dir(&vault_path) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            if file_path.is_file() {
                if let Some(ext) = file_path.extension() {
                    if ext == "bib" {
                        bib_files.push(file_path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    Ok(bib_files)
}
