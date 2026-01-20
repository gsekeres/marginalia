//! Graph commands for paper network visualization

use crate::models::PaperStatus;
use crate::storage::{PaperRepo, ConnectionRepo};
use crate::AppState;
use tauri::State;
use tracing::info;

#[derive(serde::Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub title: String,
    pub group: String,
    pub year: Option<i32>,
}

#[derive(serde::Serialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub title: String,
}

#[derive(serde::Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[tauri::command]
pub async fn get_graph(
    vault_path: String,
    state: State<'_, AppState>,
) -> Result<GraphData, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    let conn_repo = ConnectionRepo::new(&db.conn);

    let papers = paper_repo.get_all()
        .map_err(|e| format!("Failed to get papers: {}", e))?;

    let connections = conn_repo.get_all()
        .map_err(|e| format!("Failed to get connections: {}", e))?;

    let nodes: Vec<GraphNode> = papers.values().map(|p| {
        let group = match p.status {
            PaperStatus::Discovered => "discovered",
            PaperStatus::Wanted => "wanted",
            PaperStatus::Queued => "queued",
            PaperStatus::Downloaded => "downloaded",
            PaperStatus::Summarized => "summarized",
            PaperStatus::Failed => "failed",
        };

        GraphNode {
            id: p.citekey.clone(),
            label: p.citekey.clone(),
            title: p.title.clone(),
            group: group.to_string(),
            year: p.year,
        }
    }).collect();

    let edges: Vec<GraphEdge> = connections.iter().map(|c| {
        GraphEdge {
            from: c.source.clone(),
            to: c.target.clone(),
            title: c.reason.clone(),
        }
    }).collect();

    Ok(GraphData { nodes, edges })
}

#[tauri::command]
pub async fn connect_papers(
    vault_path: String,
    source: String,
    target: String,
    reason: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let paper_repo = PaperRepo::new(&db.conn);
    let conn_repo = ConnectionRepo::new(&db.conn);

    // Verify both papers exist
    if !paper_repo.exists(&source).unwrap_or(false) {
        return Err(format!("Paper not found: {}", source));
    }
    if !paper_repo.exists(&target).unwrap_or(false) {
        return Err(format!("Paper not found: {}", target));
    }

    // Check if connection already exists (in either direction)
    if conn_repo.exists(&source, &target).unwrap_or(false) ||
       conn_repo.exists(&target, &source).unwrap_or(false) {
        return Ok("exists".to_string());
    }

    // Add connection
    conn_repo.add(&source, &target, &reason)
        .map_err(|e| format!("Failed to create connection: {}", e))?;

    info!("Connected {} to {} ({})", source, target, reason);
    Ok("connected".to_string())
}

#[tauri::command]
pub async fn disconnect_papers(
    vault_path: String,
    source: String,
    target: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let conn_repo = ConnectionRepo::new(&db.conn);

    // Try to remove in both directions
    let removed1 = conn_repo.remove(&source, &target)
        .map_err(|e| format!("Failed to remove connection: {}", e))?;
    let removed2 = conn_repo.remove(&target, &source)
        .map_err(|e| format!("Failed to remove connection: {}", e))?;

    if removed1 == 0 && removed2 == 0 {
        return Err("Connection not found".to_string());
    }

    info!("Disconnected {} from {}", source, target);
    Ok(())
}
