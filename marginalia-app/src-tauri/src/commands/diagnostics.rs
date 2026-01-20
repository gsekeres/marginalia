//! Diagnostic commands
//!
//! Commands for troubleshooting and debugging.

use crate::adapters::ClaudeCliClient;
use crate::AppState;
use std::fs;
use std::process::Command;
use tauri::State;
use tracing::info;

/// Diagnostic result with system checks
#[derive(serde::Serialize)]
pub struct DiagnosticResult {
    /// Whether the vault directory is writable
    pub vault_writable: bool,
    /// Database connection status
    pub db_status: String,
    /// Claude CLI status
    pub claude_cli: ClaudeCliStatus,
    /// Network connectivity
    pub network: bool,
    /// Log directory path
    pub log_path: String,
    /// Application version
    pub app_version: String,
    /// OS version
    pub os_version: String,
}

#[derive(serde::Serialize)]
pub struct ClaudeCliStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub logged_in: bool,
}

/// Run diagnostic checks
#[tauri::command]
pub async fn run_diagnostics(
    vault_path: Option<String>,
    state: State<'_, AppState>,
) -> Result<DiagnosticResult, String> {
    info!("Running diagnostics");

    // Check vault writability
    let vault_writable = if let Some(ref path) = vault_path {
        let test_file = format!("{}/.marginalia_test", path);
        let writable = fs::write(&test_file, "test").is_ok();
        if writable {
            fs::remove_file(&test_file).ok();
        }
        writable
    } else {
        false
    };

    // Check database status
    let db_status = {
        let db_guard = state.db.lock().map_err(|e| e.to_string())?;
        match db_guard.as_ref() {
            Some(db) => {
                // Try a simple query
                match db.conn.execute("SELECT 1", []) {
                    Ok(_) => "Connected".to_string(),
                    Err(e) => format!("Error: {}", e),
                }
            }
            None => "Not connected".to_string(),
        }
    };

    // Check Claude CLI
    let claude_installed = ClaudeCliClient::is_available();
    let claude_version = if claude_installed {
        ClaudeCliClient::get_version()
    } else {
        None
    };
    let claude_logged_in = if claude_installed {
        ClaudeCliClient::is_logged_in()
    } else {
        false
    };

    // Check network connectivity (try to reach a reliable endpoint)
    let network = check_network_connectivity().await;

    // Get log path
    let log_path = state.log_dir.to_string_lossy().to_string();

    // Get OS version
    let os_version = get_os_version();

    Ok(DiagnosticResult {
        vault_writable,
        db_status,
        claude_cli: ClaudeCliStatus {
            installed: claude_installed,
            version: claude_version,
            logged_in: claude_logged_in,
        },
        network,
        log_path,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os_version,
    })
}

/// Get the log file path
#[tauri::command]
pub async fn get_log_path(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.log_dir.to_string_lossy().to_string())
}

/// Open the log folder in Finder
#[tauri::command]
pub async fn open_log_folder(state: State<'_, AppState>) -> Result<(), String> {
    let log_dir = &state.log_dir;

    // Ensure directory exists
    fs::create_dir_all(log_dir)
        .map_err(|e| format!("Failed to create log directory: {}", e))?;

    // Open in Finder (macOS)
    Command::new("open")
        .arg(log_dir)
        .spawn()
        .map_err(|e| format!("Failed to open log folder: {}", e))?;

    info!("Opened log folder: {:?}", log_dir);
    Ok(())
}

/// Check network connectivity
async fn check_network_connectivity() -> bool {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Try to reach a reliable endpoint
    client
        .head("https://api.unpaywall.org")
        .send()
        .await
        .map(|r| r.status().is_success() || r.status().as_u16() == 400)
        .unwrap_or(false)
}

/// Get OS version string
fn get_os_version() -> String {
    let output = Command::new("sw_vers")
        .args(["-productVersion"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            format!("macOS {}", String::from_utf8_lossy(&o.stdout).trim())
        }
        _ => "macOS (unknown version)".to_string(),
    }
}
