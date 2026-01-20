//! Project management commands

use crate::models::project::Project;
use crate::storage::ProjectRepo;
use crate::AppState;
use tauri::State;
use tracing::info;

#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.list().map_err(|e| format!("Failed to list projects: {}", e))
}

#[tauri::command]
pub async fn get_project(id: String, state: State<'_, AppState>) -> Result<Option<Project>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.get(&id).map_err(|e| format!("Failed to get project: {}", e))
}

#[derive(serde::Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn create_project(
    request: CreateProjectRequest,
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let mut project = Project::new(request.name);
    if let Some(color) = request.color {
        project = project.with_color(color);
    }
    if let Some(description) = request.description {
        project = project.with_description(description);
    }

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.create(&project).map_err(|e| format!("Failed to create project: {}", e))?;

    info!("Created project: {} ({})", project.name, project.id);
    Ok(project)
}

#[derive(serde::Deserialize)]
pub struct UpdateProjectRequest {
    pub id: String,
    pub name: String,
    pub color: String,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn update_project(
    request: UpdateProjectRequest,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);

    // Get existing project to preserve timestamps
    let existing = project_repo.get(&request.id)
        .map_err(|e| format!("Failed to get project: {}", e))?
        .ok_or("Project not found")?;

    let updated = Project {
        id: request.id,
        name: request.name,
        color: request.color,
        description: request.description,
        created_at: existing.created_at,
        updated_at: existing.updated_at,
        paper_count: existing.paper_count,
    };

    project_repo.update(&updated).map_err(|e| format!("Failed to update project: {}", e))?;
    info!("Updated project: {}", updated.name);
    Ok(())
}

#[tauri::command]
pub async fn delete_project(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    let deleted = project_repo.delete(&id).map_err(|e| format!("Failed to delete project: {}", e))?;

    if deleted {
        info!("Deleted project: {}", id);
        Ok(())
    } else {
        Err("Project not found".to_string())
    }
}

#[tauri::command]
pub async fn add_paper_to_project(
    project_id: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.add_paper(&project_id, &citekey)
        .map_err(|e| format!("Failed to add paper to project: {}", e))?;

    info!("Added {} to project {}", citekey, project_id);
    Ok(())
}

#[tauri::command]
pub async fn remove_paper_from_project(
    project_id: String,
    citekey: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.remove_paper(&project_id, &citekey)
        .map_err(|e| format!("Failed to remove paper from project: {}", e))?;

    info!("Removed {} from project {}", citekey, project_id);
    Ok(())
}

#[tauri::command]
pub async fn get_project_papers(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.get_papers(&project_id)
        .map_err(|e| format!("Failed to get project papers: {}", e))
}

#[tauri::command]
pub async fn get_paper_projects(
    citekey: String,
    state: State<'_, AppState>,
) -> Result<Vec<Project>, String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.get_paper_projects(&citekey)
        .map_err(|e| format!("Failed to get paper projects: {}", e))
}

#[tauri::command]
pub async fn set_paper_projects(
    citekey: String,
    project_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_guard = state.db.lock().map_err(|e| e.to_string())?;
    let db = db_guard.as_ref().ok_or("No vault is open")?;

    let project_repo = ProjectRepo::new(&db.conn);
    project_repo.set_paper_projects(&citekey, &project_ids)
        .map_err(|e| format!("Failed to set paper projects: {}", e))?;

    info!("Set projects for {}: {:?}", citekey, project_ids);
    Ok(())
}
