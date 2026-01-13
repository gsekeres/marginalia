use crate::models::{VaultIndex, PaperConnection};
use std::path::PathBuf;
use std::fs;
use chrono::Utc;

const INDEX_FILENAME: &str = ".marginalia_index.json";

fn load_index(vault_path: &str) -> Result<VaultIndex, String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    if !path.exists() {
        return Ok(VaultIndex::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read index: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse index: {}", e))
}

fn save_index(vault_path: &str, index: &VaultIndex) -> Result<(), String> {
    let path = PathBuf::from(vault_path).join(INDEX_FILENAME);
    let content = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write index: {}", e))
}

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
pub async fn get_graph(vault_path: String) -> Result<GraphData, String> {
    let index = load_index(&vault_path)?;

    let nodes: Vec<GraphNode> = index.papers.values().map(|p| {
        let group = match p.status {
            crate::models::PaperStatus::Discovered => "discovered",
            crate::models::PaperStatus::Wanted => "wanted",
            crate::models::PaperStatus::Queued => "queued",
            crate::models::PaperStatus::Downloaded => "downloaded",
            crate::models::PaperStatus::Summarized => "summarized",
            crate::models::PaperStatus::Failed => "failed",
        };

        GraphNode {
            id: p.citekey.clone(),
            label: p.citekey.clone(),
            title: p.title.clone(),
            group: group.to_string(),
            year: p.year,
        }
    }).collect();

    let edges: Vec<GraphEdge> = index.connections.iter().map(|c| {
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
) -> Result<String, String> {
    let mut index = load_index(&vault_path)?;

    // Verify both papers exist
    if !index.papers.contains_key(&source) {
        return Err(format!("Paper not found: {}", source));
    }
    if !index.papers.contains_key(&target) {
        return Err(format!("Paper not found: {}", target));
    }

    // Check if connection already exists
    let exists = index.connections.iter().any(|c| {
        (c.source == source && c.target == target) ||
        (c.source == target && c.target == source)
    });

    if exists {
        return Ok("exists".to_string());
    }

    // Add connection
    index.connections.push(PaperConnection {
        source,
        target,
        reason,
        created_at: Utc::now(),
    });

    save_index(&vault_path, &index)?;

    Ok("connected".to_string())
}

#[tauri::command]
pub async fn disconnect_papers(
    vault_path: String,
    source: String,
    target: String,
) -> Result<(), String> {
    let mut index = load_index(&vault_path)?;

    let original_len = index.connections.len();

    index.connections.retain(|c| {
        !((c.source == source && c.target == target) ||
          (c.source == target && c.target == source))
    });

    if index.connections.len() == original_len {
        return Err("Connection not found".to_string());
    }

    save_index(&vault_path, &index)?;

    Ok(())
}
