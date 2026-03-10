use crate::database;

#[tauri::command]
pub fn get_active_project_timeline(
    project_id: String,
) -> Result<database::ProjectTimeline, String> {
    database::get_active_project_timeline(&project_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub(crate) struct SaveProjectTimelineRequest {
    project_id: String,
    timeline_id: Option<String>,
    clips: Vec<database::TimelineClipDraft>,
}

#[tauri::command]
pub fn save_project_timeline(
    request: SaveProjectTimelineRequest,
) -> Result<database::ProjectTimeline, String> {
    database::save_project_timeline(
        &request.project_id,
        request.timeline_id.as_deref(),
        &request.clips,
    )
    .map_err(|e| e.to_string())
}
