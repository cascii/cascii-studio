mod settings;
mod database;

use std::fs;
use std::path::PathBuf;
use chrono::Utc;
use uuid::Uuid;

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
        let file_size = calculate_file_size(file_path)?;
        total_size += file_size;
        
        let dest_path = copy_or_move_file(file_path, project_dir.to_str().unwrap(), use_move)?;
        
        let source_type = if is_video_file(file_path) {
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
fn get_project_sources(project_id: String) -> Result<Vec<database::SourceContent>, String> {
    database::get_project_sources(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_project(project_id: String) -> Result<(), String> {
    database::delete_project(&project_id).map_err(|e| e.to_string())
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
            get_project_sources,
            delete_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
