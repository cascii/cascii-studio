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

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn duplicate_project(project_id: String) -> Result<database::Project, String> {
    tokio::task::spawn_blocking(move || duplicate_project_blocking(&project_id))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn duplicate_project_blocking(project_id: &str) -> Result<database::Project, String> {
    let old_project =
        database::get_project(project_id).map_err(|e| format!("Failed to get project: {}", e))?;
    let settings = settings::load();
    let output_dir = PathBuf::from(&settings.output_directory);
    let old_dir = output_dir.join(&old_project.project_path);
    if !old_dir.exists() {
        return Err(format!(
            "Project directory does not exist: {}",
            old_dir.display()
        ));
    }

    let new_name = format!("{} copy", old_project.project_name);
    let new_id_short = &Uuid::new_v4().to_string()[..8];
    let new_project_path = format!("{}_{}", new_name, new_id_short);
    let new_dir = output_dir.join(&new_project_path);

    copy_dir_recursive(&old_dir, &new_dir).map_err(|e| {
        let _ = fs::remove_dir_all(&new_dir);
        format!("Failed to copy project directory: {}", e)
    })?;

    let old_dir_str = old_dir.to_string_lossy().to_string();
    let new_dir_str = new_dir.to_string_lossy().to_string();

    database::duplicate_project_records(
        project_id,
        &new_name,
        &new_project_path,
        &old_dir_str,
        &new_dir_str,
    )
    .map_err(|e| {
        let _ = fs::remove_dir_all(&new_dir);
        format!("Failed to duplicate project records: {}", e)
    })
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

#[tauri::command]
pub async fn duplicate_resource(node_id: String, project_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || duplicate_resource_blocking(&node_id, &project_id))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn copy_name(name: &str) -> String {
    format!("{} copy", name)
}

fn copy_file_path(path: &std::path::Path) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let new_name = if ext.is_empty() {
        format!("{} copy", stem)
    } else {
        format!("{} copy.{}", stem, ext)
    };
    path.with_file_name(new_name)
}

fn copy_dir_name(path: &std::path::Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("folder");
    path.with_file_name(format!("{}_copy", name))
}

fn duplicate_resource_blocking(node_id: &str, project_id: &str) -> Result<String, String> {
    if let Some(source_id) = node_id
        .strip_prefix("res:source:")
        .or_else(|| node_id.strip_prefix("exp:source:"))
    {
        let sources = database::get_project_sources(project_id).map_err(|e| e.to_string())?;
        let source = sources
            .iter()
            .find(|s| s.id == source_id)
            .ok_or("Source not found")?;
        let src_path = PathBuf::from(&source.file_path);
        let dst_path = copy_file_path(&src_path);
        fs::copy(&src_path, &dst_path).map_err(|e| format!("Failed to copy file: {}", e))?;
        let new_id = Uuid::new_v4().to_string();
        let display = source.custom_name.as_deref().unwrap_or_else(|| {
            src_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("file")
        });
        let new_source = database::SourceContent {
            id: new_id.clone(),
            content_type: source.content_type.clone(),
            project_id: project_id.to_string(),
            date_added: Utc::now(),
            size: fs::metadata(&dst_path)
                .map(|m| m.len() as i64)
                .unwrap_or(source.size),
            file_path: dst_path.to_string_lossy().to_string(),
            custom_name: Some(copy_name(display)),
        };
        database::add_source_content(&new_source).map_err(|e| e.to_string())?;
        Ok(format!("res:source:{}", new_id))
    } else if let Some(cut_id) = node_id
        .strip_prefix("res:cut:")
        .or_else(|| node_id.strip_prefix("exp:cut:"))
    {
        let cuts = database::get_project_cuts(project_id).map_err(|e| e.to_string())?;
        let cut = cuts
            .iter()
            .find(|c| c.id == cut_id)
            .ok_or("Cut not found")?;
        let src_path = PathBuf::from(&cut.file_path);
        let dst_path = copy_file_path(&src_path);
        fs::copy(&src_path, &dst_path).map_err(|e| format!("Failed to copy file: {}", e))?;
        let new_id = Uuid::new_v4().to_string();
        let display = cut.custom_name.as_deref().unwrap_or_else(|| {
            src_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("cut")
        });
        let new_cut = database::VideoCut {
            id: new_id.clone(),
            project_id: project_id.to_string(),
            source_file_id: cut.source_file_id.clone(),
            file_path: dst_path.to_string_lossy().to_string(),
            date_added: Utc::now(),
            size: fs::metadata(&dst_path)
                .map(|m| m.len() as i64)
                .unwrap_or(cut.size),
            custom_name: Some(copy_name(display)),
            start_time: cut.start_time,
            end_time: cut.end_time,
            duration: cut.duration,
        };
        database::add_video_cut(&new_cut).map_err(|e| e.to_string())?;
        Ok(format!("res:cut:{}", new_id))
    } else if let Some(dir_path) = node_id
        .strip_prefix("res:framedir:")
        .or_else(|| node_id.strip_prefix("exp:framedir:"))
    {
        let conv = database::get_conversion_by_folder_path(dir_path)
            .map_err(|e| e.to_string())?
            .ok_or("Frame directory not found")?;
        let src_dir = PathBuf::from(&conv.folder_path);
        let dst_dir = copy_dir_name(&src_dir);
        copy_dir_recursive(&src_dir, &dst_dir)
            .map_err(|e| format!("Failed to copy directory: {}", e))?;
        let new_id = Uuid::new_v4().to_string();
        let display = conv.custom_name.as_deref().unwrap_or(&conv.folder_name);
        let dst_dir_str = dst_dir.to_string_lossy().to_string();
        let new_folder_name = dst_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("frames_copy")
            .to_string();
        let total_size = calculate_file_size(&dst_dir_str).unwrap_or(conv.total_size);
        let new_conv = database::AsciiConversion {
            id: new_id.clone(),
            folder_name: new_folder_name,
            folder_path: dst_dir_str,
            frame_count: conv.frame_count,
            source_file_id: conv.source_file_id.clone(),
            project_id: project_id.to_string(),
            settings: conv.settings.clone(),
            creation_date: Utc::now(),
            total_size,
            custom_name: Some(copy_name(display)),
        };
        database::add_ascii_conversion(&new_conv).map_err(|e| e.to_string())?;
        Ok(format!("res:framedir:{}", new_conv.folder_path))
    } else if let Some(preview_id) = node_id
        .strip_prefix("res:preview:")
        .or_else(|| node_id.strip_prefix("exp:preview:"))
    {
        let preview = database::get_preview(preview_id)
            .map_err(|e| e.to_string())?
            .ok_or("Preview not found")?;
        let src_dir = PathBuf::from(&preview.folder_path);
        let dst_dir = copy_dir_name(&src_dir);
        copy_dir_recursive(&src_dir, &dst_dir)
            .map_err(|e| format!("Failed to copy directory: {}", e))?;
        let new_id = Uuid::new_v4().to_string();
        let display = preview
            .custom_name
            .as_deref()
            .unwrap_or(&preview.folder_name);
        let dst_dir_str = dst_dir.to_string_lossy().to_string();
        let new_folder_name = dst_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("preview_copy")
            .to_string();
        let total_size = calculate_file_size(&dst_dir_str).unwrap_or(preview.total_size);
        let new_preview = database::Preview {
            id: new_id.clone(),
            folder_name: new_folder_name,
            folder_path: dst_dir_str,
            frame_count: preview.frame_count,
            source_file_id: preview.source_file_id.clone(),
            project_id: project_id.to_string(),
            settings: preview.settings.clone(),
            creation_date: Utc::now(),
            total_size,
            custom_name: Some(copy_name(display)),
        };
        database::add_preview(&new_preview).map_err(|e| e.to_string())?;
        Ok(format!("res:preview:{}", new_id))
    } else {
        Err(format!("Unknown resource type in node_id: {}", node_id))
    }
}
