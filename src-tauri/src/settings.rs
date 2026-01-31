use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DefaultBehavior { Move, Copy }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeleteMode { Soft, Hard }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FfmpegSource {
    /// Use ffmpeg from system PATH
    System,
    /// Use bundled ffmpeg sidecar
    Sidecar
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub id: Option<i64>,
    pub output_directory: String,
    pub default_behavior: DefaultBehavior,
    pub delete_mode: DeleteMode,
    pub debug_logs: bool,
    #[serde(default = "default_loop_enabled")]
    pub loop_enabled: bool,
    #[serde(default = "default_color_frames")]
    pub color_frames_default: bool,
    #[serde(default = "default_extract_audio")]
    pub extract_audio_default: bool,
    #[serde(default = "default_ffmpeg_source")]
    pub ffmpeg_source: FfmpegSource,
}

fn default_loop_enabled() -> bool { true }
fn default_color_frames() -> bool { true }
fn default_extract_audio() -> bool { false }
fn default_ffmpeg_source() -> FfmpegSource { FfmpegSource::System }

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: None,
            output_directory: default_output_dir().to_string_lossy().to_string(),
            default_behavior: DefaultBehavior::Move,
            delete_mode: DeleteMode::Soft,
            debug_logs: true,
            loop_enabled: true,
            color_frames_default: true,
            extract_audio_default: false,
            ffmpeg_source: FfmpegSource::System,
        }
    }
}

fn default_output_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(|| dirs::home_dir())
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn app_support_dir() -> PathBuf {
    // macOS: ~/Library/Application Support/cascii_studio
    // Linux: ~/.config/cascii_studio
    // Windows: %APPDATA%\cascii_studio
    dirs::config_dir().unwrap_or_else(|| dirs::home_dir().unwrap_or_default()).join("cascii_studio")
}

fn settings_path() -> PathBuf { app_support_dir().join("settings.json") }

pub fn load() -> Settings {
    let p = settings_path();
    if let Some(parent) = p.parent() { let _ = fs::create_dir_all(parent); }
    match fs::read_to_string(&p) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => {
            let s = Settings::default();
            let _ = save(&s);
            s
        }
    }
}

pub fn save(s: &Settings) -> Result<(), String> {
    let p = settings_path();
    if let Some(parent) = p.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let body = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&p, body).map_err(|e| e.to_string())
}
