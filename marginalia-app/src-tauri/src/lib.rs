mod models;
mod commands;
mod utils;
pub mod storage;
pub mod services;
pub mod adapters;

use std::sync::Mutex;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_appender::rolling;

/// Application state shared across commands
pub struct AppState {
    /// Current vault path (if open)
    pub vault_path: Mutex<Option<PathBuf>>,
    /// Database connection for the current vault
    pub db: Mutex<Option<storage::Database>>,
    /// Log directory path
    pub log_dir: PathBuf,
}

impl AppState {
    fn new(log_dir: PathBuf) -> Self {
        Self {
            vault_path: Mutex::new(None),
            db: Mutex::new(None),
            log_dir,
        }
    }
}

/// Get the application log directory
fn get_log_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.marginalia.app")
        .join("logs")
}

/// Set up logging/tracing with file output
fn setup_logging(log_dir: &PathBuf) {
    // Ensure log directory exists
    std::fs::create_dir_all(log_dir).ok();

    // Create a daily rolling file appender
    let file_appender = rolling::daily(log_dir, "marginalia.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the application
    // by leaking it (the app runs until exit anyway)
    std::mem::forget(_guard);

    // Set up subscriber with both console and file output
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("marginalia=info,warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_target(false)
                .compact()
        )
        .with(
            fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(non_blocking)
        )
        .init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Get log directory
    let log_dir = get_log_dir();

    // Initialize logging
    setup_logging(&log_dir);

    info!("Marginalia starting, log directory: {:?}", log_dir);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new(log_dir))
        .setup(|_app| {
            // Check Claude CLI availability on startup
            let claude_available = utils::claude::is_claude_available();
            info!("Claude CLI available: {}", claude_available);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Vault commands
            commands::vault::open_vault,
            commands::vault::create_vault,
            commands::vault::get_recent_vaults,
            commands::vault::save_index,
            commands::vault::add_recent_vault,
            commands::vault::get_vault_stats,
            commands::vault::scan_vault_files,
            commands::vault::find_bib_files,
            // Paper commands
            commands::papers::get_papers,
            commands::papers::get_paper,
            commands::papers::get_stats,
            commands::papers::update_paper_status,
            commands::papers::search_papers,
            commands::papers::add_related_paper,
            // Import/Export commands
            commands::import::import_bibtex,
            commands::import::export_bibtex,
            // PDF finder commands
            commands::pdf_finder::find_pdf,
            commands::pdf_finder::download_pdf,
            // Claude/Summarizer commands
            commands::claude::check_claude_cli,
            commands::claude::summarize_paper,
            commands::claude::read_raw_response,
            // Notes commands
            commands::notes::get_notes,
            commands::notes::save_notes,
            commands::notes::add_highlight,
            commands::notes::delete_highlight,
            // Graph commands
            commands::graph::get_graph,
            commands::graph::connect_papers,
            commands::graph::disconnect_papers,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::save_settings,
            // Job commands
            commands::jobs::start_job,
            commands::jobs::get_job,
            commands::jobs::list_jobs,
            commands::jobs::list_active_jobs,
            commands::jobs::cancel_job,
            commands::jobs::update_job_progress,
            // Diagnostic commands
            commands::diagnostics::run_diagnostics,
            commands::diagnostics::get_log_path,
            commands::diagnostics::open_log_folder,
            // Project commands
            commands::projects::list_projects,
            commands::projects::get_project,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::delete_project,
            commands::projects::add_paper_to_project,
            commands::projects::remove_paper_from_project,
            commands::projects::get_project_papers,
            commands::projects::get_paper_projects,
            commands::projects::set_paper_projects,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
