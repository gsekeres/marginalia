use crate::models::AppSettings;
use std::path::PathBuf;
use std::fs;

fn get_settings_path() -> Result<PathBuf, String> {
    let app_support = dirs::data_dir()
        .ok_or("Could not find app support directory")?
        .join("com.marginalia");

    fs::create_dir_all(&app_support)
        .map_err(|e| format!("Failed to create app support directory: {}", e))?;

    Ok(app_support.join("settings.json"))
}

#[tauri::command]
pub async fn get_settings() -> Result<AppSettings, String> {
    let settings_path = get_settings_path()?;

    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(&settings_path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings: {}", e))
}

#[tauri::command]
pub async fn save_settings(settings: AppSettings) -> Result<(), String> {
    let settings_path = get_settings_path()?;

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&settings_path, content)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    Ok(())
}
