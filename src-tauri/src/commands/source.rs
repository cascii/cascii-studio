use crate::database;
use crate::ffmpeg::ffmpeg_convert_to_mp4;
use crate::settings;
use crate::util::{
    calculate_file_size, copy_or_move_file, is_mp4_file, is_video_file, FileProgress,
};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tauri::Emitter;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub(crate) struct AddSourceFilesRequest {
    project_id: String,
    file_paths: Vec<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct AddSourceFilesArgs {
    request: AddSourceFilesRequest,
}

#[tauri::command]
pub async fn add_source_files(
    args: AddSourceFilesArgs,
    app: tauri::AppHandle,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || add_source_files_blocking(args.request, app))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn add_source_files_blocking(
    request: AddSourceFilesRequest,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let settings = settings::load();
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let source_dir = project_dir.join("source");

    if !source_dir.exists() {
        fs::create_dir_all(&source_dir)
            .map_err(|e| format!("Failed to create source directory: {}", e))?;
    }

    let use_move = matches!(settings.default_behavior, settings::DefaultBehavior::Move);

    for file_path in request.file_paths {
        let p = PathBuf::from(&file_path);
        let file_name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let is_video = is_video_file(&file_path);
        let needs_mp4_conversion = is_video && !is_mp4_file(&file_path);

        let result = (|| -> Result<(), String> {
            let _ = app.emit(
                "file-progress",
                FileProgress {
                    file_name: file_name.clone(),
                    status: "processing".to_string(),
                    message: "Processing...".to_string(),
                    percentage: None,
                },
            );
            thread::sleep(Duration::from_millis(10));

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
                ffmpeg_convert_to_mp4(&file_path, source_dir.to_str().unwrap(), &app, &file_name)?
            } else {
                copy_or_move_file(&file_path, source_dir.to_str().unwrap(), use_move)?
            };

            let file_size = calculate_file_size(&dest_path)?;

            let source_type = if is_video {
                database::SourceType::Video
            } else {
                database::SourceType::Image
            };
            let source = database::SourceContent {
                id: Uuid::new_v4().to_string(),
                content_type: source_type,
                project_id: request.project_id.clone(),
                date_added: Utc::now(),
                file_path: dest_path,
                size: file_size,
                custom_name: None,
            };

            database::add_source_content(&source).map_err(|e| e.to_string())?;
            let _ = app.emit(
                "file-progress",
                FileProgress {
                    file_name: file_name.clone(),
                    status: "completed".to_string(),
                    message: "Completed".to_string(),
                    percentage: Some(100.0),
                },
            );
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Failed to process file {}: {}", file_path, e);
            let _ = app.emit(
                "file-progress",
                FileProgress {
                    file_name: file_name.clone(),
                    status: "error".to_string(),
                    message: format!("Error: {}", e),
                    percentage: None,
                },
            );
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn pick_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .add_filter(
            "Images and Videos",
            &[
                "jpg", "jpeg", "webp", "png", "gif", "mp4", "mkv", "mov", "avi", "webm",
            ],
        )
        .blocking_pick_files();

    match picked {
        Some(files) => {
            let paths: Vec<String> = files
                .into_iter()
                .filter_map(|file_path| match file_path {
                    tauri_plugin_dialog::FilePath::Path(path) => Some(path.display().to_string()),
                    tauri_plugin_dialog::FilePath::Url(_) => None,
                })
                .collect();
            Ok(paths)
        }
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn get_project_sources(project_id: String) -> Result<Vec<database::SourceContent>, String> {
    database::get_project_sources(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_source_file(source_id: String, custom_name: Option<String>) -> Result<(), String> {
    database::update_source_custom_name(&source_id, custom_name)
        .map_err(|e| format!("Failed to rename source file: {}", e))
}

#[derive(serde::Deserialize)]
pub(crate) struct DeleteSourceFileRequest {
    source_id: String,
    file_path: String,
}

#[tauri::command]
pub fn delete_source_file(request: DeleteSourceFileRequest) -> Result<(), String> {
    println!(
        "🗑️ Deleting source file: {} ({})",
        request.source_id, request.file_path
    );
    let file_path = PathBuf::from(&request.file_path);
    if file_path.exists() {
        fs::remove_file(&file_path).map_err(|e| format!("Failed to delete file: {}", e))?;
        println!("✅ Deleted physical file: {}", request.file_path);
    } else {
        println!(
            "⚠️ File not found, skipping physical delete: {}",
            request.file_path
        );
    }

    database::delete_source_content(&request.source_id)
        .map_err(|e| format!("Failed to delete from database: {}", e))?;
    println!("✅ Source file deleted successfully");
    Ok(())
}
