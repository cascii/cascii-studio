mod settings;
mod database;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use chrono::Utc;
use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PreparedMedia {
    pub cached_abs_path: String,
    pub media_kind: String,  // "image" or "video"
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

fn get_media_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = dirs::data_dir()
        .or_else(|| dirs::config_dir())
        .ok_or_else(|| "Cannot determine app data directory".to_string())?
        .join("cascii_studio")
        .join("media");
    
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Failed to create media cache dir: {}", e))?;
    Ok(cache_dir)
}

fn guess_mime_type(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg"  => Some("image/jpeg".to_string()),
        "png"           => Some("image/png".to_string()),
        "gif"           => Some("image/gif".to_string()),
        "webp"          => Some("image/webp".to_string()),
        "mp4"           => Some("video/mp4".to_string()),
        "webm"          => Some("video/webm".to_string()),
        "mov"           => Some("video/quicktime".to_string()),
        "avi"           => Some("video/x-msvideo".to_string()),
        "mkv"           => Some("video/x-matroska".to_string()),
        _ => None,
    }
}

fn determine_media_kind(path: &Path) -> String {
    if is_video_file(path.to_str().unwrap_or("")) {
        "video".to_string()
    } else {
        "image".to_string()
    }
}

#[tauri::command]
fn prepare_media(path: String) -> Result<PreparedMedia, String> {
    // 1. Canonicalize the input path
    let source_path = PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid source path: {}", e))?;
    
    // 2. Get media cache directory
    let cache_dir = get_media_cache_dir()?;
    
    // 3. Create a unique filename based on source path hash or use original name
    let file_name = source_path.file_name()
        .ok_or_else(|| "Invalid file name".to_string())?;
    let cached_path = cache_dir.join(file_name);
    
    // 4. Try hard link first, fall back to copy
    if !cached_path.exists() {
        // Try hard link
        match fs::hard_link(&source_path, &cached_path) {
            Ok(_) => {},
            Err(_) => {
                // Fall back to copy
                fs::copy(&source_path, &cached_path).map_err(|e| format!("Failed to copy file to cache: {}", e))?;
            }
        }
    }
    
    // 5. Build PreparedMedia response
    let media_kind = determine_media_kind(&source_path);
    let mime_type = guess_mime_type(&source_path);
    let cached_abs_path = cached_path
        .to_str()
        .ok_or_else(|| "Invalid cached path".to_string())?
        .to_string();
    
    // For images, we could extract dimensions using an image library, but keeping it simple for now
    Ok(PreparedMedia {cached_abs_path, media_kind, mime_type, width: None, height: None})
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn load_settings() -> settings::Settings { settings::load() }

#[tauri::command]
fn save_settings(settings: settings::Settings) -> Result<(), String> { settings::save(&settings) }

#[tauri::command]
async fn pick_directory(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let picked = app.dialog().file().blocking_pick_folder();
    match picked {
        Some(FilePath::Path(path)) => Ok(path.display().to_string()),
        Some(FilePath::Url(url)) => Err(format!("Unsupported URL folder: {url}")),
        None => Err("No folder selected".into()),
    }
}

#[tauri::command]
fn open_directory(path: String) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    { Command::new("explorer").arg(path).spawn().map_err(|e| e.to_string())?; }
    #[cfg(target_os = "macos")]
    { Command::new("open").arg(path).spawn().map_err(|e| e.to_string())?; }
    #[cfg(target_os = "linux")]
    { Command::new("xdg-open").arg(path).spawn().map_err(|e| e.to_string())?; }

    Ok(())
}

#[tauri::command]
async fn pick_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let picked = app.dialog().file().add_filter("Images and Videos", &["jpg", "jpeg", "webp", "png","gif", "mp4", "mkv", "mov", "avi", "webm"]).blocking_pick_files();
    
    match picked {
        Some(files) => {
            let paths: Vec<String> = files.into_iter().filter_map(|file_path| {
                match file_path {
                    FilePath::Path(path) => Some(path.display().to_string()),
                    FilePath::Url(_) => None,
                }
            }).collect();
            Ok(paths)
        }
        None => Err("No files selected".into()),
    }
}

fn calculate_file_size(path: &str) -> Result<i64, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(metadata.len() as i64)
}

fn is_video_file(path: &str) -> bool {
    if let Some(ext) = PathBuf::from(path).extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        matches!(ext_lower.as_str(), "mp4" | "mov" | "avi" | "webm" | "mkv" | "flv")
    } else {
        false
    }
}

fn file_stem_str(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string()
}

/// Convert a video into H.264/AAC MP4 suitable for the webview.
/// Requires `ffmpeg` on PATH.
fn ffmpeg_convert_to_mp4(input: &Path, output: &Path) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args([
            "-y",                 // overwrite
            "-i", input.to_str().ok_or("Bad input path")?,
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-crf", "23",
            "-c:a", "aac",
            "-b:a", "128k",
            "-movflags", "+faststart",
            output.to_str().ok_or("Bad output path")?,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited with status: {status}"))
    }
}

