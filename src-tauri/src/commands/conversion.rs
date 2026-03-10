use crate::database;
use crate::ffmpeg::{extract_audio_from_video, get_ffmpeg_config};
use crate::settings;
use crate::util::{
    count_frames_and_size, default_background_color, default_foreground_color,
    generate_random_suffix, inspect_frame_directory, output_mode_from_color_flag,
    resolve_frame_metadata, scan_frames_in_dir, FrameDirectory, FrameFile,
};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;
use uuid::Uuid;

#[tauri::command]
pub fn get_project_conversions(
    project_id: String,
) -> Result<Vec<database::AsciiConversion>, String> {
    database::get_project_conversions(&project_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_conversion_by_folder_path(
    folder_path: String,
) -> Result<Option<database::AsciiConversion>, String> {
    database::get_conversion_by_folder_path(&folder_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_conversion_frame_speed(
    conversion_id: String,
    frame_speed: u32,
) -> Result<(), String> {
    println!(
        "🔄 Tauri: Updating frame_speed for conversion {} to {}",
        conversion_id, frame_speed
    );
    let result = database::update_conversion_frame_speed(&conversion_id, frame_speed);
    match &result {
        Ok(_) => println!("✅ Tauri: Database update successful"),
        Err(e) => println!("❌ Tauri: Database update failed: {}", e),
    }
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project_frames(project_id: String) -> Result<Vec<FrameDirectory>, String> {
    let project = database::get_project(&project_id).map_err(|e| e.to_string())?;
    let settings = settings::load();
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);

    if !project_dir.exists() {
        return Ok(Vec::new());
    }

    let mut frames = Vec::new();

    let scan_directory = |dir: &PathBuf, frames: &mut Vec<FrameDirectory>| -> Result<(), String> {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                        if dir_name.ends_with("_ascii") || dir_name.contains("_ascii[") {
                            let source_name = if let Some(bracket_start) = dir_name.find("_ascii[")
                            {
                                &dir_name[..bracket_start]
                            } else {
                                dir_name.strip_suffix("_ascii").unwrap_or(dir_name)
                            };

                            let folder_path = path.to_str().unwrap_or("").to_string();
                            let conversion = database::get_conversion_by_folder_path(&folder_path)
                                .ok()
                                .flatten();
                            let (frame_files, has_text_frames, has_color_frames) =
                                inspect_frame_directory(&path)?;
                            let metadata = resolve_frame_metadata(
                                &path,
                                conversion
                                    .as_ref()
                                    .map(|conversion| conversion.settings.output_mode.as_str()),
                                conversion
                                    .as_ref()
                                    .map(|conversion| conversion.settings.color)
                                    .unwrap_or(false),
                                conversion.as_ref().and_then(|conversion| {
                                    conversion.settings.foreground_color.as_deref()
                                }),
                                conversion.as_ref().and_then(|conversion| {
                                    conversion.settings.background_color.as_deref()
                                }),
                            );
                            let custom_name = conversion
                                .as_ref()
                                .and_then(|conversion| conversion.custom_name.clone());

                            let display_name = custom_name
                                .clone()
                                .unwrap_or_else(|| format!("{} - Frames", source_name));
                            frames.push(FrameDirectory {
                                conversion_id: conversion
                                    .as_ref()
                                    .map(|conversion| conversion.id.clone())
                                    .unwrap_or_else(|| format!("path:{}", folder_path)),
                                name: display_name,
                                directory_path: folder_path,
                                source_file_name: source_name.to_string(),
                                custom_name,
                                frame_count: conversion
                                    .as_ref()
                                    .map(|conversion| conversion.frame_count)
                                    .unwrap_or(frame_files.len() as i32),
                                fps: conversion
                                    .as_ref()
                                    .map(|conversion| conversion.settings.fps)
                                    .unwrap_or(24),
                                frame_speed: conversion
                                    .as_ref()
                                    .map(|conversion| conversion.settings.frame_speed)
                                    .unwrap_or_else(|| {
                                        conversion
                                            .as_ref()
                                            .map(|conversion| conversion.settings.fps)
                                            .unwrap_or(24)
                                    }),
                                color: conversion
                                    .as_ref()
                                    .map(|conversion| conversion.settings.color)
                                    .unwrap_or(false),
                                output_mode: metadata.output_mode,
                                foreground_color: Some(metadata.foreground_color),
                                background_color: Some(metadata.background_color),
                                has_text_frames,
                                has_color_frames,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    };

    let frames_dir = project_dir.join("frames");
    if frames_dir.exists() {
        scan_directory(&frames_dir, &mut frames)?;
    }

    scan_directory(&project_dir, &mut frames)?;

    let cuts_dir = project_dir.join("cuts");
    if cuts_dir.exists() {
        scan_directory(&cuts_dir, &mut frames)?;
    }

    frames.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(frames)
}

#[tauri::command]
pub fn get_frame_files(directory_path: String) -> Result<Vec<FrameFile>, String> {
    let dir = PathBuf::from(&directory_path);
    scan_frames_in_dir(&dir)
}

#[tauri::command]
pub fn read_frame_file(file_path: String) -> Result<String, String> {
    let path = PathBuf::from(&file_path);
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if extension == "cframe" {
        let bytes = fs::read(&path).map_err(|e| format!("Failed to read cframe file: {}", e))?;
        cascii_core_view::parse_cframe_text(&bytes)
            .map_err(|e| format!("Failed to decode cframe text: {}", e))
    } else {
        fs::read_to_string(&path).map_err(|e| format!("Failed to read frame file: {}", e))
    }
}

#[tauri::command]
pub fn read_cframe_file(txt_file_path: String) -> Result<Option<Vec<u8>>, String> {
    let source_path = PathBuf::from(&txt_file_path);
    let cframe_path = if source_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("cframe"))
        .unwrap_or(false)
    {
        source_path
    } else {
        source_path.with_extension("cframe")
    };

    if !cframe_path.exists() {
        return Ok(None);
    }

    let data = fs::read(&cframe_path).map_err(|e| format!("Failed to read cframe file: {}", e))?;

    Ok(Some(data))
}

#[derive(serde::Deserialize)]
pub(crate) struct UpdateFrameCustomNameRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    #[serde(rename = "customName")]
    custom_name: Option<String>,
}

#[tauri::command]
pub fn update_frame_custom_name(request: UpdateFrameCustomNameRequest) -> Result<(), String> {
    let conversion = database::get_conversion_by_folder_path(&request.folder_path)
        .map_err(|e| format!("Failed to find conversion: {}", e))?
        .ok_or("Conversion not found")?;

    database::update_conversion_custom_name(&conversion.id, request.custom_name)
        .map_err(|e| format!("Failed to update custom name: {}", e))
}

#[tauri::command]
pub fn delete_frame_directory(directory_path: String) -> Result<(), String> {
    let dir_path = PathBuf::from(&directory_path);

    if !dir_path.exists() {
        return Err("Directory does not exist".to_string());
    }

    fs::remove_dir_all(&dir_path)
        .map_err(|e| format!("Failed to delete frame directory: {}", e))?;

    database::delete_conversion_by_folder_path(&directory_path)
        .map_err(|e| format!("Failed to delete conversion from database: {}", e))?;

    Ok(())
}

#[derive(serde::Deserialize, Clone)]
pub(crate) struct ConvertToAsciiRequest {
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
    #[serde(default)]
    preprocess_enabled: bool,
    #[serde(default)]
    preprocess_preset: Option<String>,
    #[serde(default)]
    preprocess_custom: Option<String>,
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

#[tauri::command]
pub async fn convert_to_ascii(
    app: tauri::AppHandle,
    request: ConvertToAsciiRequest,
) -> Result<String, String> {
    use cascii::{AsciiConverter, ConversionOptions, VideoOptions};

    let input_path = PathBuf::from(&request.file_path);
    if !input_path.exists() {
        return Err(format!("File not found: {}", request.file_path));
    }

    let is_image = input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            matches!(
                ext.to_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
        .unwrap_or(false);

    let settings = settings::load();
    let project = database::get_project(&request.project_id)
        .map_err(|e| format!("Failed to get project: {}", e))?;
    let project_dir = PathBuf::from(&settings.output_directory).join(&project.project_path);
    let frames_dir = project_dir.join("frames");

    fs::create_dir_all(&frames_dir)
        .map_err(|e| format!("Failed to create frames directory: {}", e))?;

    let random_suffix = generate_random_suffix();
    let folder_name = format!(
        "{}_ascii{}",
        input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output"),
        random_suffix
    );
    let output_dir = frames_dir.join(&folder_name);
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;

    let input_path_clone = input_path.clone();
    let output_dir_clone = output_dir.clone();
    let request_clone = request.clone();
    let fps = request.fps.unwrap_or(30);
    let source_id_for_progress = request.source_file_id.clone();
    let source_id_for_complete = request.source_file_id.clone();
    let display_name = request.custom_name.clone().unwrap_or_else(|| {
        input_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string()
    });
    let display_name_for_return = display_name.clone();
    let folder_name_clone = folder_name.clone();

    let preprocess_filter = if request.preprocess_enabled {
        let selected = request
            .preprocess_preset
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match selected {
            Some("other") => cascii::preprocessing::resolve_preprocess_filter(
                request.preprocess_custom.as_deref(),
                None,
            )
            .map_err(|e| format!("Invalid preprocessing filter: {}", e))?,
            Some(preset_name) => {
                cascii::preprocessing::resolve_preprocess_filter(None, Some(preset_name))
                    .map_err(|e| format!("Invalid preprocessing preset: {}", e))?
            }
            None => return Err("Preprocessing is enabled but no preset was selected".to_string()),
        }
    } else {
        None
    };

    let app_for_complete = app.clone();
    let project_id_for_db = request.project_id.clone();
    let custom_name_for_db = request.custom_name.clone();
    let luminance_for_db = request.luminance;
    let font_ratio_for_db = request.font_ratio;
    let columns_for_db = request.columns;
    let color_for_db = request.color;
    let extract_audio_flag = request.extract_audio;
    let is_image_for_audio = is_image;

    let input_path_for_audio = input_path.clone();
    let project_dir_for_audio = project_dir.clone();
    let random_suffix_for_audio = random_suffix.clone();
    let source_id_for_audio = request.source_file_id.clone();
    let project_id_for_audio = request.project_id.clone();

    let current_settings = settings::load();
    let ffmpeg_config = get_ffmpeg_config(&app, &current_settings.ffmpeg_source);

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
                let output_file = output_dir_clone.join(format!(
                    "{}.txt",
                    input_path_clone
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output")
                ));
                converter
                    .convert_image(&input_path_clone, &output_file, &conv_opts)
                    .map_err(|e| format!("Failed to convert image: {}", e))?;
                Ok(output_dir_clone)
            } else {
                let video_opts = VideoOptions {
                    fps,
                    start: None,
                    end: None,
                    columns: request_clone.columns,
                    extract_audio: false,
                    preprocess_filter: preprocess_filter.clone(),
                };

                println!("🎬 Starting video conversion: {}", source_id_for_progress);
                let app_clone = app.clone();
                let source_id_owned = source_id_for_progress.clone();

                use std::sync::atomic::{AtomicU8, Ordering};
                let last_reported_percent = std::sync::Arc::new(AtomicU8::new(0));
                let last_percent_clone = std::sync::Arc::clone(&last_reported_percent);

                converter
                    .convert_video_with_progress(
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

                            if percentage > last || completed == total {
                                last_percent_clone.store(percentage, Ordering::Relaxed);
                                let _ = app_clone.emit(
                                    "conversion-progress",
                                    ConversionProgress {
                                        source_id: source_id_owned.clone(),
                                        percentage,
                                    },
                                );
                            }
                        }),
                    )
                    .map_err(|e| format!("Failed to convert video: {}", e))?;

                Ok(output_dir_clone)
            }
        })
        .await;

        match conversion_result {
            Ok(Ok(result_path)) => match count_frames_and_size(&result_path) {
                Ok((frame_count, total_size)) => {
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
                            output_mode: output_mode_from_color_flag(color_for_db),
                            foreground_color: Some(default_foreground_color()),
                            background_color: Some(default_background_color()),
                        },
                        creation_date: Utc::now(),
                        total_size,
                        custom_name: custom_name_for_db.clone(),
                    };

                    match database::add_ascii_conversion(&conversion) {
                        Ok(_) => {
                            println!("✅ Conversion complete: {}", source_id_for_complete);

                            let mut audio_message = String::new();
                            println!(
                                "🔊 Audio extraction check: flag={}, is_image={}",
                                extract_audio_flag, is_image_for_audio
                            );
                            if extract_audio_flag && !is_image_for_audio {
                                println!("🎵 Starting audio extraction...");
                                let audio_dir = project_dir_for_audio.join("audio");
                                match extract_audio_from_video(
                                    &input_path_for_audio,
                                    &audio_dir,
                                    &random_suffix_for_audio,
                                ) {
                                    Ok((audio_folder_path, audio_size, duration)) => {
                                        let audio_folder_name = audio_folder_path
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("audio")
                                            .to_string();

                                        let audio_extraction = database::AudioExtraction {
                                            id: Uuid::new_v4().to_string(),
                                            folder_name: audio_folder_name,
                                            folder_path: audio_folder_path
                                                .to_str()
                                                .unwrap_or("")
                                                .to_string(),
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
                                                audio_message = format!(
                                                    " + Audio extracted ({} bytes)",
                                                    audio_size
                                                );
                                                println!("✅ Audio extraction saved to database");
                                            }
                                            Err(e) => {
                                                println!(
                                                    "❌ Failed to save audio to database: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("❌ Failed to extract audio: {}", e);
                                    }
                                }
                            }

                            let _ = app_for_complete.emit(
                                "conversion-complete",
                                ConversionComplete {
                                    source_id: source_id_for_complete,
                                    success: true,
                                    message: format!(
                                        "ASCII frames saved to: {} ({} frames, {} bytes){}",
                                        result_path.display(),
                                        frame_count,
                                        total_size,
                                        audio_message
                                    ),
                                },
                            );
                        }
                        Err(e) => {
                            println!("❌ Failed to save to database: {}", e);
                            let _ = app_for_complete.emit(
                                "conversion-complete",
                                ConversionComplete {
                                    source_id: source_id_for_complete,
                                    success: false,
                                    message: format!(
                                        "Failed to save conversion to database: {}",
                                        e
                                    ),
                                },
                            );
                        }
                    }
                }
                Err(e) => {
                    let _ = app_for_complete.emit(
                        "conversion-complete",
                        ConversionComplete {
                            source_id: source_id_for_complete,
                            success: false,
                            message: e,
                        },
                    );
                }
            },
            Ok(Err(e)) => {
                let _ = app_for_complete.emit(
                    "conversion-complete",
                    ConversionComplete {
                        source_id: source_id_for_complete,
                        success: false,
                        message: e,
                    },
                );
            }
            Err(e) => {
                let _ = app_for_complete.emit(
                    "conversion-complete",
                    ConversionComplete {
                        source_id: source_id_for_complete,
                        success: false,
                        message: format!("Task failed: {}", e),
                    },
                );
            }
        }
    });

    Ok(format!(
        "Conversion started for: {}",
        display_name_for_return
    ))
}

