use crate::database;
use crate::ffmpeg::get_ffmpeg_config;
use crate::settings;
use crate::util::{
    default_background_color, default_foreground_color, generate_random_suffix,
    output_mode_from_color_flag,
};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub(crate) struct CreatePreviewRequest {
    video_path: String,
    timestamp: f64,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: u32,
    color: bool,
    project_id: String,
    source_file_id: String,
}

#[tauri::command]
pub async fn create_preview(
    app: tauri::AppHandle,
    request: CreatePreviewRequest,
) -> Result<database::Preview, String> {
    use cascii::{AsciiConverter, ConversionOptions};
    use std::process::{Command, Stdio};

    let input_path = PathBuf::from(&request.video_path);
    if !input_path.exists() {
        return Err(format!("Video file not found: {}", request.video_path));
    }

    let settings = settings::load();
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let previews_dir = project_dir.join("previews");

    fs::create_dir_all(&previews_dir)
        .map_err(|e| format!("Failed to create previews directory: {}", e))?;

    let frame_number = (request.timestamp * request.fps as f64).floor() as u32;

    let source_name = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("video");
    let random_suffix = generate_random_suffix();
    let folder_name = format!(
        "preview_{}_frame_{:04}{}",
        source_name, frame_number, random_suffix
    );
    let output_dir = previews_dir.join(&folder_name);

    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create preview output directory: {}", e))?;

    let temp_frame_path = output_dir.join("temp_frame.png");

    let ffmpeg_config = get_ffmpeg_config(&app, &settings.ffmpeg_source);
    let ffmpeg_path = ffmpeg_config.ffmpeg_path.clone();

    let ffmpeg_cmd = ffmpeg_path
        .as_ref()
        .and_then(|p| p.to_str())
        .unwrap_or("ffmpeg");

    let status = Command::new(ffmpeg_cmd)
        .arg("-ss")
        .arg(format!("{}", request.timestamp))
        .arg("-i")
        .arg(&request.video_path)
        .arg("-vframes")
        .arg("1")
        .arg("-y")
        .arg(&temp_frame_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| {
            format!(
                "Failed to run ffmpeg: {}. Make sure ffmpeg is installed.",
                e
            )
        })?;

    if !status.success() {
        return Err(format!(
            "ffmpeg frame extraction failed with status: {}",
            status
        ));
    }

    if !temp_frame_path.exists() {
        return Err("Failed to extract frame from video".to_string());
    }

    let converter = AsciiConverter::new().with_ffmpeg_config(ffmpeg_config);

    let output_mode = if request.color {
        cascii::OutputMode::TextAndColor
    } else {
        cascii::OutputMode::TextOnly
    };

    let conv_opts = ConversionOptions::default()
        .with_columns(request.columns)
        .with_font_ratio(request.font_ratio)
        .with_luminance(request.luminance)
        .with_output_mode(output_mode);

    let output_txt_path = output_dir.join("frame_0001.txt");

    converter
        .convert_image(&temp_frame_path, &output_txt_path, &conv_opts)
        .map_err(|e| format!("Failed to convert frame to ASCII: {}", e))?;

    #[derive(serde::Serialize)]
    struct PreviewDetailsFile {
        version: String,
        frames: usize,
        luminance: u8,
        font_ratio: f32,
        columns: u32,
        fps: u32,
        output: String,
        audio: bool,
        background_color: String,
        color: String,
    }

    let details = PreviewDetailsFile {
        version: env!("CARGO_PKG_VERSION").to_string(),
        frames: 1,
        luminance: request.luminance,
        font_ratio: request.font_ratio,
        columns: request.columns,
        fps: request.fps,
        output: output_mode_from_color_flag(request.color),
        audio: false,
        background_color: default_background_color(),
        color: default_foreground_color(),
    };
    let details_toml = toml::to_string_pretty(&details)
        .map_err(|e| format!("Failed to serialize preview details: {}", e))?;
    fs::write(output_dir.join("details.toml"), details_toml)
        .map_err(|e| format!("Failed to write preview details: {}", e))?;

    let _ = fs::remove_file(&temp_frame_path);

    let mut total_size = 0i64;
    if let Ok(entries) = fs::read_dir(&output_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                total_size += metadata.len() as i64;
            }
        }
    }

    let preview = database::Preview {
        id: Uuid::new_v4().to_string(),
        folder_name: folder_name.clone(),
        folder_path: output_dir.to_str().unwrap_or("").to_string(),
        frame_count: 1,
        source_file_id: request.source_file_id,
        project_id: request.project_id,
        settings: database::PreviewSettings {
            luminance: request.luminance,
            font_ratio: request.font_ratio,
            columns: request.columns,
            fps: request.fps,
            color: request.color,
            output_mode: output_mode_from_color_flag(request.color),
            foreground_color: Some(default_foreground_color()),
            background_color: Some(default_background_color()),
        },
        creation_date: Utc::now(),
        total_size,
        custom_name: None,
    };

    database::add_preview(&preview)
        .map_err(|e| format!("Failed to save preview to database: {}", e))?;

    println!("✅ Preview created: {} ({} bytes)", folder_name, total_size);

    Ok(preview)
}

#[tauri::command]
pub fn get_project_previews(project_id: String) -> Result<Vec<database::Preview>, String> {
    database::get_project_previews(&project_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub(crate) struct DeletePreviewRequest {
    preview_id: String,
    folder_path: String,
}

#[tauri::command]
pub fn delete_preview(request: DeletePreviewRequest) -> Result<(), String> {
    let dir_path = PathBuf::from(&request.folder_path);
    if dir_path.exists() {
        fs::remove_dir_all(&dir_path)
            .map_err(|e| format!("Failed to delete preview folder: {}", e))?;
    }

    database::delete_preview(&request.preview_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub(crate) struct RenamePreviewRequest {
    preview_id: String,
    custom_name: Option<String>,
}

#[tauri::command]
pub fn rename_preview(request: RenamePreviewRequest) -> Result<(), String> {
    database::update_preview_custom_name(&request.preview_id, request.custom_name)
        .map_err(|e| format!("Failed to rename preview: {}", e))
}
