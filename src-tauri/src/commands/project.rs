use crate::database;
use crate::ffmpeg::ffmpeg_convert_to_mp4;
use crate::settings;
use crate::util::{
    calculate_file_size, copy_or_move_file, is_mp4_file, is_video_file, open_directory,
    FileProgress,
};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct CreateProjectRequest {
    project_name: String,
    file_paths: Vec<String>,
}

#[tauri::command]
pub async fn create_project(
    request: CreateProjectRequest,
    app: tauri::AppHandle,
) -> Result<database::Project, String> {
    tokio::task::spawn_blocking(move || create_project_blocking(request, app))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn create_project_blocking(
    request: CreateProjectRequest,
    app: tauri::AppHandle,
) -> Result<database::Project, String> {
    let settings = settings::load();

    let project_id = Uuid::new_v4().to_string();
    let project_folder_name = format!(
        "{}_{}",
        request.project_name.replace(" ", "_").to_lowercase(),
        &project_id[..8]
    );

    let project_dir = PathBuf::from(&settings.output_directory).join(&project_folder_name);
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).map_err(|e| e.to_string())?;

    let has_video = request.file_paths.iter().any(|p| is_video_file(p));
    let project_type = if has_video || request.file_paths.len() > 1 {
        database::ProjectType::Animation
    } else {
        database::ProjectType::Image
    };

    let now = Utc::now();
    let mut project = database::Project {
        id: project_id.clone(),
        project_name: request.project_name.clone(),
        project_type,
        project_path: project_folder_name,
        size: 0,
        frames: 0,
        creation_date: now,
        last_modified: now,
    };
    database::create_project(&project).map_err(|e| e.to_string())?;

    let use_move = matches!(settings.default_behavior, settings::DefaultBehavior::Move);

    let mut total_size = 0i64;
    let mut frame_count = 0;
    let total_files = request.file_paths.len();

    for (index, file_path) in request.file_paths.iter().enumerate() {
        let p = PathBuf::from(file_path);
        let file_name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Err(e) = app.emit(
            "file-progress",
            FileProgress {
                file_name: file_name.clone(),
                status: "processing".to_string(),
                message: format!("Processing {} of {}...", index + 1, total_files),
                percentage: None,
            },
        ) {
            eprintln!("Failed to emit progress event: {}", e);
        }

        thread::sleep(Duration::from_millis(10));

        let is_video = is_video_file(file_path);
        let needs_mp4_conversion = is_video && !is_mp4_file(file_path);

        let result = (|| -> Result<(), String> {
            let dest_path = if needs_mp4_conversion {
                let _ = app.emit(
                    "file-progress",
                    FileProgress {
                        file_name: file_name.clone(),
                        status: "processing".to_string(),
                        message: "Converting to MP4... 0%".to_string(),
                        percentage: Some(0.0),
                    },
                );
                thread::sleep(Duration::from_millis(10));

                ffmpeg_convert_to_mp4(file_path, source_dir.to_str().unwrap(), &app, &file_name)?
            } else {
                copy_or_move_file(file_path, source_dir.to_str().unwrap(), use_move)?
            };

            let file_size = calculate_file_size(&dest_path)?;
            total_size += file_size;

            let source_type = if is_video {
                database::SourceType::Video
            } else {
                database::SourceType::Image
            };
            let source = database::SourceContent {
                id: Uuid::new_v4().to_string(),
                content_type: source_type,
                project_id: project_id.clone(),
                date_added: Utc::now(),
                size: file_size,
                file_path: dest_path,
                custom_name: None,
            };
            database::add_source_content(&source).map_err(|e| e.to_string())?;
            frame_count += 1;

            Ok(())
        })();

        match result {
            Ok(_) => {
                let _ = app.emit(
                    "file-progress",
                    FileProgress {
                        file_name: file_name.clone(),
                        status: "completed".to_string(),
                        message: "Completed".to_string(),
                        percentage: Some(100.0),
                    },
                );
            }
            Err(e) => {
                let _ = app.emit(
                    "file-progress",
                    FileProgress {
                        file_name: file_name.clone(),
                        status: "error".to_string(),
                        message: format!("Error: {}", e),
                        percentage: None,
                    },
                );
                return Err(e);
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    if frame_count > 0 {
        database::update_project_size_and_frames(&project_id, total_size, frame_count)
            .map_err(|e| e.to_string())?;
        project.size = total_size;
        project.frames = frame_count;
        project.last_modified = Utc::now();
    }

    Ok(project)
}

#[tauri::command]
pub fn get_all_projects() -> Result<Vec<database::Project>, String> {
    database::get_all_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_project(project_id: String, project_name: String) -> Result<(), String> {
    let normalized_name = project_name.trim();
    if normalized_name.is_empty() {
        return Err("Project name cannot be empty".to_string());
    }

    database::update_project_name(&project_id, normalized_name)
        .map_err(|e| format!("Failed to rename project: {}", e))
}

#[tauri::command]
pub fn open_project_folder(project_id: String) -> Result<(), String> {
    let project =
        database::get_project(&project_id).map_err(|e| format!("Failed to get project: {}", e))?;
    let settings = settings::load();
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);

    if !project_dir.exists() {
        return Err(format!(
            "Project directory does not exist: {}",
            project_dir.display()
        ));
    }

    open_directory(project_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub fn get_project(project_id: String) -> Result<database::Project, String> {
    database::get_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project(project_id: String) -> Result<(), String> {
    let project = database::get_project(&project_id).map_err(|e| e.to_string())?;

    let settings = settings::load();
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);

    database::delete_project(&project_id).map_err(|e| e.to_string())?;

    if project_dir.exists() {
        fs::remove_dir_all(&project_dir)
            .map_err(|e| format!("Failed to delete project directory: {}", e))?;
    }

    Ok(())
}