#[derive(serde::Deserialize)]
pub(crate) struct CutFramesRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    #[serde(rename = "startIndex")]
    start_index: usize,
    #[serde(rename = "endIndex")]
    end_index: usize,
}

#[tauri::command]
pub async fn cut_frames(
    request: CutFramesRequest,
    app: tauri::AppHandle,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || cut_frames_blocking(request, app))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn cut_frames_blocking(
    request: CutFramesRequest,
    _app: tauri::AppHandle,
) -> Result<String, String> {
    let conversion = database::get_conversion_by_folder_path(&request.folder_path)
        .map_err(|e| e.to_string())?
        .ok_or("Original conversion not found")?;

    let original_dir = PathBuf::from(&conversion.folder_path);
    if !original_dir.exists() {
        return Err("Original directory not found".to_string());
    }

    let parent_dir = original_dir.parent().ok_or("Invalid directory structure")?;
    let random_suffix = generate_random_suffix();

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

    let frame_files = scan_frames_in_dir(&original_dir)?;

    let frames_to_copy: Vec<_> = frame_files
        .iter()
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
        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");

        let new_filename = format!("frame_{:04}.{}", new_idx + 1, ext);
        let dest_path = new_folder_path.join(new_filename);

        fs::copy(&src_path, &dest_path)
            .map_err(|e| format!("Failed to copy {}: {}", frame.name, e))?;

        let size = fs::metadata(&dest_path).map(|m| m.len()).unwrap_or(0);
        total_size += size as i64;
        copied_count += 1;
    }

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
        settings: conversion.settings,
        creation_date: Utc::now(),
        total_size,
        custom_name,
    };

    database::add_ascii_conversion(&new_conversion).map_err(|e| e.to_string())?;

    Ok(format!(
        "Cut frames saved to: {} ({} frames)",
        new_folder_path.display(),
        copied_count
    ))
}

