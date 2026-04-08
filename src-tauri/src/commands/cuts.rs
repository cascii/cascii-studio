use crate::database;
use crate::ffmpeg::get_ffmpeg_config;
use crate::settings;
use crate::util::FileProgress;
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;
use uuid::Uuid;

fn emit_progress(
    app: &tauri::AppHandle,
    event_name: &str,
    file_name: &str,
    status: &str,
    message: impl Into<String>,
    percentage: Option<f32>,
) {
    let _ = app.emit(
        event_name,
        FileProgress {
            file_name: file_name.to_string(),
            status: status.to_string(),
            message: message.into(),
            percentage,
        },
    );
}

fn parse_ffmpeg_timestamp_seconds(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<f64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

fn parse_ffmpeg_progress_seconds(line: &str) -> Option<f64> {
    if let Some(raw) = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
    {
        let micros = raw.trim().parse::<f64>().ok()?;
        return Some(micros / 1_000_000.0);
    }

    line.strip_prefix("out_time=")
        .and_then(parse_ffmpeg_timestamp_seconds)
}

fn probe_duration_seconds(ffprobe_cmd: &str, path: &PathBuf) -> Option<f64> {
    use std::process::Command;

    Command::new(ffprobe_cmd)
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("csv=p=0")
        .arg(path)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|text| text.trim().parse::<f64>().ok())
        .filter(|duration| duration.is_finite() && *duration > 0.0)
}

#[derive(serde::Deserialize)]
pub(crate) struct CutVideoRequest {
    source_file_path: String,
    project_id: String,
    source_file_id: String,
    start_time: f64,
    end_time: f64,
}

#[derive(serde::Deserialize)]
pub(crate) struct CutVideoArgs {
    request: CutVideoRequest,
}

#[tauri::command]
pub async fn cut_video(
    args: CutVideoArgs,
    app: tauri::AppHandle,
) -> Result<database::VideoCut, String> {
    tokio::task::spawn_blocking(move || cut_video_blocking(args.request, app))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn cut_video_blocking(request: CutVideoRequest, app: tauri::AppHandle) -> Result<database::VideoCut, String> {
    use std::process::{Command, Stdio};

    let settings = settings::load();
    let project = database::get_project(&request.project_id).map_err(|e| format!("Failed to get project: {}", e))?;

    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let cuts_dir = project_dir.join("cuts");

    fs::create_dir_all(&cuts_dir).map_err(|e| format!("Failed to create cuts directory: {}", e))?;

    let input_path = PathBuf::from(&request.source_file_path);
    let file_stem = input_path.file_stem().and_then(|s| s.to_str()).ok_or("Invalid input filename")?;
    let cut_id = Uuid::new_v4().to_string();
    let output_filename = format!("{}_cut_{}.mp4", file_stem, &cut_id[..8]);
    let output_path = cuts_dir.join(&output_filename);
    let duration = request.end_time - request.start_time;

    let _ = app.emit("cut-progress", FileProgress {file_name: output_filename.clone(), status: "processing".to_string(), message: "Cutting video...".to_string(), percentage: Some(0.0)});

    println!("🎬 Cutting video: {} -> {}", request.source_file_path, output_path.display());
    println!("   Start: {}s, End: {}s, Duration: {}s", request.start_time, request.end_time, duration);

    let status = Command::new("ffmpeg")
        .arg("-ss")
        .arg(format!("{}", request.start_time))
        .arg("-i")
        .arg(&request.source_file_path)
        .arg("-t")
        .arg(format!("{}", duration))
        .arg("-c:v")
        .arg("libx264")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-avoid_negative_ts")
        .arg("make_zero")
        .arg("-y")
        .arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| {format!("Failed to run ffmpeg: {}. Make sure ffmpeg is installed.", e)})?;

    if !status.success() {
        return Err(format!("ffmpeg cut failed with status: {}", status));
    }

    let file_size = fs::metadata(&output_path)
        .map_err(|e| format!("Failed to get file size: {}", e))?
        .len() as i64;

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

    let _ = app.emit("cut-progress", FileProgress {file_name: output_filename, status: "completed".to_string(), message: "Cut completed".to_string(), percentage: Some(100.0)});

    println!("✅ Cut saved: {} ({} bytes)", cut.file_path, file_size);

    Ok(cut)
}

