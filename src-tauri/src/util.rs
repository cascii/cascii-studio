use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::{DialogExt, FilePath};

    let picked = app.dialog().file().blocking_pick_folder();
    match picked {
        Some(FilePath::Path(path)) => Ok(path.display().to_string()),
        Some(FilePath::Url(url)) => Err(format!("Unsupported URL folder: {url}")),
        None => Err("No folder selected".into()),
    }
}

#[tauri::command]
pub fn open_directory(path: String) -> Result<(), String> {
    use std::process::Command;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn calculate_file_size(path: &str) -> Result<i64, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(metadata.len() as i64)
}

pub(crate) fn is_video_file(path: &str) -> bool {
    if let Some(ext) = PathBuf::from(path).extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        matches!(
            ext_lower.as_str(),
            "mp4" | "mov" | "avi" | "webm" | "mkv" | "flv"
        )
    } else {
        false
    }
}

pub(crate) fn is_mp4_file(path: &str) -> bool {
    if let Some(ext) = PathBuf::from(path).extension() {
        let ext_lower = ext.to_string_lossy().to_lowercase();
        ext_lower == "mp4"
    } else {
        false
    }
}

pub(crate) fn copy_or_move_file(
    source: &str,
    dest_dir: &str,
    use_move: bool,
) -> Result<String, String> {
    let source_path = PathBuf::from(source);
    let file_name = source_path
        .file_name()
        .ok_or_else(|| "Invalid source file".to_string())?;

    let dest_path = PathBuf::from(dest_dir).join(file_name);

    if use_move {
        fs::rename(source, &dest_path).map_err(|e| e.to_string())?;
    } else {
        fs::copy(source, &dest_path).map_err(|e| e.to_string())?;
    }

    Ok(dest_path.display().to_string())
}

pub(crate) fn output_mode_from_color_flag(color: bool) -> String {
    if color {
        "text+color".to_string()
    } else {
        "text-only".to_string()
    }
}

pub(crate) fn default_foreground_color() -> String {
    "white".to_string()
}

pub(crate) fn default_background_color() -> String {
    "black".to_string()
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedFrameMetadata {
    pub output_mode: String,
    pub foreground_color: String,
    pub background_color: String,
}

pub(crate) fn resolve_frame_metadata(
    directory: &Path,
    fallback_output_mode: Option<&str>,
    fallback_color: bool,
    fallback_foreground_color: Option<&str>,
    fallback_background_color: Option<&str>,
) -> ResolvedFrameMetadata {
    let default_output_mode = fallback_output_mode
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| output_mode_from_color_flag(fallback_color));

    let mut resolved = ResolvedFrameMetadata {
        output_mode: default_output_mode,
        foreground_color: fallback_foreground_color
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(default_foreground_color),
        background_color: fallback_background_color
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(default_background_color),
    };

    let details_path = directory.join("details.toml");
    if let Ok(details_text) = fs::read_to_string(&details_path) {
        if let Ok(details) = cascii_core_view::ProjectDetails::from_toml_str(&details_text) {
            if let Some(output) = details.output {
                if !output.trim().is_empty() {
                    resolved.output_mode = output;
                }
            }
            if let Some(color) = details.color {
                if !color.trim().is_empty() {
                    resolved.foreground_color = color;
                }
            }
            if let Some(background_color) = details.background_color {
                if !background_color.trim().is_empty() {
                    resolved.background_color = background_color;
                }
            }
        }
    }

    resolved
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct FileProgress {
    pub file_name: String,
    pub status: String,
    pub message: String,
    pub percentage: Option<f32>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct FrameDirectory {
    pub conversion_id: String,
    pub name: String,
    pub directory_path: String,
    pub source_file_name: String,
    pub custom_name: Option<String>,
    pub frame_count: i32,
    pub fps: u32,
    pub frame_speed: u32,
    pub color: bool,
    pub output_mode: String,
    pub foreground_color: Option<String>,
    pub background_color: Option<String>,
    pub has_text_frames: bool,
    pub has_color_frames: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub(crate) struct FrameFile {
    pub path: String,
    pub name: String,
    pub index: u32,
}

impl FrameFile {
    fn extract_index(stem: &str, fallback: u32) -> u32 {
        if let Some(suffix) = stem.strip_prefix("frame_") {
            suffix.parse::<u32>().unwrap_or(0)
        } else {
            let digits = stem
                .chars()
                .filter(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u32>().unwrap_or(fallback)
        }
    }
}

pub(crate) fn inspect_frame_directory(
    dir: &PathBuf,
) -> Result<(Vec<FrameFile>, bool, bool), String> {
    if !dir.exists() {
        return Err("Directory does not exist".to_string());
    }

    let mut frame_variants: BTreeMap<(u32, String), (Option<PathBuf>, Option<PathBuf>)> =
        BTreeMap::new();
    let mut has_text_frames = false;
    let mut has_color_frames = false;
    let mut fallback_index = 0u32;

    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        let normalized_ext = ext.to_ascii_lowercase();
                        if normalized_ext == "txt" || normalized_ext == "cframe" {
                            if let Some(stem) = path.file_stem().and_then(|n| n.to_str()) {
                                let index = FrameFile::extract_index(stem, fallback_index);
                                fallback_index = fallback_index.saturating_add(1);
                                let entry = frame_variants
                                    .entry((index, stem.to_string()))
                                    .or_insert((None, None));
                                if normalized_ext == "txt" {
                                    has_text_frames = true;
                                    entry.0 = Some(path.clone());
                                } else {
                                    has_color_frames = true;
                                    entry.1 = Some(path.clone());
                                }
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

    let frames = frame_variants
        .into_iter()
        .filter_map(|((index, stem), (txt_path, cframe_path))| {
            let chosen_path = txt_path.or(cframe_path)?;
            let name = chosen_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| stem.clone());
            Some(FrameFile {
                path: chosen_path.to_string_lossy().to_string(),
                name,
                index,
            })
        })
        .collect::<Vec<_>>();

    Ok((frames, has_text_frames, has_color_frames))
}

pub(crate) fn scan_frames_in_dir(dir: &PathBuf) -> Result<Vec<FrameFile>, String> {
    inspect_frame_directory(dir).map(|(frames, _, _)| frames)
}

pub(crate) fn count_frames_and_size(dir: &PathBuf) -> Result<(i32, i64), String> {
    let mut total_size = 0i64;
    let (frames, _, _) = inspect_frame_directory(dir)?;
    let frame_count = frames.len() as i32;
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext = ext.to_ascii_lowercase();
                if ext == "txt" || ext == "cframe" {
                    if let Ok(metadata) = fs::metadata(&path) {
                        total_size += metadata.len() as i64;
                    }
                } else if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name == "details.toml")
                    .unwrap_or(false)
                {
                    if let Ok(metadata) = fs::metadata(&path) {
                        total_size += metadata.len() as i64;
                    }
                }
            }
        }
    }

    Ok((frame_count, total_size))
}

pub(crate) fn generate_random_suffix() -> String {
    let random_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();

    format!("[{}]", random_string)
}