#[derive(serde::Deserialize)]
pub(crate) struct CropFramesRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
}

#[tauri::command]
pub async fn crop_frames(request: CropFramesRequest) -> Result<String, String> {
    tokio::task::spawn_blocking(move || crop_frames_blocking(request))
        .await
        .map_err(|e| format!("Task failed: {}", e))?
}

fn crop_frames_blocking(request: CropFramesRequest) -> Result<String, String> {
    let current_settings = settings::load();
    let conversion = database::get_conversion_by_folder_path(&request.folder_path)
        .map_err(|e| e.to_string())?
        .ok_or("Original conversion not found")?;

    let original_dir = PathBuf::from(&conversion.folder_path);
    if !original_dir.exists() {
        return Err("Original directory not found".to_string());
    }

    if current_settings.crop_output == settings::CropOutput::CurrentFrames {
        let temp_dir = original_dir
            .parent()
            .ok_or("Invalid directory structure")?
            .join(format!(".crop_tmp_{}", Uuid::new_v4()));

        let result = cascii::crop_frames(
            &original_dir,
            request.top,
            request.bottom,
            request.left,
            request.right,
            &temp_dir,
        )
        .map_err(|e| {
            let _ = fs::remove_dir_all(&temp_dir);
            e.to_string()
        })?;

        for entry in fs::read_dir(&original_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("frame_")
                    && (name.ends_with(".txt") || name.ends_with(".cframe"))
                {
                    let _ = fs::remove_file(&path);
                }
            }
        }

        for entry in fs::read_dir(&temp_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let dest = original_dir.join(entry.file_name());
            fs::rename(entry.path(), &dest).map_err(|e| e.to_string())?;
        }

        let _ = fs::remove_dir_all(&temp_dir);

        database::update_conversion_dimensions(
            &conversion.id,
            result.frame_count as i32,
            result.total_size as i64,
        )
        .map_err(|e| e.to_string())?;

        Ok(format!(
            "Cropped frames in-place: {} ({} frames, {}x{})",
            original_dir.display(),
            result.frame_count,
            result.new_width,
            result.new_height
        ))
    } else {
        let parent_dir = original_dir.parent().ok_or("Invalid directory structure")?;
        let random_suffix = generate_random_suffix();

        let base_name = if let Some(idx) = conversion.folder_name.find("_ascii") {
            &conversion.folder_name[..idx]
        } else if let Some(idx) = conversion.folder_name.find("_cut") {
            &conversion.folder_name[..idx]
        } else {
            &conversion.folder_name
        };

        let new_folder_name = format!("{}_ascii{}", base_name, random_suffix);
        let new_folder_path = parent_dir.join(&new_folder_name);

        let result = cascii::crop_frames(
            &original_dir,
            request.top,
            request.bottom,
            request.left,
            request.right,
            &new_folder_path,
        )
        .map_err(|e| e.to_string())?;

        let custom_name = if let Some(name) = &conversion.custom_name {
            Some(format!("Cropped {}", name))
        } else {
            Some("Cropped frames".to_string())
        };

        let new_conversion = database::AsciiConversion {
            id: Uuid::new_v4().to_string(),
            folder_name: new_folder_name,
            folder_path: new_folder_path.to_str().unwrap_or("").to_string(),
            frame_count: result.frame_count as i32,
            source_file_id: conversion.source_file_id,
            project_id: conversion.project_id,
            settings: conversion.settings,
            creation_date: Utc::now(),
            total_size: result.total_size as i64,
            custom_name,
        };

        database::add_ascii_conversion(&new_conversion).map_err(|e| e.to_string())?;

        Ok(format!(
            "Cropped frames saved to: {} ({} frames, {}x{})",
            new_folder_path.display(),
            result.frame_count,
            result.new_width,
            result.new_height
        ))
    }
}
