use crate::database;
use crate::ffmpeg::get_ffmpeg_config;
use crate::settings;
use cascii::{AsciiConverter, ToVideoOptions};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
struct ExportProgress {
    stage: String,
    message: String,
    clip_index: usize,
    total_clips: usize,
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
        (database::TimelineMediaType::Frames | database::TimelineMediaType::Frame, database::TimelineResourceKind::AsciiConversion) => {
            let conv = database::get_conversion(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query conversion: {e}"))?
                .ok_or_else(|| format!("conversion not found: {}", clip.actual_resource_id))?;
            let fps = if conv.settings.frame_speed > 0 { conv.settings.frame_speed } else { conv.settings.fps };
            Ok((conv.folder_path, true, fps))
        }
        (database::TimelineMediaType::Frames | database::TimelineMediaType::Frame, database::TimelineResourceKind::Preview) => {
            let preview = database::get_preview(&clip.actual_resource_id)
                .map_err(|e| format!("failed to query preview: {e}"))?
                .ok_or_else(|| format!("preview not found: {}", clip.actual_resource_id))?;
            Ok((preview.folder_path, true, preview.settings.fps))
        }
        _ => Err(format!("unsupported clip type: {:?}/{:?}", clip.media_type, clip.resource_kind)),
    }
}

fn render_frames_to_intermediate_mp4(frames_dir: &str, fps: u32, output_path: &PathBuf, ffmpeg_config: &cascii::FfmpegConfig, use_colors: bool) -> Result<(), String> {
    println!("  🎞️  rendering frames from {} at {}fps → {} (colors={})", frames_dir, fps, output_path.display(), use_colors);

    let converter = AsciiConverter::new().with_ffmpeg_config(ffmpeg_config.clone());
    let to_video_opts = ToVideoOptions {
        output_path: output_path.clone(),
        font_size: 14.0,
        crf: 18,
        mux_audio: false,
        use_colors: Some(use_colors),
    };

    converter.render_frames_to_video(std::path::Path::new(frames_dir), fps, &to_video_opts, |progress| {
        println!("    render progress: {:?}", progress);
    }).map_err(|e| format!("failed to render frames to video: {e}"))?;

    println!("  ✅ rendered {}", output_path.display());
    Ok(())
}

fn concat_videos_ffmpeg(segments: &[PathBuf], output_path: &PathBuf) -> Result<(), String> {
    println!("🔗 concatenating {} segments → {}", segments.len(), output_path.display());

    let tmp_dir = output_path.parent().unwrap_or(std::path::Path::new("."));
    let concat_list_path = tmp_dir.join(".export_concat_list.txt");

    let list_content: String = segments.iter().map(|p| format!("file '{}'\n", p.display())).collect();
    fs::write(&concat_list_path, &list_content).map_err(|e| format!("failed to write concat list: {e}"))?;

    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-f").arg("concat")
        .arg("-safe").arg("0")
        .arg("-i").arg(&concat_list_path)
        .arg("-c").arg("copy")
        .arg("-movflags").arg("+faststart")
        .arg(output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("failed to run ffmpeg concat: {e}"))?;

    let _ = fs::remove_file(&concat_list_path);

    if !status.success() {
        // If copy fails (mismatched codecs), try re-encoding
        println!("  ⚠️ concat copy failed, re-encoding...");
        let status = Command::new("ffmpeg")
            .arg("-y")
            .arg("-f").arg("concat")
            .arg("-safe").arg("0")
            .arg("-i").arg(&{
                let p = tmp_dir.join(".export_concat_list.txt");
                fs::write(&p, &list_content).map_err(|e| format!("failed to write concat list: {e}"))?;
                p
            })
            .arg("-c:v").arg("libx264")
            .arg("-crf").arg("18")
            .arg("-preset").arg("medium")
            .arg("-c:a").arg("aac")
            .arg("-movflags").arg("+faststart")
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
pub async fn export_timeline_mp4(project_id: String, output_path: String, app: tauri::AppHandle) -> Result<String, String> {
    println!("🎬 export_timeline_mp4: project={}, output={}", project_id, output_path);

    tokio::task::spawn_blocking(move || {
        let emit_progress = |stage: &str, message: &str, clip_index: usize, total_clips: usize| {
            let _ = app.emit("export-progress", ExportProgress {
                stage: stage.to_string(),
                message: message.to_string(),
                clip_index,
                total_clips,
            });
        };

        let timeline = database::get_active_project_timeline(&project_id).map_err(|e| format!("failed to get timeline: {e}"))?;
        let clips = timeline.clips;

        if clips.is_empty() {
            return Err("timeline is empty — nothing to export".to_string());
        }

        println!("  📋 timeline has {} clips", clips.len());
        emit_progress("resolving", "Resolving timeline clips...", 0, clips.len());

        let settings = settings::load();
        let ffmpeg_config = get_ffmpeg_config(&app, &settings.ffmpeg_source);

        let output = PathBuf::from(&output_path);
        let tmp_dir = output.parent().unwrap_or(std::path::Path::new(".")).join(".cascii_export_tmp");
        fs::create_dir_all(&tmp_dir).map_err(|e| format!("failed to create temp dir: {e}"))?;

        let mut segment_paths: Vec<PathBuf> = Vec::new();

        for (i, clip) in clips.iter().enumerate() {
            println!("  📎 clip {}/{}: {:?} / {:?} → {}", i + 1, clips.len(), clip.media_type, clip.resource_kind, clip.actual_resource_id);
            emit_progress("processing", &format!("Processing clip {}/{}...", i + 1, clips.len()), i, clips.len());

            let (media_path, is_frames, fps) = resolve_clip_media(clip)?;

            if is_frames {
                let intermediate = tmp_dir.join(format!("segment_{:04}.mp4", i));
                let use_colors = matches!(clip.frame_render_mode, Some(database::FrameRenderMode::ColorFrames));
                render_frames_to_intermediate_mp4(&media_path, fps, &intermediate, &ffmpeg_config, use_colors)?;
                segment_paths.push(intermediate);
            } else {
                // For video clips, re-encode to a consistent format so concat works
                let intermediate = tmp_dir.join(format!("segment_{:04}.mp4", i));
                println!("  🎥 re-encoding video {} → {}", media_path, intermediate.display());
                let status = Command::new("ffmpeg")
                    .arg("-y")
                    .arg("-i").arg(&media_path)
                    .arg("-c:v").arg("libx264")
                    .arg("-crf").arg("18")
                    .arg("-preset").arg("medium")
                    .arg("-c:a").arg("aac")
                    .arg("-movflags").arg("+faststart")
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

        emit_progress("concatenating", "Concatenating segments...", clips.len(), clips.len());

        if segment_paths.len() == 1 {
            fs::copy(&segment_paths[0], &output).map_err(|e| format!("failed to copy single segment: {e}"))?;
        } else {
            concat_videos_ffmpeg(&segment_paths, &output)?;
        }

        // Cleanup temp directory
        let _ = fs::remove_dir_all(&tmp_dir);

        let file_size = fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        println!("✅ export complete: {} ({} bytes)", output.display(), file_size);
        emit_progress("complete", &format!("Export complete ({} bytes)", file_size), clips.len(), clips.len());

        Ok(output_path)
    }).await.map_err(|e| format!("export task failed: {e}"))?
}