#[tauri::command]
pub fn get_project_cuts(project_id: String) -> Result<Vec<database::VideoCut>, String> {
    database::get_project_cuts(&project_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub(crate) struct DeleteCutRequest {
    cut_id: String,
    file_path: String,
}

#[tauri::command]
pub fn delete_cut(request: DeleteCutRequest) -> Result<(), String> {
    let path = PathBuf::from(&request.file_path);
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete cut file: {}", e))?;
    }
    database::delete_cut(&request.cut_id).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub(crate) struct RenameCutRequest {
    cut_id: String,
    custom_name: Option<String>,
}

#[tauri::command]
pub fn rename_cut(request: RenameCutRequest) -> Result<(), String> {
    database::update_cut_custom_name(&request.cut_id, request.custom_name).map_err(|e| format!("Failed to rename cut: {}", e))
}

#[derive(serde::Deserialize)]
pub(crate) struct PreprocessVideoRequest {
    source_file_path: String,
    project_id: String,
    source_file_id: String,
    preset: Option<String>,
    custom_filter: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct PreprocessVideoArgs {
    request: PreprocessVideoRequest,
}

#[tauri::command]
pub async fn preprocess_video(args: PreprocessVideoArgs, app: tauri::AppHandle) -> Result<database::VideoCut, String> {
    tokio::task::spawn_blocking(move || preprocess_video_blocking(args.request, app)).await.map_err(|e| format!("Task failed: {}", e))?
}

fn preprocess_video_blocking(request: PreprocessVideoRequest, app: tauri::AppHandle) -> Result<database::VideoCut, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let filter = match request.preset.as_deref() {
        Some("other") => {
            cascii::preprocessing::resolve_preprocess_filter(request.custom_filter.as_deref(), None).map_err(|e| format!("Invalid preprocessing filter: {}", e))?
        }
        Some(preset_name) => {
            cascii::preprocessing::resolve_preprocess_filter(None, Some(preset_name)).map_err(|e| format!("Invalid preprocessing preset: {}", e))?
        }
        None => return Err("No preprocessing preset selected".to_string()),
    };
    let filter = filter.ok_or("Empty preprocessing filter")?;
    let current_settings = settings::load();
    let project = database::get_project(&request.project_id).map_err(|e| format!("Failed to get project: {}", e))?;
    let project_dir = PathBuf::from(&current_settings.output_directory).join(&project.project_path);
    let cuts_dir = project_dir.join("cuts");
    fs::create_dir_all(&cuts_dir).map_err(|e| format!("Failed to create cuts directory: {}", e))?;

    let input_path = PathBuf::from(&request.source_file_path);
    let file_stem = input_path.file_stem().and_then(|s| s.to_str()).ok_or("Invalid input filename")?;

    let cut_id = Uuid::new_v4().to_string();
    let in_place = current_settings.preprocess_output == settings::PreprocessOutput::CurrentFile;

    let output_path = if in_place {
        input_path.with_extension("preprocessed.mp4")
    } else {
        let output_filename = format!("{}_preprocessed_{}.mp4", file_stem, &cut_id[..8]);
        cuts_dir.join(&output_filename)
    };
    let progress_file_name = input_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(file_stem)
        .to_string();

    emit_progress(&app, "preprocess-progress", &progress_file_name, "processing", "Preprocessing video...", Some(0.0));

    println!("🎨 Preprocessing video: {} -> {}", request.source_file_path, output_path.display());

    let ffmpeg_config = get_ffmpeg_config(&app, &current_settings.ffmpeg_source);
    let ffmpeg_cmd = ffmpeg_config
        .ffmpeg_path
        .as_ref()
        .and_then(|p| p.to_str())
        .unwrap_or("ffmpeg");
    let ffprobe_cmd = ffmpeg_config
        .ffprobe_path
        .as_ref()
        .and_then(|p| p.to_str())
        .unwrap_or("ffprobe");
    let source_duration = probe_duration_seconds(ffprobe_cmd, &input_path).unwrap_or(0.0);

    let mut child = Command::new(ffmpeg_cmd)
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(&request.source_file_path)
        .arg("-vf")
        .arg(&filter)
        .arg("-c:v")
        .arg("libx264")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-y")
        .arg(&output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {format!("Failed to run ffmpeg: {}. Make sure ffmpeg is installed.", e)})?;

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        let mut last_reported = 0u8;

        for line in reader.lines().map_while(Result::ok) {
            if source_duration <= 0.0 {
                continue;
            }

            if let Some(processed_seconds) = parse_ffmpeg_progress_seconds(&line) {
                let percentage = ((processed_seconds / source_duration) * 100.0).clamp(0.0, 99.0).floor() as u8;

                if percentage > last_reported {
                    last_reported = percentage;
                    emit_progress(&app, "preprocess-progress", &progress_file_name, "processing",  format!("Preprocessing video... {}%", percentage), Some(percentage as f32));
                }
            }
        }
    }

    let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;

    if !status.success() {
        emit_progress(&app, "preprocess-progress", &progress_file_name, "error", "Preprocessing failed", None);
        let _ = fs::remove_file(&output_path);
        return Err("ffmpeg preprocessing failed".to_string());
    }

    if in_place {
        fs::rename(&output_path, &input_path).map_err(|e| format!("Failed to replace original file: {}", e))?;
    }

    let final_path = if in_place { &input_path } else { &output_path };
    let file_size = fs::metadata(final_path).map_err(|e| format!("Failed to get file size: {}", e))?.len() as i64;

    let duration = probe_duration_seconds(ffprobe_cmd, &final_path.to_path_buf()).unwrap_or(0.0);

    let cut = database::VideoCut {
        id: cut_id,
        project_id: request.project_id,
        source_file_id: request.source_file_id,
        file_path: final_path.to_str().unwrap_or("").to_string(),
        date_added: Utc::now(),
        size: file_size,
        custom_name: Some(format!("Preprocessed {}", file_stem)),
        start_time: 0.0,
        end_time: duration,
        duration,
    };

    database::add_video_cut(&cut).map_err(|e| format!("Failed to save to database: {}", e))?;
    emit_progress(&app, "preprocess-progress", &progress_file_name, "completed", "Preprocessing completed", Some(100.0));

    println!("✅ Preprocessed video saved: {} ({} bytes)",  final_path.display(), file_size);
    Ok(cut)
}
