use crate::database;
use crate::ffmpeg::{command_exists, get_ffmpeg_config, get_sidecar_paths};
use crate::settings;
use cascii::{AsciiConverter, ToVideoOptions};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::Emitter;
use uuid::Uuid;

#[derive(Clone, serde::Serialize)]
struct ExportProgress {
    stage: String,
    message: String,
    clip_index: usize,
    total_clips: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Mp4,
    Mov,
    Mkv,
}

impl ExportFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Mkv => "mkv",
        }
    }

    fn append_container_args(self, args: &mut Vec<String>) {
        if matches!(self, Self::Mp4) {
            args.push("-movflags".into());
            args.push("+faststart".into());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum ExportResolution {
    #[serde(rename = "720p")]
    P720,
    #[serde(rename = "1080p")]
    P1080,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "2160p")]
    P2160,
}

impl ExportResolution {
    fn dimensions(self) -> (u32, u32) {
        match self {
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 => (3840, 2160),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportQuality {
    Draft,
    Balanced,
    High,
}

impl ExportQuality {
    fn crf(self) -> u8 {
        match self {
            Self::Draft => 28,
            Self::Balanced => 23,
            Self::High => 18,
        }
    }

    fn preset(self) -> &'static str {
        match self {
            Self::Draft => "veryfast",
            Self::Balanced => "medium",
            Self::High => "slow",
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineExportRequest {
    pub project_id: String,
    pub output_path: String,
    pub format: ExportFormat,
    pub resolution: ExportResolution,
    pub frame_rate: u32,
    pub quality: ExportQuality,
    pub include_audio: bool,
}

/// Resolve the file path (video) or directory path (frames) for a timeline clip.
/// Returns (path, is_frames, fps).
fn resolve_clip_media(clip: &database::TimelineClip) -> Result<(String, bool, u32), String> {
    match (&clip.media_type, &clip.resource_kind) {
        (database::TimelineMediaType::Video, database::TimelineResourceKind::Source) => {
            let source = database::get_source(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query source: {e}"))?
                .ok_or_else(|| format!("source not found: {}", clip.actual_resource_id))?;
            Ok((source.file_path, false, 30))
        }
        (database::TimelineMediaType::Video, database::TimelineResourceKind::Cut) => {
            let cut = database::get_cut(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query cut: {e}"))?
                .ok_or_else(|| format!("cut not found: {}", clip.actual_resource_id))?;
            Ok((cut.file_path, false, 30))
        }
        (
            database::TimelineMediaType::Frames | database::TimelineMediaType::Frame,
            database::TimelineResourceKind::AsciiConversion,
        ) => {
            let conv = database::get_conversion(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query conversion: {e}"))?
                .ok_or_else(|| format!("conversion not found: {}", clip.actual_resource_id))?;
            let fps = if conv.settings.frame_speed > 0 {
                conv.settings.frame_speed
            } else {
                conv.settings.fps
            };
            Ok((conv.folder_path, true, fps))
        }
        (
            database::TimelineMediaType::Frames | database::TimelineMediaType::Frame,
            database::TimelineResourceKind::Preview,
        ) => {
            let preview = database::get_preview(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query preview: {e}"))?
                .ok_or_else(|| format!("preview not found: {}", clip.actual_resource_id))?;
            Ok((preview.folder_path, true, preview.settings.fps))
        }
        _ => Err(format!(
            "unsupported clip type: {:?}/{:?}",
            clip.media_type, clip.resource_kind
        )),
    }
}

fn render_frames_to_intermediate_video(
    frames_dir: &str,
    fps: u32,
    output_path: &PathBuf,
    ffmpeg_config: &cascii::FfmpegConfig,
    use_colors: bool,
    include_audio: bool,
    quality: ExportQuality,
) -> Result<(), String> {
    println!(
        "  🎞️  rendering frames from {} at {}fps → {} (colors={}, audio={})",
        frames_dir,
        fps,
        output_path.display(),
        use_colors,
        include_audio
    );

    let converter = AsciiConverter::new().with_ffmpeg_config(ffmpeg_config.clone());
    let to_video_opts = ToVideoOptions {
        output_path: output_path.clone(),
        font_size: 14.0,
        crf: quality.crf(),
        mux_audio: include_audio,
        use_colors: Some(use_colors),
    };

    converter
        .render_frames_to_video(
            std::path::Path::new(frames_dir),
            fps,
            &to_video_opts,
            |progress| {
                println!("    render progress: {:?}", progress);
            },
        )
        .map_err(|e| format!("failed to render frames to video: {e}"))?;

    println!("  ✅ rendered {}", output_path.display());
    Ok(())
}

fn render_frames_to_intermediate_mp4(
    frames_dir: &str,
    fps: u32,
    output_path: &PathBuf,
    ffmpeg_config: &cascii::FfmpegConfig,
    use_colors: bool,
) -> Result<(), String> {
    render_frames_to_intermediate_video(
        frames_dir,
        fps,
        output_path,
        ffmpeg_config,
        use_colors,
        false,
        ExportQuality::High,
    )
}

fn run_ffmpeg_command(ffmpeg_cmd: &OsString, args: &[String], context: &str) -> Result<(), String> {
    let output = Command::new(ffmpeg_cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("{context}: failed to run ffmpeg: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            Err(format!("{context}: ffmpeg exited with {}", output.status))
        } else {
            Err(format!("{context}: {stderr}"))
        }
    }
}

fn resolve_ffmpeg_binaries(
    app: &tauri::AppHandle,
    ffmpeg_source: &settings::FfmpegSource,
) -> Result<(OsString, OsString), String> {
    match ffmpeg_source {
        settings::FfmpegSource::System => {
            if command_exists("ffmpeg") && command_exists("ffprobe") {
                Ok((OsString::from("ffmpeg"), OsString::from("ffprobe")))
            } else if let Some((ffmpeg_path, ffprobe_path)) = get_sidecar_paths(app) {
                Ok((ffmpeg_path.into_os_string(), ffprobe_path.into_os_string()))
            } else {
                Err("ffmpeg/ffprobe not found in PATH or sidecar binaries".to_string())
            }
        }
        settings::FfmpegSource::Sidecar => get_sidecar_paths(app)
            .map(|(ffmpeg_path, ffprobe_path)| {
                (ffmpeg_path.into_os_string(), ffprobe_path.into_os_string())
            })
            .ok_or_else(|| "sidecar ffmpeg/ffprobe not found".to_string()),
    }
}

fn has_audio_track(ffprobe_cmd: &OsString, input_path: &Path) -> Result<bool, String> {
    let output = Command::new(ffprobe_cmd)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a")
        .arg("-show_entries")
        .arg("stream=index")
        .arg("-of")
        .arg("csv=p=0")
        .arg(input_path)
        .output()
        .map_err(|e| format!("failed to run ffprobe on {}: {e}", input_path.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffprobe failed for {}: {}",
            input_path.display(),
            stderr.trim()
        ));
    }

    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

fn normalize_segment(
    ffmpeg_cmd: &OsString,
    ffprobe_cmd: &OsString,
    input_path: &Path,
    output_path: &Path,
    resolution: ExportResolution,
    frame_rate: u32,
    quality: ExportQuality,
    include_audio: bool,
) -> Result<(), String> {
    let (target_width, target_height) = resolution.dimensions();
    let filter_graph = format!(
        "scale=w={target_width}:h={target_height}:force_original_aspect_ratio=decrease,pad={target_width}:{target_height}:(ow-iw)/2:(oh-ih)/2:black,fps={frame_rate},format=yuv420p"
    );
    let has_audio = if include_audio {
        has_audio_track(ffprobe_cmd, input_path)?
    } else {
        false
    };

    let mut args = vec!["-y".into(), "-i".into(), input_path.display().to_string()];

    if include_audio && !has_audio {
        args.push("-f".into());
        args.push("lavfi".into());
        args.push("-i".into());
        args.push("anullsrc=channel_layout=stereo:sample_rate=44100".into());
    }

    args.push("-map".into());
    args.push("0:v:0".into());

    if include_audio {
        args.push("-map".into());
        args.push(if has_audio { "0:a:0" } else { "1:a:0" }.into());
    }

    args.push("-vf".into());
    args.push(filter_graph);
    args.push("-c:v".into());
    args.push("libx264".into());
    args.push("-crf".into());
    args.push(quality.crf().to_string());
    args.push("-preset".into());
    args.push(quality.preset().into());
    args.push("-g".into());
    args.push(frame_rate.to_string());
    args.push("-pix_fmt".into());
    args.push("yuv420p".into());

    if include_audio {
        args.push("-c:a".into());
        args.push("aac".into());
        args.push("-b:a".into());
        args.push("192k".into());
        args.push("-shortest".into());
    } else {
        args.push("-an".into());
    }

    args.push("-movflags".into());
    args.push("+faststart".into());
    args.push(output_path.display().to_string());

    run_ffmpeg_command(
        ffmpeg_cmd,
        &args,
        &format!("failed to normalize segment {}", input_path.display()),
    )
}

fn finalize_single_segment(
    ffmpeg_cmd: &OsString,
    segment_path: &Path,
    output_path: &Path,
    format: ExportFormat,
    quality: ExportQuality,
    include_audio: bool,
    frame_rate: u32,
) -> Result<(), String> {
    let mut copy_args = vec![
        "-y".into(),
        "-i".into(),
        segment_path.display().to_string(),
        "-map".into(),
        "0:v:0".into(),
    ];

    if include_audio {
        copy_args.push("-map".into());
        copy_args.push("0:a:0".into());
        copy_args.push("-c".into());
        copy_args.push("copy".into());
    } else {
        copy_args.push("-c:v".into());
        copy_args.push("copy".into());
        copy_args.push("-an".into());
    }

    format.append_container_args(&mut copy_args);
    copy_args.push(output_path.display().to_string());

    if run_ffmpeg_command(
        ffmpeg_cmd,
        &copy_args,
        &format!("failed to finalize export {}", output_path.display()),
    )
    .is_ok()
    {
        return Ok(());
    }

    let mut fallback_args = vec![
        "-y".into(),
        "-i".into(),
        segment_path.display().to_string(),
        "-map".into(),
        "0:v:0".into(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        quality.crf().to_string(),
        "-preset".into(),
        quality.preset().into(),
        "-g".into(),
        frame_rate.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];

    if include_audio {
        fallback_args.push("-map".into());
        fallback_args.push("0:a:0".into());
        fallback_args.push("-c:a".into());
        fallback_args.push("aac".into());
        fallback_args.push("-b:a".into());
        fallback_args.push("192k".into());
    } else {
        fallback_args.push("-an".into());
    }

    format.append_container_args(&mut fallback_args);
    fallback_args.push(output_path.display().to_string());

    run_ffmpeg_command(
        ffmpeg_cmd,
        &fallback_args,
        &format!("failed to re-encode export {}", output_path.display()),
    )
}

fn finalize_multiple_segments(
    ffmpeg_cmd: &OsString,
    segments: &[PathBuf],
    output_path: &Path,
    format: ExportFormat,
    quality: ExportQuality,
    include_audio: bool,
    frame_rate: u32,
) -> Result<(), String> {
    let tmp_dir = output_path.parent().unwrap_or(Path::new("."));
    let concat_list_path = tmp_dir.join(".export_concat_list.txt");
    let list_content: String = segments
        .iter()
        .map(|p| format!("file '{}'\n", p.display()))
        .collect();
    fs::write(&concat_list_path, &list_content)
        .map_err(|e| format!("failed to write concat list: {e}"))?;

    let mut copy_args = vec![
        "-y".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        concat_list_path.display().to_string(),
        "-c".into(),
        "copy".into(),
    ];
    format.append_container_args(&mut copy_args);
    copy_args.push(output_path.display().to_string());

    let copy_result = run_ffmpeg_command(
        ffmpeg_cmd,
        &copy_args,
        &format!("failed to concatenate {}", output_path.display()),
    );

    if copy_result.is_ok() {
        let _ = fs::remove_file(&concat_list_path);
        return Ok(());
    }

    let mut fallback_args = vec![
        "-y".into(),
        "-f".into(),
        "concat".into(),
        "-safe".into(),
        "0".into(),
        "-i".into(),
        concat_list_path.display().to_string(),
        "-c:v".into(),
        "libx264".into(),
        "-crf".into(),
        quality.crf().to_string(),
        "-preset".into(),
        quality.preset().into(),
        "-g".into(),
        frame_rate.to_string(),
        "-pix_fmt".into(),
        "yuv420p".into(),
    ];

    if include_audio {
        fallback_args.push("-c:a".into());
        fallback_args.push("aac".into());
        fallback_args.push("-b:a".into());
        fallback_args.push("192k".into());
    } else {
        fallback_args.push("-an".into());
    }

    format.append_container_args(&mut fallback_args);
    fallback_args.push(output_path.display().to_string());

    let result = run_ffmpeg_command(
        ffmpeg_cmd,
        &fallback_args,
        &format!(
            "failed to re-encode concatenated export {}",
            output_path.display()
        ),
    );
    let _ = fs::remove_file(&concat_list_path);
    result
}

fn finalize_export_segments(
    ffmpeg_cmd: &OsString,
    segments: &[PathBuf],
    output_path: &Path,
    format: ExportFormat,
    quality: ExportQuality,
    include_audio: bool,
    frame_rate: u32,
) -> Result<(), String> {
    match segments {
        [] => Err("no normalized segments to export".to_string()),
        [single] => finalize_single_segment(
            ffmpeg_cmd,
            single,
            output_path,
            format,
            quality,
            include_audio,
            frame_rate,
        ),
        _ => finalize_multiple_segments(
            ffmpeg_cmd,
            segments,
            output_path,
            format,
            quality,
            include_audio,
            frame_rate,
        ),
    }
}

fn validate_export_request(request: &TimelineExportRequest) -> Result<(), String> {
    if !matches!(request.frame_rate, 24 | 30 | 60) {
        return Err(format!(
            "unsupported export frame rate: {}",
            request.frame_rate
        ));
    }

    Ok(())
}

fn ensure_output_path(output_path: &str, format: ExportFormat) -> PathBuf {
    let mut path = PathBuf::from(output_path);
    path.set_extension(format.extension());
    path
}

#[tauri::command]
pub async fn export_timeline_video(
    request: TimelineExportRequest,
    app: tauri::AppHandle,
) -> Result<String, String> {
    println!(
        "🎬 export_timeline_video: project={}, output={}, format={:?}, resolution={:?}, frame_rate={}, quality={:?}, include_audio={}",
        request.project_id,
        request.output_path,
        request.format,
        request.resolution,
        request.frame_rate,
        request.quality,
        request.include_audio
    );

    tokio::task::spawn_blocking(move || {
        let emit_progress = |stage: &str, message: &str, clip_index: usize, total_clips: usize| {
            let _ = app.emit(
                "export-progress",
                ExportProgress {
                    stage: stage.to_string(),
                    message: message.to_string(),
                    clip_index,
                    total_clips,
                },
            );
        };

        validate_export_request(&request)?;

        let timeline = database::get_active_project_timeline(&request.project_id)
            .map_err(|e| format!("failed to get timeline: {e}"))?;
        let clips = timeline.clips;

        if clips.is_empty() {
            return Err("timeline is empty — nothing to export".to_string());
        }

        let settings = settings::load();
        let ffmpeg_config = get_ffmpeg_config(&app, &settings.ffmpeg_source);
        let (ffmpeg_cmd, ffprobe_cmd) = resolve_ffmpeg_binaries(&app, &settings.ffmpeg_source)?;
        let output_path = ensure_output_path(&request.output_path, request.format);
        let tmp_dir = output_path
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!(".cascii_export_tmp_{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

        println!("  📋 timeline has {} clips", clips.len());
        emit_progress("resolving", "Resolving timeline clips...", 0, clips.len());

        let export_result = (|| -> Result<String, String> {
            let mut normalized_segments: Vec<PathBuf> = Vec::new();

            for (index, clip) in clips.iter().enumerate() {
                println!(
                    "  📎 clip {}/{}: {:?} / {:?} → {}",
                    index + 1,
                    clips.len(),
                    clip.media_type,
                    clip.resource_kind,
                    clip.actual_resource_id
                );
                emit_progress(
                    "processing",
                    &format!("Processing clip {}/{}...", index + 1, clips.len()),
                    index + 1,
                    clips.len(),
                );

                let (media_path, is_frames, clip_fps) = resolve_clip_media(clip)?;
                let normalized_segment = tmp_dir.join(format!("segment_{:04}.mp4", index));

                if is_frames {
                    let raw_segment = tmp_dir.join(format!("segment_{:04}_raw.mp4", index));
                    let use_colors = matches!(
                        clip.frame_render_mode,
                        Some(database::FrameRenderMode::ColorFrames)
                    );

                    render_frames_to_intermediate_video(
                        &media_path,
                        clip_fps,
                        &raw_segment,
                        &ffmpeg_config,
                        use_colors,
                        request.include_audio,
                        request.quality,
                    )?;

                    normalize_segment(
                        &ffmpeg_cmd,
                        &ffprobe_cmd,
                        &raw_segment,
                        &normalized_segment,
                        request.resolution,
                        request.frame_rate,
                        request.quality,
                        request.include_audio,
                    )?;

                    let _ = fs::remove_file(&raw_segment);
                } else {
                    normalize_segment(
                        &ffmpeg_cmd,
                        &ffprobe_cmd,
                        Path::new(&media_path),
                        &normalized_segment,
                        request.resolution,
                        request.frame_rate,
                        request.quality,
                        request.include_audio,
                    )?;
                }

                normalized_segments.push(normalized_segment);
            }

            emit_progress(
                "concatenating",
                "Finalizing export...",
                clips.len(),
                clips.len(),
            );

            finalize_export_segments(
                &ffmpeg_cmd,
                &normalized_segments,
                &output_path,
                request.format,
                request.quality,
                request.include_audio,
                request.frame_rate,
            )?;

            let file_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
            println!(
                "✅ export complete: {} ({} bytes)",
                output_path.display(),
                file_size
            );
            emit_progress(
                "complete",
                &format!("Export complete ({} bytes)", file_size),
                clips.len(),
                clips.len(),
            );

            Ok(output_path.display().to_string())
        })();

        if let Err(error) = &export_result {
            emit_progress("error", error, clips.len(), clips.len());
        }

        let _ = fs::remove_dir_all(&tmp_dir);
        export_result
    })
    .await
    .map_err(|e| format!("export task failed: {e}"))?
}

fn concat_videos_ffmpeg(segments: &[PathBuf], output_path: &PathBuf) -> Result<(), String> {
    println!(
        "🔗 concatenating {} segments → {}",
        segments.len(),
        output_path.display()
    );

    let tmp_dir = output_path.parent().unwrap_or(std::path::Path::new("."));
    let concat_list_path = tmp_dir.join(".export_concat_list.txt");

    let list_content: String = segments
        .iter()
        .map(|p| format!("file '{}'\n", p.display()))
        .collect();
    fs::write(&concat_list_path, &list_content)
        .map_err(|e| format!("failed to write concat list: {e}"))?;

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0")
        .arg("-i")
        .arg(&concat_list_path)
        .arg("-c")
        .arg("copy")
        .arg("-movflags")
        .arg("+faststart")
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("failed to run ffmpeg concat: {e}"))?;

    let _ = fs::remove_file(&concat_list_path);

    if !status.success() {
        println!("  ⚠️ concat copy failed, re-encoding...");
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f")
            .arg("concat")
            .arg("-safe")
            .arg("0")
            .arg("-i")
            .arg(&{
                let path = tmp_dir.join(".export_concat_list.txt");
                fs::write(&path, &list_content)
                    .map_err(|e| format!("failed to write concat list: {e}"))?;
                path
            })
            .arg("-c:v")
            .arg("libx264")
            .arg("-crf")
            .arg("18")
            .arg("-preset")
            .arg("medium")
            .arg("-c:a")
            .arg("aac")
            .arg("-movflags")
            .arg("+faststart")
            .arg(output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .map_err(|e| format!("ffmpeg re-encode concat failed: {e}"))?;

        let _ = fs::remove_file(tmp_dir.join(".export_concat_list.txt"));

        if !status.success() {
            return Err("ffmpeg concatenation failed".to_string());
        }
    }

    println!("✅ concatenated to {}", output_path.display());
    Ok(())
}

#[tauri::command]
pub async fn export_timeline_mp4(
    project_id: String,
    output_path: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    println!(
        "🎬 export_timeline_mp4: project={}, output={}",
        project_id, output_path
    );

    tokio::task::spawn_blocking(move || {
        let emit_progress = |stage: &str, message: &str, clip_index: usize, total_clips: usize| {
            let _ = app.emit(
                "export-progress",
                ExportProgress {
                    stage: stage.to_string(),
                    message: message.to_string(),
                    clip_index,
                    total_clips,
                },
            );
        };

        let timeline = database::get_active_project_timeline(&project_id)
            .map_err(|e| format!("failed to get timeline: {e}"))?;
        let clips = timeline.clips;

        if clips.is_empty() {
            return Err("timeline is empty — nothing to export".to_string());
        }

        println!("  📋 timeline has {} clips", clips.len());
        emit_progress("resolving", "Resolving timeline clips...", 0, clips.len());

        let settings = settings::load();
        let ffmpeg_config = get_ffmpeg_config(&app, &settings.ffmpeg_source);

        let output = PathBuf::from(&output_path);
        let tmp_dir = output
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(".cascii_export_tmp");
        fs::create_dir_all(&tmp_dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

        let mut segment_paths: Vec<PathBuf> = Vec::new();

        for (index, clip) in clips.iter().enumerate() {
            println!(
                "  📎 clip {}/{}: {:?} / {:?} → {}",
                index + 1,
                clips.len(),
                clip.media_type,
                clip.resource_kind,
                clip.actual_resource_id
            );
            emit_progress(
                "processing",
                &format!("Processing clip {}/{}...", index + 1, clips.len()),
                index,
                clips.len(),
            );

            let (media_path, is_frames, fps) = resolve_clip_media(clip)?;

            if is_frames {
                let intermediate = tmp_dir.join(format!("segment_{:04}.mp4", index));
                let use_colors = matches!(
                    clip.frame_render_mode,
                    Some(database::FrameRenderMode::ColorFrames)
                );
                render_frames_to_intermediate_mp4(
                    &media_path,
                    fps,
                    &intermediate,
                    &ffmpeg_config,
                    use_colors,
                )?;
                segment_paths.push(intermediate);
            } else {
                let intermediate = tmp_dir.join(format!("segment_{:04}.mp4", index));
                println!(
                    "  🎥 re-encoding video {} → {}",
                    media_path,
                    intermediate.display()
                );
                let status = Command::new("ffmpeg")
                    .arg("-y")
                    .arg("-i")
                    .arg(&media_path)
                    .arg("-c:v")
                    .arg("libx264")
                    .arg("-crf")
                    .arg("18")
                    .arg("-preset")
                    .arg("medium")
                    .arg("-c:a")
                    .arg("aac")
                    .arg("-movflags")
                    .arg("+faststart")
                    .arg(&intermediate)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .status()
                    .map_err(|e| format!("ffmpeg re-encode failed: {e}"))?;
                if !status.success() {
                    return Err(format!("failed to re-encode video clip: {}", media_path));
                }
                println!("  ✅ re-encoded {}", intermediate.display());
                segment_paths.push(intermediate);
            }
        }

        emit_progress(
            "concatenating",
            "Concatenating segments...",
            clips.len(),
            clips.len(),
        );

        if segment_paths.len() == 1 {
            fs::copy(&segment_paths[0], &output)
                .map_err(|e| format!("failed to copy single segment: {e}"))?;
        } else {
            concat_videos_ffmpeg(&segment_paths, &output)?;
        }

        let _ = fs::remove_dir_all(&tmp_dir);

        let file_size = fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        println!(
            "✅ export complete: {} ({} bytes)",
            output.display(),
            file_size
        );
        emit_progress(
            "complete",
            &format!("Export complete ({} bytes)", file_size),
            clips.len(),
            clips.len(),
        );

        Ok(output_path)
    })
    .await
    .map_err(|e| format!("export task failed: {e}"))?
}
