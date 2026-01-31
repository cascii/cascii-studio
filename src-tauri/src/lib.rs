mod settings;
mod database;

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use chrono::Utc;
use uuid::Uuid;
use tauri::Emitter;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct PreparedMedia {
    pub cached_abs_path: String,
    pub media_kind: MediaKind,
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

fn determine_media_kind(path: &Path) -> MediaKind {
    if is_video_file(path.to_str().unwrap_or("")) {
        MediaKind::Video
    } else {
        MediaKind::Image
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
fn set_loop_enabled(enabled: bool) -> Result<(), String> {
    let mut s = settings::load();
    s.loop_enabled = enabled;
    settings::save(&s)
}

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
async fn add_source_files(args: AddSourceFilesArgs, app: tauri::AppHandle) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        add_source_files_blocking(args.request, app)
    }).await.map_err(|e| format!("Task failed: {}", e))?
}

fn add_source_files_blocking(request: AddSourceFilesRequest, app: tauri::AppHandle) -> Result<(), String> {
    // Load settings
    let settings = settings::load();

    // Get the project to determine the project directory
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;

    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let source_dir = project_dir.join("source");

    // Ensure source directory exists
    if !source_dir.exists() {
        fs::create_dir_all(&source_dir).map_err(|e| format!("Failed to create source directory: {}", e))?;
    }

    let use_move = matches!(settings.default_behavior, settings::DefaultBehavior::Move);

    // Process each file
    for file_path in request.file_paths {
        let p = PathBuf::from(&file_path);
        let file_name = p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let is_video = is_video_file(&file_path);
        let needs_mp4_conversion = is_video && !is_mp4_file(&file_path);

        let result = (|| -> Result<(), String> {
            // Emit processing event so the UI can show progress (same event name as create_project)
            let _ = app.emit("file-progress", FileProgress {
                file_name: file_name.clone(),
                status: "processing".to_string(),
                message: "Processing...".to_string(),
                percentage: None,
            });
            thread::sleep(Duration::from_millis(10));

            let dest_path = if needs_mp4_conversion {
                // Convert any non-MP4 video to MP4
                let _ = app.emit("file-progress", FileProgress {
                    file_name: file_name.clone(),
                    status: "processing".to_string(),
                    message: "Converting to MP4... 0%".to_string(),
                    percentage: Some(0.0),
                });
                thread::sleep(Duration::from_millis(10));
                ffmpeg_convert_to_mp4(&file_path, source_dir.to_str().unwrap(), &app, &file_name)?
            } else {
                // Copy or move all other files as-is
                copy_or_move_file(&file_path, source_dir.to_str().unwrap(), use_move)?
            };

            let file_size = calculate_file_size(&dest_path)?;

            let source_type = if is_video { database::SourceType::Video } else { database::SourceType::Image };
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

            // Emit completion event
            let _ = app.emit("file-progress", FileProgress {
                file_name: file_name.clone(),
                status: "completed".to_string(),
                message: "Completed".to_string(),
                percentage: Some(100.0),
            });
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("Failed to process file {}: {}", file_path, e);
            let _ = app.emit("file-progress", FileProgress {
                file_name: file_name.clone(),
                status: "error".to_string(),
                message: format!("Error: {}", e),
                percentage: None,
            });
            // Continue with other files
        }
    }

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
        // Treat dialog cancel as "no selection" (not an error), so the UI doesn't panic on a rejected invoke.
        None => Ok(Vec::new()),
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

fn is_mp4_file(path: &str) -> bool {
    if let Some(ext) = PathBuf::from(path).extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        ext_lower == "mp4"
    } else {
        false
    }
}

fn get_video_duration(input_path: &str) -> Result<f32, String> {
    use std::process::Command;
    
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            input_path
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;
    
    if !output.status.success() {
        return Err("Failed to get video duration".to_string());
    }
    
    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str.trim()
        .parse::<f32>()
        .map_err(|e| format!("Failed to parse duration: {}", e))
}

fn ffmpeg_convert_to_mp4(input_path: &str, output_dir: &str, app: &tauri::AppHandle, file_name: &str) -> Result<String, String> {
    use std::process::{Command, Stdio};
    use std::io::{BufRead, BufReader};
    
    let input = PathBuf::from(input_path);
    let file_stem = input.file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid input filename")?;
    
    let output_path = PathBuf::from(output_dir).join(format!("{}.mp4", file_stem));
    
    // Get video duration first
    let duration = get_video_duration(input_path).unwrap_or(0.0);
    
    // Run ffmpeg with progress monitoring
    let mut child = Command::new("ffmpeg")
        .arg("-i")
        .arg(input_path)
        .arg("-c:v").arg("libx264")
        .arg("-c:a").arg("aac")
        .arg("-movflags").arg("+faststart")
        .arg("-progress").arg("pipe:2")
        .arg("-y")  // Overwrite without asking
        .arg(&output_path)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run ffmpeg: {}. Make sure ffmpeg is installed.", e))?;
    
    // Parse progress from stderr
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                // Look for "out_time_ms=" or "time=" in the progress output
                if line.starts_with("out_time_ms=") {
                    if let Some(time_us) = line.strip_prefix("out_time_ms=") {
                        if let Ok(microseconds) = time_us.parse::<f32>() {
                            let current_time = microseconds / 1_000_000.0;
                            if duration > 0.0 {
                                let percentage = (current_time / duration * 100.0).min(99.0);
                                let _ = app.emit("file-progress", FileProgress {
                                    file_name: file_name.to_string(),
                                    status: "processing".to_string(),
                                    message: format!("Converting to MP4... {:.0}%", percentage),
                                    percentage: Some(percentage),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    
    let status = child.wait()
        .map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;
    
    if !status.success() {
        return Err(format!("ffmpeg conversion failed with status: {}", status));
    }
    
    Ok(output_path.to_str()
        .ok_or("Invalid output path")?
        .to_string())
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

#[derive(serde::Deserialize)]
struct AddSourceFilesRequest {
    project_id: String,
    file_paths: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AddSourceFilesArgs {
    request: AddSourceFilesRequest,
}

#[derive(Clone, serde::Serialize)]
struct FileProgress {
    file_name: String,
    status: String, // "pending", "processing", "completed", "error"
    message: String,
    percentage: Option<f32>,
}

#[tauri::command]
async fn create_project(request: CreateProjectRequest, app: tauri::AppHandle) -> Result<database::Project, String> {
    // Spawn the actual work in a blocking task to prevent UI freeze
    tokio::task::spawn_blocking(move || {
        create_project_blocking(request, app)
    }).await.map_err(|e| format!("Task failed: {}", e))?
}

fn create_project_blocking(request: CreateProjectRequest, app: tauri::AppHandle) -> Result<database::Project, String> {
    // Load settings to get output directory and default behavior
    let settings = settings::load();

    // Generate project ID and create project directory
    let project_id = Uuid::new_v4().to_string();
    let project_folder_name = format!("{}_{}",
        request.project_name.replace(" ", "_").to_lowercase(),
        &project_id[..8]
    );

    let project_dir = PathBuf::from(&settings.output_directory).join(&project_folder_name);
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).map_err(|e| e.to_string())?;

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

    // Process and save source files with progress tracking
    let mut total_size = 0i64;
    let mut frame_count = 0;
    let total_files = request.file_paths.len();

    for (index, file_path) in request.file_paths.iter().enumerate() {
        let p = PathBuf::from(file_path);
        let file_name = p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Emit processing event
        if let Err(e) = app.emit("file-progress", FileProgress {
            file_name: file_name.clone(),
            status: "processing".to_string(),
            message: format!("Processing {} of {}...", index + 1, total_files),
            percentage: None,
        }) {
            eprintln!("Failed to emit progress event: {}", e);
        }

        // Small delay to ensure event is sent
        thread::sleep(Duration::from_millis(10));

        let is_video = is_video_file(file_path);
        let needs_mp4_conversion = is_video && !is_mp4_file(file_path);

        let result = (|| -> Result<(), String> {
            let dest_path = if needs_mp4_conversion {
                // Convert any non-MP4 video to MP4
                let _ = app.emit("file-progress", FileProgress {
                    file_name: file_name.clone(),
                    status: "processing".to_string(),
                    message: "Converting to MP4... 0%".to_string(),
                    percentage: Some(0.0),
                });
                thread::sleep(Duration::from_millis(10));

                ffmpeg_convert_to_mp4(file_path, source_dir.to_str().unwrap(), &app, &file_name)?
            } else {
                // Copy or move all other files as-is
                copy_or_move_file(file_path, source_dir.to_str().unwrap(), use_move)?
            };

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
                custom_name: None,
            };
            database::add_source_content(&source).map_err(|e| e.to_string())?;
            frame_count += 1;
            
            Ok(())
        })();
        
        // Emit completion or error
        match result {
            Ok(_) => {
                let _ = app.emit("file-progress", FileProgress {
                    file_name: file_name.clone(),
                    status: "completed".to_string(),
                    message: "Completed".to_string(),
                    percentage: Some(100.0),
                });
            }
            Err(e) => {
                let _ = app.emit("file-progress", FileProgress {
                    file_name: file_name.clone(),
                    status: "error".to_string(),
                    message: format!("Error: {}", e),
                    percentage: None,
                });
                return Err(e);
            }
        }
        
        // Small delay to allow UI to update
        thread::sleep(Duration::from_millis(50));
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
fn get_project_conversions(project_id: String) -> Result<Vec<database::AsciiConversion>, String> {
    database::get_project_conversions(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_conversion_by_folder_path(folder_path: String) -> Result<Option<database::AsciiConversion>, String> {
    database::get_conversion_by_folder_path(&folder_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_conversion_frame_speed(conversion_id: String, frame_speed: u32) -> Result<(), String> {
    println!("🔄 Tauri: Updating frame_speed for conversion {} to {}", conversion_id, frame_speed);
    let result = database::update_conversion_frame_speed(&conversion_id, frame_speed);
    match &result {
        Ok(_) => println!("✅ Tauri: Database update successful"),
        Err(e) => println!("❌ Tauri: Database update failed: {}", e),
    }
    result.map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FrameDirectory {
    pub name: String,           // Display name like "Notan Nigres - Frames"
    pub directory_path: String, // Full path to the frames directory
    pub source_file_name: String, // Original source file name
    pub custom_name: Option<String>, // Custom display name for the frame directory
}

#[tauri::command]
fn get_project_frames(project_id: String) -> Result<Vec<FrameDirectory>, String> {
    // Get project details
    let project = database::get_project(&project_id).map_err(|e| e.to_string())?;

    // Load settings to get output directory
    let settings = settings::load();

    // Construct the full path to the project directory
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);

    if !project_dir.exists() {
        return Ok(Vec::new());
    }

    // Scan for directories ending with "_ascii"
    let mut frames = Vec::new();

    // Helper function to scan a directory for ascii frame folders
    let scan_directory = |dir: &PathBuf, frames: &mut Vec<FrameDirectory>| -> Result<(), String> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                        if dir_name.ends_with("_ascii") || dir_name.contains("_ascii[") {
                            // Extract source file name (remove "_ascii[...]" suffix)
                            let source_name = if let Some(bracket_start) = dir_name.find("_ascii[") {
                                // Has random suffix: extract part before "_ascii["
                                &dir_name[..bracket_start]
                            } else {
                                // Old format without random suffix
                                dir_name.strip_suffix("_ascii").unwrap_or(dir_name)
                            };

                            // Get custom name from database if it exists
                            let folder_path = path.to_str().unwrap_or("");
                            let custom_name = database::get_conversion_by_folder_path(folder_path)
                                .ok()
                                .flatten()
                                .and_then(|conversion| conversion.custom_name);

                            // Create display name: use custom_name if available, otherwise "{Source Name} - Frames"
                            let display_name = custom_name.clone().unwrap_or_else(|| format!("{} - Frames", source_name));

                            frames.push(FrameDirectory {
                                name: display_name,
                                directory_path: folder_path.to_string(),
                                source_file_name: source_name.to_string(),
                                custom_name,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    };

    // Scan the frames directory (new structure)
    let frames_dir = project_dir.join("frames");
    if frames_dir.exists() {
        scan_directory(&frames_dir, &mut frames)?;
    }

    // Also scan the main project directory for backward compatibility
    scan_directory(&project_dir, &mut frames)?;

    // Also scan the cuts subdirectory for frames converted from cuts (legacy)
    let cuts_dir = project_dir.join("cuts");
    if cuts_dir.exists() {
        scan_directory(&cuts_dir, &mut frames)?;
    }

    // Sort by name for consistent ordering
    frames.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(frames)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FrameFile {
    pub path: String,
    pub name: String,
    pub index: u32,
}

fn scan_frames_in_dir(dir: &PathBuf) -> Result<Vec<FrameFile>, String> {
    if !dir.exists() {
        return Err("Directory does not exist".to_string());
    }
    
    let mut frames = Vec::new();
    
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if ext == "txt" {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                // Extract frame index from filename (e.g., "frame_0001.txt" -> 1)
                                let index = if file_name.starts_with("frame_") {
                                    file_name
                                        .strip_prefix("frame_")
                                        .and_then(|s| s.strip_suffix(".txt"))
                                        .and_then(|s| s.parse::<u32>().ok())
                                        .unwrap_or(0)
                                } else {
                                    // Try to extract number from filename
                                    file_name
                                        .chars()
                                        .filter(|c| c.is_ascii_digit())
                                        .collect::<String>()
                                        .parse::<u32>()
                                        .unwrap_or(0)
                                };
                                
                                frames.push(FrameFile {
                                    path: path.to_str().unwrap_or("").to_string(),
                                    name: file_name.to_string(),
                                    index,
                                });
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            return Err(format!("Failed to read directory: {}", e));
        }
    }
    
    // Sort by index
    frames.sort_by(|a, b| a.index.cmp(&b.index));
    
    Ok(frames)
}

#[tauri::command]
fn get_frame_files(directory_path: String) -> Result<Vec<FrameFile>, String> {
    let dir = PathBuf::from(&directory_path);
    scan_frames_in_dir(&dir)
}

#[tauri::command]
fn read_frame_file(file_path: String) -> Result<String, String> {
    fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read frame file: {}", e))
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

#[tauri::command]
fn rename_source_file(source_id: String, custom_name: Option<String>) -> Result<(), String> {
    database::update_source_custom_name(&source_id, custom_name)
        .map_err(|e| format!("Failed to rename source file: {}", e))
}

#[derive(serde::Deserialize)]
struct DeleteSourceFileRequest {
    source_id: String,
    file_path: String,
}

#[tauri::command]
fn delete_source_file(request: DeleteSourceFileRequest) -> Result<(), String> {
    println!("🗑️ Deleting source file: {} ({})", request.source_id, request.file_path);

    // Delete the physical file
    let file_path = PathBuf::from(&request.file_path);
    if file_path.exists() {
        fs::remove_file(&file_path)
            .map_err(|e| format!("Failed to delete file: {}", e))?;
        println!("✅ Deleted physical file: {}", request.file_path);
    } else {
        println!("⚠️ File not found, skipping physical delete: {}", request.file_path);
    }

    // Delete from database (this also deletes associated conversions and cuts)
    database::delete_source_content(&request.source_id)
        .map_err(|e| format!("Failed to delete from database: {}", e))?;

    println!("✅ Source file deleted successfully");
    Ok(())
}

#[derive(serde::Deserialize)]
struct UpdateFrameCustomNameRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    #[serde(rename = "customName")]
    custom_name: Option<String>,
}

#[tauri::command]
fn update_frame_custom_name(request: UpdateFrameCustomNameRequest) -> Result<(), String> {
    // First get the conversion by folder path to find the ID
    let conversion = database::get_conversion_by_folder_path(&request.folder_path)
        .map_err(|e| format!("Failed to find conversion: {}", e))?
        .ok_or("Conversion not found")?;

    // Update the custom name
    database::update_conversion_custom_name(&conversion.id, request.custom_name)
        .map_err(|e| format!("Failed to update custom name: {}", e))
}

#[tauri::command]
fn delete_frame_directory(directory_path: String) -> Result<(), String> {
    let dir_path = PathBuf::from(&directory_path);

    // Check if directory exists
    if !dir_path.exists() {
        return Err("Directory does not exist".to_string());
    }

    // Delete the directory and all its contents
    fs::remove_dir_all(&dir_path)
        .map_err(|e| format!("Failed to delete frame directory: {}", e))?;

    // Find and delete the corresponding conversion from database
    // The conversion folder_path should match the directory_path
    database::delete_conversion_by_folder_path(&directory_path)
        .map_err(|e| format!("Failed to delete conversion from database: {}", e))?;

    Ok(())
}

#[derive(serde::Deserialize, Clone)]
struct ConvertToAsciiRequest {
    file_path: String,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: Option<u32>,
    project_id: String,
    source_file_id: String,
    custom_name: Option<String>,
    #[serde(default)]
    color: bool,
    #[serde(default)]
    extract_audio: bool,
}

#[derive(Clone, serde::Serialize)]
struct ConversionProgress {
    source_id: String,
    percentage: u8,
}

#[derive(Clone, serde::Serialize)]
struct ConversionComplete {
    source_id: String,
    success: bool,
    message: String,
}

/// Check if a command exists on the system PATH
fn command_exists(cmd: &str) -> bool {
    // Try running the command directly
    if let Ok(status) = std::process::Command::new(cmd)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        if status.success() {
            return true;
        }
    }

    // Fallback: use 'which' on Unix or 'where' on Windows
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Check if system ffmpeg is available (exposed as Tauri command)
#[tauri::command]
fn check_system_ffmpeg() -> bool {
    let ffmpeg_exists = command_exists("ffmpeg");
    let ffprobe_exists = command_exists("ffprobe");
    let available = ffmpeg_exists && ffprobe_exists;
    println!("🔍 System ffmpeg check: ffmpeg={}, ffprobe={}, available={}",
             ffmpeg_exists, ffprobe_exists, available);
    available
}

/// Get sidecar ffmpeg paths if they exist
fn get_sidecar_paths(app: &tauri::AppHandle) -> Option<(PathBuf, PathBuf)> {
    use tauri::Manager;

    #[cfg(target_os = "windows")]
    let (ffmpeg_name, ffprobe_name) = ("ffmpeg.exe", "ffprobe.exe");
    #[cfg(not(target_os = "windows"))]
    let (ffmpeg_name, ffprobe_name) = ("ffmpeg", "ffprobe");

    // List of directories to check for sidecar binaries
    let mut dirs_to_check: Vec<PathBuf> = Vec::new();

    // Check resource directory (for production builds)
    if let Ok(resource_dir) = app.path().resource_dir() {
        dirs_to_check.push(resource_dir.join("binaries"));
        dirs_to_check.push(resource_dir);
    }

    // Check relative to executable (for development)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            dirs_to_check.push(exe_dir.join("binaries"));
            // Go up to find src-tauri/binaries in dev mode
            if let Some(parent) = exe_dir.parent() {
                if let Some(grandparent) = parent.parent() {
                    if let Some(great_grandparent) = grandparent.parent() {
                        dirs_to_check.push(great_grandparent.join("src-tauri").join("binaries"));
                    }
                }
            }
        }
    }

    // Check current working directory
    if let Ok(cwd) = std::env::current_dir() {
        dirs_to_check.push(cwd.join("src-tauri").join("binaries"));
        dirs_to_check.push(cwd.join("binaries"));
    }

    // Try each directory
    for binaries_dir in &dirs_to_check {
        let ffmpeg_path = binaries_dir.join(ffmpeg_name);
        let ffprobe_path = binaries_dir.join(ffprobe_name);

        println!("🔍 Checking for sidecar in: {}", binaries_dir.display());

        if ffmpeg_path.exists() && ffprobe_path.exists() {
            println!("✓ Found sidecar binaries in: {}", binaries_dir.display());
            return Some((ffmpeg_path, ffprobe_path));
        }
    }

    println!("⚠ Sidecar binaries not found in any checked location");
    None
}

/// Check if sidecar ffmpeg is available (exposed as Tauri command)
#[tauri::command]
fn check_sidecar_ffmpeg(app: tauri::AppHandle) -> bool {
    let available = get_sidecar_paths(&app).is_some();
    println!("🔍 Sidecar ffmpeg check: {}", if available { "available" } else { "not found" });
    available
}

/// Get FfmpegConfig based on user settings
fn get_ffmpeg_config(app: &tauri::AppHandle, ffmpeg_source: &settings::FfmpegSource) -> cascii::FfmpegConfig {
    match ffmpeg_source {
        settings::FfmpegSource::System => {
            if command_exists("ffmpeg") && command_exists("ffprobe") {
                println!("🎬 Using system ffmpeg (from settings)");
                cascii::FfmpegConfig::new()
            } else {
                // Fall back to sidecar if system not available
                println!("⚠ System ffmpeg not found, falling back to sidecar...");
                get_sidecar_config(app)
            }
        }
        settings::FfmpegSource::Sidecar => {
            println!("🎬 Using sidecar ffmpeg (from settings)");
            get_sidecar_config(app)
        }
    }
}

/// Get FfmpegConfig for sidecar binaries
fn get_sidecar_config(app: &tauri::AppHandle) -> cascii::FfmpegConfig {
    let mut config = cascii::FfmpegConfig::new();

    if let Some((ffmpeg_path, ffprobe_path)) = get_sidecar_paths(app) {
        println!("✓ Using bundled ffmpeg: {}", ffmpeg_path.display());
        println!("✓ Using bundled ffprobe: {}", ffprobe_path.display());
        config = config.with_ffmpeg(ffmpeg_path).with_ffprobe(ffprobe_path);
    } else {
        println!("⚠ Warning: Sidecar ffmpeg/ffprobe not found. Video conversion will fail.");
        println!("  Please place ffmpeg binaries in the app's binaries folder.");
    }

    config
}

#[tauri::command]
async fn convert_to_ascii(app: tauri::AppHandle, request: ConvertToAsciiRequest) -> Result<String, String> {
    use cascii::{AsciiConverter, ConversionOptions, VideoOptions};

    let input_path = PathBuf::from(&request.file_path);

    if !input_path.exists() {
        return Err(format!("File not found: {}", request.file_path));
    }

    // Determine if it's an image or video
    let is_image = input_path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp"))
        .unwrap_or(false);

    // Get project directory to save frames in /frames subdirectory
    let settings = settings::load();
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let frames_dir = project_dir.join("frames");

    // Ensure frames directory exists
    fs::create_dir_all(&frames_dir)
        .map_err(|e| format!("Failed to create frames directory: {}", e))?;

    // Create output directory in /frames
    let random_suffix = generate_random_suffix();
    let folder_name = format!("{}_ascii{}",
        input_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output"),
        random_suffix
    );
    let output_dir = frames_dir.join(&folder_name);

    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    // Prepare variables for the background task
    let input_path_clone = input_path.clone();
    let output_dir_clone = output_dir.clone();
    let request_clone = request.clone();
    let fps = request.fps.unwrap_or(30);
    let source_id_for_progress = request.source_file_id.clone();
    let source_id_for_complete = request.source_file_id.clone();
    let display_name = request.custom_name.clone().unwrap_or_else(|| {
        input_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string()
    });
    let display_name_for_return = display_name.clone();
    let folder_name_clone = folder_name.clone();

    // Clone values that need to be used after spawn_blocking moves them
    let app_for_complete = app.clone();
    let project_id_for_db = request.project_id.clone();
    let custom_name_for_db = request.custom_name.clone();
    // Copy settings values (they're Copy types but request_clone gets moved into spawn_blocking)
    let luminance_for_db = request.luminance;
    let font_ratio_for_db = request.font_ratio;
    let columns_for_db = request.columns;
    let color_for_db = request.color;
    let extract_audio_flag = request.extract_audio;
    let is_image_for_audio = is_image;  // Copy for use in audio extraction check
    
    // Clone values for audio extraction
    let input_path_for_audio = input_path.clone();
    let project_dir_for_audio = project_dir.clone();
    let random_suffix_for_audio = random_suffix.clone();
    let source_id_for_audio = request.source_file_id.clone();
    let project_id_for_audio = request.project_id.clone();

    // Get ffmpeg config based on user settings
    let current_settings = settings::load();
    let ffmpeg_config = get_ffmpeg_config(&app, &current_settings.ffmpeg_source);

    // Spawn the conversion in a background task (don't await - returns immediately)
    tokio::spawn(async move {
        let conversion_result = tokio::task::spawn_blocking(move || -> Result<PathBuf, String> {
            let converter = AsciiConverter::new().with_ffmpeg_config(ffmpeg_config);

            let output_mode = if request_clone.color {
                cascii::OutputMode::TextAndColor
            } else {
                cascii::OutputMode::TextOnly
            };
            let conv_opts = ConversionOptions::default()
                .with_columns(request_clone.columns)
                .with_font_ratio(request_clone.font_ratio)
                .with_luminance(request_clone.luminance)
                .with_output_mode(output_mode);

            if is_image {
                // Convert single image
                let output_file = output_dir_clone.join(format!("{}.txt",
                    input_path_clone.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output")
                ));

                converter.convert_image(&input_path_clone, &output_file, &conv_opts)
                    .map_err(|e| format!("Failed to convert image: {}", e))?;

                Ok(output_dir_clone)
            } else {
                // Convert video with progress callback
                let video_opts = VideoOptions {
                    fps,
                    start: None,
                    end: None,
                    columns: request_clone.columns,
                    extract_audio: false, // Audio extraction handled separately
                };

                println!("🎬 Starting video conversion: {}", source_id_for_progress);
                let app_clone = app.clone();

                // Pre-clone source_id ONCE outside the hot loop (not per-frame)
                let source_id_owned = source_id_for_progress.clone();

                // Atomic tracker for throttling - only emit when percentage changes
                use std::sync::atomic::{AtomicU8, Ordering};
                let last_reported_percent = std::sync::Arc::new(AtomicU8::new(0));
                let last_percent_clone = std::sync::Arc::clone(&last_reported_percent);

                converter.convert_video_with_progress(
                    &input_path_clone,
                    &output_dir_clone,
                    &video_opts,
                    &conv_opts,
                    false,
                    Some(move |completed: usize, total: usize| {
                        let percentage = if total > 0 {
                            ((completed as f64 / total as f64) * 100.0) as u8
                        } else {
                            0
                        };

                        let last = last_percent_clone.load(Ordering::Relaxed);

                        // Only emit when percentage actually changes OR on completion
                        if percentage > last || completed == total {
                            last_percent_clone.store(percentage, Ordering::Relaxed);
                            let _ = app_clone.emit("conversion-progress", ConversionProgress {source_id: source_id_owned.clone(),percentage});
                        }
                    }),
                ).map_err(|e| format!("Failed to convert video: {}", e))?;

                Ok(output_dir_clone)
            }
        })
        .await;

        // Process the result and emit completion event
        match conversion_result {
            Ok(Ok(result_path)) => {
                // Count frames and calculate total size
                match count_frames_and_size(&result_path) {
                    Ok((frame_count, total_size)) => {
                        // Create database entry for the conversion
                        let conversion = database::AsciiConversion {
                            id: Uuid::new_v4().to_string(),
                            folder_name: folder_name_clone,
                            folder_path: result_path.to_str().unwrap_or("").to_string(),
                            frame_count,
                            source_file_id: source_id_for_complete.clone(),
                            project_id: project_id_for_db.clone(),
                            settings: database::ConversionSettings {
                                luminance: luminance_for_db,
                                font_ratio: font_ratio_for_db,
                                columns: columns_for_db,
                                fps,
                                frame_speed: fps,
                                color: color_for_db,
                            },
                            creation_date: Utc::now(),
                            total_size,
                            custom_name: custom_name_for_db.clone(),
                        };

                        match database::add_ascii_conversion(&conversion) {
                            Ok(_) => {
                                println!("✅ Conversion complete: {}", source_id_for_complete);
                                
                                // Extract audio if flag is set and it's a video
                                let mut audio_message = String::new();
                                println!("🔊 Audio extraction check: flag={}, is_image={}", extract_audio_flag, is_image_for_audio);
                                if extract_audio_flag && !is_image_for_audio {
                                    println!("🎵 Starting audio extraction...");
                                    let audio_dir = project_dir_for_audio.join("audio");
                                    match extract_audio_from_video(&input_path_for_audio, &audio_dir, &random_suffix_for_audio) {
                                        Ok((audio_folder_path, audio_size, duration)) => {
                                            // Save audio extraction to database
                                            let audio_folder_name = audio_folder_path.file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("audio")
                                                .to_string();
                                            
                                            let audio_extraction = database::AudioExtraction {
                                                id: Uuid::new_v4().to_string(),
                                                folder_name: audio_folder_name,
                                                folder_path: audio_folder_path.to_str().unwrap_or("").to_string(),
                                                source_file_id: source_id_for_audio.clone(),
                                                project_id: project_id_for_audio.clone(),
                                                creation_date: Utc::now(),
                                                total_size: audio_size,
                                                audio_track_beginning: 0.0,
                                                audio_track_end: duration,
                                                custom_name: None,
                                            };
                                            
                                            match database::add_audio_extraction(&audio_extraction) {
                                                Ok(_) => {
                                                    audio_message = format!(" + Audio extracted ({} bytes)", audio_size);
                                                    println!("✅ Audio extraction saved to database");
                                                }
                                                Err(e) => {
                                                    println!("❌ Failed to save audio to database: {}", e);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            println!("❌ Failed to extract audio: {}", e);
                                        }
                                    }
                                }
                                
                                let _ = app_for_complete.emit("conversion-complete", ConversionComplete {source_id: source_id_for_complete, success: true, message: format!("ASCII frames saved to: {} ({} frames, {} bytes){}", result_path.display(), frame_count, total_size, audio_message)});
                            }
                            Err(e) => {
                                println!("❌ Failed to save to database: {}", e);
                                let _ = app_for_complete.emit("conversion-complete", ConversionComplete {source_id: source_id_for_complete, success: false, message: format!("Failed to save conversion to database: {}", e)});
                            }
                        }
                    }
                    Err(e) => {
                        let _ = app_for_complete.emit("conversion-complete", ConversionComplete {source_id: source_id_for_complete, success: false, message: e});
                    }
                }
            }
            Ok(Err(e)) => {
                let _ = app_for_complete.emit("conversion-complete", ConversionComplete {source_id: source_id_for_complete, success: false, message: e});
            }
            Err(e) => {
                let _ = app_for_complete.emit("conversion-complete", ConversionComplete {source_id: source_id_for_complete, success: false, message: format!("Task failed: {}", e)});
            }
        }
    });

    // Return immediately - conversion runs in background
    Ok(format!("Conversion started for: {}", display_name_for_return))
}

fn generate_random_suffix() -> String {
    use rand::{Rng, thread_rng};
    use rand::distributions::Alphanumeric;

    let random_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();

    format!("[{}]", random_string)
}

fn count_frames_and_size(dir: &PathBuf) -> Result<(i32, i64), String> {
    let mut frame_count = 0i32;
    let mut total_size = 0i64;

    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext == "txt" {
                    frame_count += 1;
                    if let Ok(metadata) = fs::metadata(&path) {
                        total_size += metadata.len() as i64;
                    }
                }
            }
        }
    }

    Ok((frame_count, total_size))
}

fn extract_audio_from_video(input_path: &PathBuf, audio_dir: &PathBuf, random_suffix: &str) -> Result<(PathBuf, i64, f64), String> {
    use std::process::{Command, Stdio};

    // Get video duration for audio_track_end
    let duration = get_video_duration(input_path.to_str().unwrap_or("")).unwrap_or(0.0) as f64;

    // Create audio directory
    fs::create_dir_all(audio_dir)
        .map_err(|e| format!("Failed to create audio directory: {}", e))?;

    // Generate output filename
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");
    let folder_name = format!("{}_audio{}", file_stem, random_suffix);
    let output_folder = audio_dir.join(&folder_name);

    fs::create_dir_all(&output_folder)
        .map_err(|e| format!("Failed to create audio output folder: {}", e))?;

    let output_file = output_folder.join(format!("{}.mp3", file_stem));

    println!("🎵 Extracting audio from: {} to: {}", input_path.display(), output_file.display());

    // Run ffmpeg to extract audio
    let status = Command::new("ffmpeg")
        .arg("-i").arg(input_path)
        .arg("-vn")  // No video
        .arg("-acodec").arg("libmp3lame")
        .arg("-q:a").arg("2")  // High quality
        .arg("-y")  // Overwrite
        .arg(&output_file)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for audio extraction: {}. Make sure ffmpeg is installed.", e))?;

    if !status.success() {
        return Err(format!("ffmpeg audio extraction failed with status: {}", status));
    }

    // Calculate file size
    let file_size = fs::metadata(&output_file)
        .map_err(|e| format!("Failed to get audio file size: {}", e))?
        .len() as i64;

    println!("✅ Audio extracted: {} ({} bytes, duration: {}s)", output_file.display(), file_size, duration);

    Ok((output_folder, file_size, duration))
}

// ============== Video Cuts Commands ==============

#[derive(serde::Deserialize)]
struct CutVideoRequest {
    source_file_path: String,
    project_id: String,
    source_file_id: String,
    start_time: f64,  // in seconds
    end_time: f64,    // in seconds
}

#[derive(serde::Deserialize)]
struct CutVideoArgs {
    request: CutVideoRequest,
}

#[tauri::command]
async fn cut_video(args: CutVideoArgs, app: tauri::AppHandle) -> Result<database::VideoCut, String> {
    tokio::task::spawn_blocking(move || {
        cut_video_blocking(args.request, app)
    }).await.map_err(|e| format!("Task failed: {}", e))?
}

fn cut_video_blocking(request: CutVideoRequest, app: tauri::AppHandle) -> Result<database::VideoCut, String> {
    use std::process::{Command, Stdio};

    let settings = settings::load();
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;

    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let cuts_dir = project_dir.join("cuts");

    // Ensure cuts directory exists
    fs::create_dir_all(&cuts_dir).map_err(|e| format!("Failed to create cuts directory: {}", e))?;

    // Generate unique filename
    let input_path = PathBuf::from(&request.source_file_path);
    let file_stem = input_path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid input filename")?;

    let cut_id = Uuid::new_v4().to_string();
    let output_filename = format!("{}_cut_{}.mp4", file_stem, &cut_id[..8]);
    let output_path = cuts_dir.join(&output_filename);

    let duration = request.end_time - request.start_time;

    // Emit progress event
    let _ = app.emit("cut-progress", FileProgress {
        file_name: output_filename.clone(),
        status: "processing".to_string(),
        message: "Cutting video...".to_string(),
        percentage: Some(0.0),
    });

    println!("🎬 Cutting video: {} -> {}", request.source_file_path, output_path.display());
    println!("   Start: {}s, End: {}s, Duration: {}s", request.start_time, request.end_time, duration);

    // Run ffmpeg to cut the video
    // Using -ss before -i for fast seeking, then -t for duration
    let status = Command::new("ffmpeg")
        .arg("-ss").arg(format!("{}", request.start_time))
        .arg("-i").arg(&request.source_file_path)
        .arg("-t").arg(format!("{}", duration))
        .arg("-c:v").arg("libx264")
        .arg("-c:a").arg("aac")
        .arg("-movflags").arg("+faststart")
        .arg("-avoid_negative_ts").arg("make_zero")
        .arg("-y")
        .arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Failed to run ffmpeg: {}. Make sure ffmpeg is installed.", e))?;

    if !status.success() {
        return Err(format!("ffmpeg cut failed with status: {}", status));
    }

    // Calculate file size
    let file_size = fs::metadata(&output_path)
        .map_err(|e| format!("Failed to get file size: {}", e))?
        .len() as i64;

    // Create database entry
    let cut = database::VideoCut {
        id: cut_id,
        project_id: request.project_id,
        source_file_id: request.source_file_id,
        file_path: output_path.to_str().unwrap_or("").to_string(),
        date_added: Utc::now(),
        size: file_size,
        custom_name: None,
        start_time: request.start_time,
        end_time: request.end_time,
        duration,
    };

    database::add_video_cut(&cut).map_err(|e| format!("Failed to save cut to database: {}", e))?;

    // Emit completion event
    let _ = app.emit("cut-progress", FileProgress {
        file_name: output_filename,
        status: "completed".to_string(),
        message: "Cut completed".to_string(),
        percentage: Some(100.0),
    });

    println!("✅ Cut saved: {} ({} bytes)", cut.file_path, file_size);

    Ok(cut)
}

#[tauri::command]
fn get_project_cuts(project_id: String) -> Result<Vec<database::VideoCut>, String> {
    database::get_project_cuts(&project_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct DeleteCutRequest {
    cut_id: String,
    file_path: String,
}

#[tauri::command]
fn delete_cut(request: DeleteCutRequest) -> Result<(), String> {
    // Delete the file
    let path = PathBuf::from(&request.file_path);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete cut file: {}", e))?;
    }
    // Delete from database
    database::delete_cut(&request.cut_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
struct RenameCutRequest {
    cut_id: String,
    custom_name: Option<String>,
}

#[tauri::command]
fn rename_cut(request: RenameCutRequest) -> Result<(), String> {
    database::update_cut_custom_name(&request.cut_id, request.custom_name)
        .map_err(|e| format!("Failed to rename cut: {}", e))
}

#[derive(serde::Deserialize)]
struct CutFramesRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    #[serde(rename = "startIndex")]
    start_index: usize,
    #[serde(rename = "endIndex")]
    end_index: usize,
}

#[tauri::command]
async fn cut_frames(request: CutFramesRequest, app: tauri::AppHandle) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        cut_frames_blocking(request, app)
    }).await.map_err(|e| format!("Task failed: {}", e))?
}

fn cut_frames_blocking(request: CutFramesRequest, _app: tauri::AppHandle) -> Result<String, String> {
    // 1. Get original conversion
    let conversion = database::get_conversion_by_folder_path(&request.folder_path)
        .map_err(|e| e.to_string())?
        .ok_or("Original conversion not found")?;

    // 2. Prepare new paths
    let original_dir = PathBuf::from(&conversion.folder_path);
    if !original_dir.exists() {
        return Err("Original directory not found".to_string());
    }

    let parent_dir = original_dir.parent().ok_or("Invalid directory structure")?;
    let random_suffix = generate_random_suffix();
    
    // Extract base name logic:
    // If folder name is "video_ascii[abc]", we want "video_cut[xyz]"
    // If folder name is "video_cut[abc]", we want "video_cut[xyz]" (new unique suffix)
    let base_name = if let Some(idx) = conversion.folder_name.find("_ascii") {
        &conversion.folder_name[..idx]
    } else if let Some(idx) = conversion.folder_name.find("_cut") {
         &conversion.folder_name[..idx]
    } else {
        &conversion.folder_name
    };
    
    let new_folder_name = format!("{}_ascii{}", base_name, random_suffix);
    let new_folder_path = parent_dir.join(&new_folder_name);
    
    fs::create_dir_all(&new_folder_path).map_err(|e| e.to_string())?;
    
    // 3. Copy frames
    // Get all frame files from original directory, sorted by index
    let frame_files = scan_frames_in_dir(&original_dir)?;
    
    // Filter by index range (inclusive)
    // Note: start_index and end_index are indices into the sorted array (0-based)
    // NOT the frame number in the filename.
    let frames_to_copy: Vec<_> = frame_files.iter()
        .skip(request.start_index)
        .take(request.end_index - request.start_index + 1)
        .collect();
        
    if frames_to_copy.is_empty() {
        return Err("No frames selected".to_string());
    }

    let mut copied_count = 0;
    let mut total_size = 0i64;
    
    for (new_idx, frame) in frames_to_copy.iter().enumerate() {
        let src_path = PathBuf::from(&frame.path);
        let ext = src_path.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        
        // Format: frame_0001.txt (1-based index)
        let new_filename = format!("frame_{:04}.{}", new_idx + 1, ext);
        let dest_path = new_folder_path.join(new_filename);
        
        fs::copy(&src_path, &dest_path).map_err(|e| format!("Failed to copy {}: {}", frame.name, e))?;
        
        let size = fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0);
        total_size += size as i64;
        copied_count += 1;
    }
    
    // 4. Create new DB entry
    let custom_name = if let Some(name) = &conversion.custom_name {
        Some(format!("{} (Cut)", name))
    } else {
        Some("Cut frames".to_string())
    };

    let new_conversion = database::AsciiConversion {
        id: Uuid::new_v4().to_string(),
        folder_name: new_folder_name,
        folder_path: new_folder_path.to_str().unwrap_or("").to_string(),
        frame_count: copied_count as i32,
        source_file_id: conversion.source_file_id,
        project_id: conversion.project_id,
        settings: conversion.settings, // Copy settings
        creation_date: Utc::now(),
        total_size,
        custom_name,
    };
    
    database::add_ascii_conversion(&new_conversion).map_err(|e| e.to_string())?;
    
    Ok(format!("Cut frames saved to: {} ({} frames)", new_folder_path.display(), copied_count))
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
            set_loop_enabled,
            pick_directory,
            open_directory,
            pick_files,
            add_source_files,
            create_project,
            get_all_projects,
            get_project,
            get_project_sources,
            get_project_conversions,
            get_conversion_by_folder_path,
            get_project_frames,
            get_frame_files,
            read_frame_file,
            delete_project,
            delete_frame_directory,
            update_frame_custom_name,
            prepare_media,
            convert_to_ascii,
            update_conversion_frame_speed,
            rename_source_file,
            delete_source_file,
            cut_video,
            get_project_cuts,
            delete_cut,
            rename_cut,
            cut_frames,
            check_system_ffmpeg,
            check_sidecar_ffmpeg
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
