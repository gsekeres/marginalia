mod models;
mod commands;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|_app| {
            // Check Claude CLI availability on startup
            let claude_available = utils::claude::is_claude_available();
            println!("Claude CLI available: {}", claude_available);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
