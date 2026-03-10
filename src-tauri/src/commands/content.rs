use crate::database;

#[tauri::command]
pub fn get_project_content(
    project_id: String,
) -> Result<Vec<database::ProjectContentEntry>, String> {
    database::get_project_content(&project_id)
        .map_err(|e| format!("Failed to get project content: {}", e))
}

#[derive(serde::Deserialize)]
pub(crate) struct SaveProjectContentRequest {
    project_id: String,
    entries: Vec<database::ProjectContentDraft>,
}

#[tauri::command]
pub fn save_project_content(request: SaveProjectContentRequest) -> Result<(), String> {
    database::save_project_content(&request.project_id, &request.entries)
        .map_err(|e| format!("Failed to save project content: {}", e))
}
