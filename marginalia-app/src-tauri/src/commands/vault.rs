use std::path::PathBuf;
use std::fs;
use crate::models::{VaultIndex, RecentVault, AppSettings};
use chrono::Utc;

const INDEX_FILENAME: &str = ".marginalia_index.json";

pub struct AppState {
    pub vault_path: std::sync::Mutex<Option<PathBuf>>,
    pub index: std::sync::Mutex<VaultIndex>,
    pub settings: std::sync::Mutex<AppSettings>,
}

#[tauri::command]
pub async fn open_vault(path: String) -> Result<VaultIndex, String> {
    let vault_path = PathBuf::from(&path);

    if !vault_path.exists() {
        return Err(format!("Vault path does not exist: {}", path));
    }

    let index_path = vault_path.join(INDEX_FILENAME);

    let index = if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read index: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse index: {}", e))?
    } else {
        VaultIndex::new()
    };

    Ok(index)
}

#[tauri::command]
pub async fn create_vault(path: String) -> Result<VaultIndex, String> {
    let vault_path = PathBuf::from(&path);

    // Create vault directory structure
    fs::create_dir_all(&vault_path)
        .map_err(|e| format!("Failed to create vault directory: {}", e))?;

    let papers_path = vault_path.join("papers");
    fs::create_dir_all(&papers_path)
        .map_err(|e| format!("Failed to create papers directory: {}", e))?;

    // Create empty index
    let index = VaultIndex::new();
    let index_path = vault_path.join(INDEX_FILENAME);
    let content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    fs::write(&index_path, content)
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(index)
}

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

#[tauri::command]
pub async fn save_index(path: String, index: VaultIndex) -> Result<(), String> {
    let vault_path = PathBuf::from(&path);
    let index_path = vault_path.join(INDEX_FILENAME);

    let content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    fs::write(&index_path, content)
        .map_err(|e| format!("Failed to write index: {}", e))?;

    Ok(())
}

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

#[tauri::command]
pub async fn get_vault_stats(path: String) -> Result<crate::models::VaultStats, String> {
    let index = open_vault(path).await?;
    Ok(index.stats())
}

/// Scan vault folder for existing PDFs and summaries, updating the index
#[tauri::command]
pub async fn scan_vault_files(path: String) -> Result<ScanResult, String> {
    let vault_path = PathBuf::from(&path);
    let index_path = vault_path.join(INDEX_FILENAME);
    let papers_path = vault_path.join("papers");

    // Load existing index
    let mut index: VaultIndex = if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("Failed to read index: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse index: {}", e))?
    } else {
        VaultIndex::new()
    };

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

                // Check if paper exists in index
                if let Some(paper) = index.papers.get_mut(&citekey) {
                    let pdf_path = paper_dir.join("paper.pdf");
                    let summary_path = paper_dir.join("summary.md");

                    let mut changed = false;

                    // Check for PDF
                    if pdf_path.exists() && paper.pdf_path.is_none() {
                        paper.pdf_path = Some(format!("papers/{}/paper.pdf", citekey));
                        if paper.status == crate::models::PaperStatus::Discovered {
                            paper.status = crate::models::PaperStatus::Downloaded;
                        }
                        changed = true;
                    }

                    // Check for summary
                    if summary_path.exists() && paper.summary_path.is_none() {
                        paper.summary_path = Some(format!("papers/{}/summary.md", citekey));
                        paper.status = crate::models::PaperStatus::Summarized;
                        changed = true;
                    }

                    if changed {
                        updated += 1;
                    }
                }
            }
        }
    }

    // Save updated index
    if updated > 0 {
        let content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;
        fs::write(&index_path, content)
            .map_err(|e| format!("Failed to write index: {}", e))?;
    }

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
