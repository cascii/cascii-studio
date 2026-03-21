use crate::util::is_video_file;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

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

    fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create media cache dir: {}", e))?;
    Ok(cache_dir)
}

fn guess_mime_type(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        "gif" => Some("image/gif".to_string()),
        "webp" => Some("image/webp".to_string()),
        "mp4" => Some("video/mp4".to_string()),
        "webm" => Some("video/webm".to_string()),
        "mov" => Some("video/quicktime".to_string()),
        "avi" => Some("video/x-msvideo".to_string()),
        "mkv" => Some("video/x-matroska".to_string()),
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

/// Build a URL-safe cache filename from a source path.
/// Characters like # (fragment delimiter), ?, [, ] etc. in filenames break
/// asset:// URLs, so we replace anything that isn't alphanumeric, `-`, `_`, or `.`
/// and append a path hash to avoid collisions.
fn safe_cache_filename(source_path: &Path) -> String {
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let safe_stem: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut hasher = DefaultHasher::new();
    source_path.hash(&mut hasher);
    format!("{}_{:08x}.{}", safe_stem, hasher.finish() as u32, ext)
}

#[tauri::command]
pub fn prepare_media(path: String) -> Result<PreparedMedia, String> {
    let source_path = PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Invalid source path: {}", e))?;
    let cache_dir = get_media_cache_dir()?;
    let cached_path = cache_dir.join(safe_cache_filename(&source_path));
    if !cached_path.exists() {
        match fs::hard_link(&source_path, &cached_path) {
            Ok(_) => {}
            Err(_) => {
                fs::copy(&source_path, &cached_path)
                    .map_err(|e| format!("Failed to copy file to cache: {}", e))?;
            }
        }
    }

    let media_kind = determine_media_kind(&source_path);
    let mime_type = guess_mime_type(&source_path);
    let cached_abs_path = cached_path
        .to_str()
        .ok_or_else(|| "Invalid cached path".to_string())?
        .to_string();
    Ok(PreparedMedia {
        cached_abs_path,
        media_kind,
        mime_type,
        width: None,
        height: None,
    })
}