fn copy_or_move_file(source: &str, dest_dir: &str, use_move: bool) -> Result<String, String> {
    let source_path = PathBuf::from(source);
    let file_name = source_path.file_name()
        .ok_or_else(|| "Invalid source file".to_string())?;
    
    let dest_path = PathBuf::from(dest_dir).join(file_name);
    
    if use_move {
        fs::rename(source, &dest_path).map_err(|e| e.to_string())?;
    } else {
        fs::copy(source, &dest_path).map_err(|e| e.to_string())?;
    }
    
    Ok(dest_path.display().to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CreateProjectRequest {
    project_name: String,
    file_paths: Vec<String>,
}

#[tauri::command]
fn create_project(request: CreateProjectRequest) -> Result<database::Project, String> {
    // Load settings to get output directory and default behavior
    let settings = settings::load();
    
    // Generate project ID and create project directory
    let project_id = Uuid::new_v4().to_string();
    let project_folder_name = format!("{}_{}", 
        request.project_name.replace(" ", "_").to_lowercase(),
        &project_id[..8]
    );
    
    let project_dir = PathBuf::from(&settings.output_directory).join(&project_folder_name);
    fs::create_dir_all(&project_dir).map_err(|e| e.to_string())?;
    
    // Determine project type based on files
    let has_video = request.file_paths.iter().any(|p| is_video_file(p));
    let project_type = if has_video || request.file_paths.len() > 1 {
        database::ProjectType::Animation
    } else {
        database::ProjectType::Image
    };

    // Create and save the project FIRST (with initial size/frame values)
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
    
    // Process and save source files
    let mut total_size = 0i64;
    let mut frame_count = 0;
    
    for file_path in request.file_paths.iter() {
        let p = PathBuf::from(file_path);
        let is_video = is_video_file(file_path);
        let ext_lower = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();

        if is_video && ext_lower != "mp4" {
            // Convert non-mp4 videos to H.264/AAC mp4 in the project folder.
            let stem = file_stem_str(&p);
            let dest_mp4 = project_dir.join(format!("{stem}.mp4"));

            ffmpeg_convert_to_mp4(&p, &dest_mp4)?;
            let converted_size = calculate_file_size(dest_mp4.to_str().unwrap())?;
            total_size += converted_size;

            let source = database::SourceContent {
                id: Uuid::new_v4().to_string(),
                content_type: database::SourceType::Video,
                project_id: project_id.clone(),
                date_added: Utc::now(),
                size: converted_size,
                file_path: dest_mp4.display().to_string(),
            };
            database::add_source_content(&source).map_err(|e| e.to_string())?;
            frame_count += 1;

            // If the user prefers Move, we can optionally move the original file into the project folder.
            // (Not referenced by the project; just archived alongside.)
            if use_move {
                let _ = copy_or_move_file(file_path, project_dir.to_str().unwrap(), true);
            }

            continue;
        }

        // Images and mp4s: copy or move as-is into the project folder.
        let dest_path = copy_or_move_file(file_path, project_dir.to_str().unwrap(), use_move)?;

        let file_size = calculate_file_size(&dest_path)?;
        total_size += file_size;

        let source_type = if is_video { database::SourceType::Video } else { database::SourceType::Image };
        let source = database::SourceContent {
            id: Uuid::new_v4().to_string(),
            content_type: source_type,
            project_id: project_id.clone(),
            date_added: Utc::now(),
            size: file_size,
            file_path: dest_path,
        };
        database::add_source_content(&source).map_err(|e| e.to_string())?;
        frame_count += 1;
    }
    
    // Update the project with the final size and frame count
    if frame_count > 0 {
        database::update_project_size_and_frames(&project_id, total_size, frame_count).map_err(|e| e.to_string())?;
        project.size = total_size;
        project.frames = frame_count;
        project.last_modified = Utc::now();
    }
    
    Ok(project)
}

#[tauri::command]
fn get_all_projects() -> Result<Vec<database::Project>, String> {
    database::get_all_projects().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_project(project_id: String) -> Result<database::Project, String> {
    database::get_project(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_project_sources(project_id: String) -> Result<Vec<database::SourceContent>, String> {
    database::get_project_sources(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_project(project_id: String) -> Result<(), String> {
    // First, get the project details to find the project path
    let project = database::get_project(&project_id).map_err(|e| e.to_string())?;
    
    // Load settings to get the output directory
    let settings = settings::load();
    
    // Construct the full path to the project directory
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    
    // Delete the project from the database first
    database::delete_project(&project_id).map_err(|e| e.to_string())?;
    
    // Then delete the physical directory if it exists
    if project_dir.exists() {
        fs::remove_dir_all(&project_dir).map_err(|e| format!("Failed to delete project directory: {}", e))?;
    }
    
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            load_settings,
            save_settings,
            pick_directory,
            open_directory,
            pick_files,
            create_project,
            get_all_projects,
            get_project,
            get_project_sources,
            delete_project,
            prepare_media
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
