use crate::settings;
use crate::util::FileProgress;
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

pub(crate) fn get_video_duration(input_path: &str) -> Result<f32, String> {
    use std::process::Command;

    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            input_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err("Failed to get video duration".to_string());
    }

    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str
        .trim()
        .parse::<f32>()
        .map_err(|e| format!("Failed to parse duration: {}", e))
}

pub(crate) fn ffmpeg_convert_to_mp4(
    input_path: &str,
    output_dir: &str,
    app: &tauri::AppHandle,
    file_name: &str,
) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let input = PathBuf::from(input_path);
    let file_stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Invalid input filename")?;

    let output_path = PathBuf::from(output_dir).join(format!("{}.mp4", file_stem));

    let duration = get_video_duration(input_path).unwrap_or(0.0);

    let mut child = Command::new("ffmpeg")
        .arg("-i")
        .arg(input_path)
        .arg("-c:v")
        .arg("libx264")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-progress")
        .arg("pipe:2")
        .arg("-y")
        .arg(&output_path)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to run ffmpeg: {}. Make sure ffmpeg is installed.",
                e
            )
        })?;

    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.starts_with("out_time_ms=") {
                    if let Some(time_us) = line.strip_prefix("out_time_ms=") {
                        if let Ok(microseconds) = time_us.parse::<f32>() {
                            let current_time = microseconds / 1_000_000.0;
                            if duration > 0.0 {
                                let percentage = (current_time / duration * 100.0).min(99.0);
                                let _ = app.emit(
                                    "file-progress",
                                    FileProgress {
                                        file_name: file_name.to_string(),
                                        status: "processing".to_string(),
                                        message: format!("Converting to MP4... {:.0}%", percentage),
                                        percentage: Some(percentage),
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for ffmpeg: {}", e))?;

    if !status.success() {
        return Err(format!("ffmpeg conversion failed with status: {}", status));
    }

    Ok(output_path
        .to_str()
        .ok_or("Invalid output path")?
        .to_string())
}

pub(crate) fn extract_audio_from_video(
    input_path: &PathBuf,
    audio_dir: &PathBuf,
    random_suffix: &str,
) -> Result<(PathBuf, i64, f64), String> {
    use std::process::{Command, Stdio};

    let duration = get_video_duration(input_path.to_str().unwrap_or("")).unwrap_or(0.0) as f64;

    fs::create_dir_all(audio_dir)
        .map_err(|e| format!("Failed to create audio directory: {}", e))?;

    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");
    let folder_name = format!("{}_audio{}", file_stem, random_suffix);
    let output_folder = audio_dir.join(&folder_name);

    fs::create_dir_all(&output_folder)
        .map_err(|e| format!("Failed to create audio output folder: {}", e))?;

    let output_file = output_folder.join(format!("{}.mp3", file_stem));

    println!(
        "🎵 Extracting audio from: {} to: {}",
        input_path.display(),
        output_file.display()
    );

    let status = Command::new("ffmpeg")
        .arg("-i")
        .arg(input_path)
        .arg("-vn")
        .arg("-acodec")
        .arg("libmp3lame")
        .arg("-q:a")
        .arg("2")
        .arg("-y")
        .arg(&output_file)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| {
            format!(
                "Failed to run ffmpeg for audio extraction: {}. Make sure ffmpeg is installed.",
                e
            )
        })?;

    if !status.success() {
        return Err(format!(
            "ffmpeg audio extraction failed with status: {}",
            status
        ));
    }

    let file_size = fs::metadata(&output_file)
        .map_err(|e| format!("Failed to get audio file size: {}", e))?
        .len() as i64;
    println!(
        "✅ Audio extracted: {} ({} bytes, duration: {}s)",
        output_file.display(),
        file_size,
        duration
    );

    Ok((output_folder, file_size, duration))
}

pub(crate) fn command_exists(cmd: &str) -> bool {
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

#[tauri::command]
pub fn check_system_ffmpeg() -> bool {
    let ffmpeg_exists = command_exists("ffmpeg");
    let ffprobe_exists = command_exists("ffprobe");
    let available = ffmpeg_exists && ffprobe_exists;
    println!(
        "🔍 System ffmpeg check: ffmpeg={}, ffprobe={}, available={}",
        ffmpeg_exists, ffprobe_exists, available
    );
    available
}

pub(crate) fn get_sidecar_paths(app: &tauri::AppHandle) -> Option<(PathBuf, PathBuf)> {
    #[cfg(target_os = "windows")]
    let (ffmpeg_name, ffprobe_name) = ("ffmpeg.exe", "ffprobe.exe");
    #[cfg(not(target_os = "windows"))]
    let (ffmpeg_name, ffprobe_name) = ("ffmpeg", "ffprobe");

    let mut dirs_to_check: Vec<PathBuf> = Vec::new();

    if let Ok(resource_dir) = app.path().resource_dir() {
        dirs_to_check.push(resource_dir.join("binaries"));
        dirs_to_check.push(resource_dir);
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            dirs_to_check.push(exe_dir.join("binaries"));
            if let Some(parent) = exe_dir.parent() {
                if let Some(grandparent) = parent.parent() {
                    if let Some(great_grandparent) = grandparent.parent() {
                        dirs_to_check.push(great_grandparent.join("src-tauri").join("binaries"));
                    }
                }
            }
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        dirs_to_check.push(cwd.join("src-tauri").join("binaries"));
        dirs_to_check.push(cwd.join("binaries"));
    }

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

#[tauri::command]
pub fn check_sidecar_ffmpeg(app: tauri::AppHandle) -> bool {
    let available = get_sidecar_paths(&app).is_some();
    println!(
        "🔍 Sidecar ffmpeg check: {}",
        if available { "available" } else { "not found" }
    );
    available
}

pub(crate) fn get_ffmpeg_config(
    app: &tauri::AppHandle,
    ffmpeg_source: &settings::FfmpegSource,
) -> cascii::FfmpegConfig {
    match ffmpeg_source {
        settings::FfmpegSource::System => {
            if command_exists("ffmpeg") && command_exists("ffprobe") {
                println!("🎬 Using system ffmpeg (from settings)");
                cascii::FfmpegConfig::new()
            } else {
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

pub(crate) fn get_sidecar_config(app: &tauri::AppHandle) -> cascii::FfmpegConfig {
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
