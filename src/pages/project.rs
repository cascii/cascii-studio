use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;

use super::open::Project;
use crate::components::video_player::VideoPlayer;
use crate::components::ascii_frames_viewer::{AsciiFramesViewer, ConversionSettings};
use crate::components::settings::{SourceFiles, AvailableFrames, AvailableCuts, Controls};
use crate::components::settings::available_cuts::VideoCut;

// Wasm bindings to Tauri API
#[wasm_bindgen(inline_js = r#"
export async function tauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);   // v2
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args); // v1
  throw new Error('Tauri invoke is not available on this page');
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;
}

// Wasm bindings to Tauri event API (for file conversion progress)
#[wasm_bindgen(inline_js = r#"
export async function tauriListen(event, handler) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, handler);
  throw new Error('Tauri listen is not available');
}

export async function tauriUnlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriListen)]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> JsValue;
    #[wasm_bindgen(js_name = tauriUnlisten)]
    async fn tauri_unlisten(unlisten_fn: JsValue);
}

#[derive(Serialize, Deserialize)]
struct AddSourceFilesRequest {
    project_id: String,
    file_paths: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct AddSourceFilesArgs {
    request: AddSourceFilesRequest,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct FileProgress {
    file_name: String,
    status: String,
    message: String,
    percentage: Option<f32>,
}

// Wasm binding to our custom JS shim for convertFileSrc
#[wasm_bindgen(inline_js = r#"
export function appConvertFileSrc(path) {
  if (window.__APP__convertFileSrc) {
    return window.__APP__convertFileSrc(path);
  }
  console.error('__APP__convertFileSrc not found');
  return path;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = appConvertFileSrc)]
    fn app_convert_file_src(path: &str) -> String;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PreparedMedia {
    pub cached_abs_path: String,
    pub media_kind: String,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceContent {
    pub id: String,
    pub content_type: String, // "Image" or "Video"
    pub project_id: String,
    pub date_added: DateTime<Utc>,
    pub size: i64,
    pub file_path: String,
    #[serde(default)]
    pub custom_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrameDirectory {
    pub name: String,
    pub directory_path: String,
    pub source_file_name: String,
}

#[derive(Properties, PartialEq)]
pub struct ProjectPageProps {
    pub project_id: String,
}

#[function_component(ProjectPage)]
pub fn project_page(props: &ProjectPageProps) -> Html {
    let project_id = use_state(|| props.project_id.clone());
    let project = use_state(|| None::<Project>);
    let source_files = use_state(|| Vec::<SourceContent>::new());
    let frame_directories = use_state(|| Vec::<FrameDirectory>::new());
    let selected_source = use_state(|| None::<SourceContent>);
    let selected_frame_dir = use_state(|| None::<FrameDirectory>);
    let selected_frame_settings = use_state(|| None::<ConversionSettings>);
    let asset_url = use_state(|| None::<String>);
    let error_message = use_state(|| Option::<String>::None);
    let is_loading_media = use_state(|| false);
    let url_cache = use_state(|| HashMap::<String, String>::new());    // URL cache to avoid recomputing asset URLs
    let is_adding_files = use_state(|| false);
    let file_progress_map = use_state(|| HashMap::<String, FileProgress>::new());
    
    // ASCII conversion settings
    let luminance = use_state(|| 1u8);
    let font_ratio = use_state(|| 0.7f32);
    let columns = use_state(|| 200u32);
    let fps = use_state(|| 30u32);
    let is_converting = use_state(|| false);
    let conversion_message = use_state(|| Option::<String>::None);
    let is_playing = use_state(|| false);
    let should_reset = use_state(|| false);
    let synced_progress = use_state(|| 0.0f64); // 0-100 percentage
    let seek_percentage = use_state(|| None::<f64>);
    let frames_loading = use_state(|| false);
    let frame_speed = use_state(|| None::<u32>);
    let current_conversion_id = use_state(|| None::<String>);
    let selected_speed = use_state(|| crate::components::ascii_frames_viewer::SpeedSelection::Custom);

    // Collapsible section states
    let source_files_collapsed = use_state(|| false);
    let frames_collapsed = use_state(|| false);
    let cuts_collapsed = use_state(|| false);
    let controls_collapsed = use_state(|| false);

    // Video cuts state
    let video_cuts = use_state(|| Vec::<VideoCut>::new());
    let selected_cut = use_state(|| None::<VideoCut>);
    let is_cutting = use_state(|| false);

    {
        let project_id = project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let error_message = error_message.clone();

        use_effect_with((*project_id).clone(), move |id| {
            let id = id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch project details
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project", args).await {
                    result => {
                        if let Ok(p) = serde_wasm_bindgen::from_value(result) {
                            project.set(Some(p));
                        } else {
                            error_message.set(Some("Failed to fetch project details.".to_string()));
                        }
                    }
                }

                // Fetch source files
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_sources", args).await {
                    result => {
                        if let Ok(s) = serde_wasm_bindgen::from_value(result) {
                            source_files.set(s);
                        } else {
                            error_message.set(Some("Failed to fetch source files.".to_string()));
                        }
                    }
                }

                // Fetch frame directories
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_frames", args).await {
                    result => {
                        if let Ok(frames) = serde_wasm_bindgen::from_value(result) {
                            frame_directories.set(frames);
                        } else {
                            // Not critical, just log silently
                        }
                    }
                }

                // Fetch video cuts
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_cuts", args).await {
                    result => {
                        if let Ok(cuts) = serde_wasm_bindgen::from_value(result) {
                            video_cuts.set(cuts);
                        }
                        // Not critical, just log silently
                    }
                }
            });

            || ()
        });
    }

    // When a source is selected, prepare the media and convert to asset:// URL
    let on_select_source = {
        let selected_source = selected_source.clone();
        let asset_url = asset_url.clone();
        let error_message = error_message.clone();
        let is_loading_media = is_loading_media.clone();
        let url_cache = url_cache.clone();

        Callback::from(move |source: SourceContent| {
            let file_path = source.file_path.clone();
            
            // Check cache first
            if let Some(cached_url) = url_cache.get(&file_path) {
                selected_source.set(Some(source));
                asset_url.set(Some(cached_url.clone()));
                return;
            }
            
            // Not in cache, prepare media
            let selected_source = selected_source.clone();
            let asset_url = asset_url.clone();
            let error_message = error_message.clone();
            let is_loading_media = is_loading_media.clone();
            let url_cache = url_cache.clone();
            let source_clone = source.clone();
            
            is_loading_media.set(true);
            
            wasm_bindgen_futures::spawn_local(async move {
                // Call prepare_media to get cached path
                let args = serde_wasm_bindgen::to_value(&json!({ "path": file_path })).unwrap();
                match tauri_invoke("prepare_media", args).await {
                    result => {
                        if let Ok(prepared) = serde_wasm_bindgen::from_value::<PreparedMedia>(result) {
                            // Convert cached path to asset:// URL
                            let asset_url_str = app_convert_file_src(&prepared.cached_abs_path);
                            
                            // Store in cache
                            let mut cache = (*url_cache).clone();
                            cache.insert(file_path, asset_url_str.clone());
                            url_cache.set(cache);
                            
                            // Update state
                            selected_source.set(Some(source_clone));
                            asset_url.set(Some(asset_url_str));
                        } else {
                            error_message.set(Some("Failed to prepare media file.".to_string()));
                        }
                        is_loading_media.set(false);
                    }
                }
            });
        })
    };

    // Callback to cut video
    let on_cut_video = {
        let project_id = project_id.clone();
        let selected_source = selected_source.clone();
        let video_cuts = video_cuts.clone();
        let is_cutting = is_cutting.clone();
        let error_message = error_message.clone();

        Callback::from(move |(start_time, end_time): (f64, f64)| {
            if let Some(source) = &*selected_source {
                let project_id = (*project_id).clone();
                let source_file_id = source.id.clone();
                let source_file_path = source.file_path.clone();
                let video_cuts = video_cuts.clone();
                let is_cutting = is_cutting.clone();
                let error_message = error_message.clone();

                is_cutting.set(true);

                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "args": {
                            "request": {
                                "source_file_path": source_file_path,
                                "project_id": project_id,
                                "source_file_id": source_file_id,
                                "start_time": start_time,
                                "end_time": end_time
                            }
                        }
                    })).unwrap();

                    match tauri_invoke("cut_video", args).await {
                        result => {
                            is_cutting.set(false);
                            if serde_wasm_bindgen::from_value::<VideoCut>(result.clone()).is_ok() {
                                // Refresh cuts list
                                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                if let Ok(cuts) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await) {
                                    video_cuts.set(cuts);
                                }
                            } else {
                                error_message.set(Some("Failed to cut video".to_string()));
                            }
                        }
                    }
                });
            }
        })
    };

    // Callback to delete a cut
    let on_delete_cut = {
        let video_cuts = video_cuts.clone();
        let project_id = project_id.clone();
        let selected_cut = selected_cut.clone();

        Callback::from(move |cut: VideoCut| {
            let video_cuts = video_cuts.clone();
            let project_id = (*project_id).clone();
            let cut_id = cut.id.clone();
            let file_path = cut.file_path.clone();
            let selected_cut = selected_cut.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "cutId": cut_id,
                        "filePath": file_path
                    }
                })).unwrap();
                let _ = tauri_invoke("delete_cut", args).await;

                // Clear selection if deleted cut was selected
                if selected_cut.as_ref().map(|s| s.id == cut_id).unwrap_or(false) {
                    selected_cut.set(None);
                }

                // Refresh
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await) {
                    video_cuts.set(cuts);
                }
            });
        })
    };

    // Callback to refresh cuts after rename
    let on_rename_cut = {
        let video_cuts = video_cuts.clone();
        let project_id = project_id.clone();

        Callback::from(move |(_cut_id, _new_name): (String, String)| {
            let video_cuts = video_cuts.clone();
            let project_id = (*project_id).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await) {
                    video_cuts.set(cuts);
                }
            });
        })
    };

    html! {
        <div class="container project-page">
            <div class="project-layout">
                <div class="left-sidebar">
                    <SourceFiles
                        source_files={(*source_files).clone()}
                        selected_source={(*selected_source).clone()}
                        source_files_collapsed={*source_files_collapsed}
                        on_toggle_collapsed={{
                            let source_files_collapsed = source_files_collapsed.clone();
                            Callback::from(move |_| {
                                source_files_collapsed.set(!*source_files_collapsed);
                            })
                        }}
                        on_select_source={on_select_source.clone()}
                        on_add_files={{
                            let project_id = project_id.clone();
                            let source_files = source_files.clone();
                            let error_message = error_message.clone();
                            let is_adding_files = is_adding_files.clone();
                            let file_progress_map = file_progress_map.clone();
                            Some(Callback::from(move |_| {
                                web_sys::console::log_1(&"🎯 Add files button clicked!".into());
                                let project_id = project_id.clone();
                                let source_files = source_files.clone();
                                let error_message = error_message.clone();
                                let is_adding_files = is_adding_files.clone();
                                let file_progress_map = file_progress_map.clone();

                                wasm_bindgen_futures::spawn_local(async move {
                                    web_sys::console::log_1(&"🚀 Starting file picking process...".into());
                                    error_message.set(None);
                                    // Pick files
                                    match tauri_invoke("pick_files", JsValue::NULL).await {
                                        result => {
                                            web_sys::console::log_1(&format!("📤 Pick files result received").into());
                                            let result_clone = result.clone();
                                            match serde_wasm_bindgen::from_value::<Vec<String>>(result) {
                                                Ok(file_paths) => {
                                                    web_sys::console::log_1(&format!("✅ Parsed {} file paths", file_paths.len()).into());
                                                    if !file_paths.is_empty() {
                                                        web_sys::console::log_1(&format!("📁 Files selected: {:?}", file_paths).into());

                                                        // Initialize progress UI for selected files
                                                        let mut initial_map = HashMap::new();
                                                        for file_path in file_paths.iter() {
                                                            let file_name = std::path::Path::new(file_path)
                                                                .file_name()
                                                                .and_then(|n| n.to_str())
                                                                .unwrap_or("unknown")
                                                                .to_string();
                                                            initial_map.insert(file_name.clone(), FileProgress {
                                                                file_name,
                                                                status: "pending".to_string(),
                                                                message: "Waiting...".to_string(),
                                                                percentage: None,
                                                            });
                                                        }
                                                        file_progress_map.set(initial_map);
                                                        is_adding_files.set(true);

                                                        // Listen for backend progress events (shared with create_project)
                                                        let file_progress_map_clone = file_progress_map.clone();
                                                        let callback: Closure<dyn Fn(JsValue)> = Closure::new(move |event: JsValue| {
                                                            if let Ok(payload) = js_sys::Reflect::get(&event, &"payload".into()) {
                                                                if let Ok(progress) = serde_wasm_bindgen::from_value::<FileProgress>(payload) {
                                                                    let mut map = (*file_progress_map_clone).clone();
                                                                    map.insert(progress.file_name.clone(), progress);
                                                                    file_progress_map_clone.set(map);
                                                                }
                                                            }
                                                        });
                                                        let unlisten_handle = tauri_listen("file-progress", callback.as_ref().unchecked_ref()).await;

                                                        // Files picked successfully, add them to the project
                                                        if !project_id.is_empty() {
                                                            web_sys::console::log_1(&format!("💾 Adding files to project: {}", project_id.as_str()).into());
                                                            let invoke_args = AddSourceFilesArgs {
                                                                request: AddSourceFilesRequest {
                                                                    project_id: (*project_id).to_string(),
                                                                    file_paths: file_paths,
                                                                }
                                                            };
                                                            // Backend command signature is: add_source_files(args: AddSourceFilesArgs, ...)
                                                            // So invoke payload must be { args: { request: ... } }
                                                            let add_files_args = serde_wasm_bindgen::to_value(&json!({ "args": invoke_args })).unwrap();

                                                            web_sys::console::log_1(&"📡 Calling add_source_files command...".into());
                                                            let add_result = tauri_invoke("add_source_files", add_files_args).await;
                                                            web_sys::console::log_1(&format!("📡 add_source_files result: {:?}", add_result).into());

                                                            // Clean up listener
                                                            tauri_unlisten(unlisten_handle).await;
                                                            drop(callback);
                                                            is_adding_files.set(false);

                                                            // For now, assume success and refresh the source files list
                                                            web_sys::console::log_1(&"🔄 Refreshing source files list...".into());
                                                            let args = serde_wasm_bindgen::to_value(&json!({ "projectId": *project_id })).unwrap();
                                                            match tauri_invoke("get_project_sources", args).await {
                                                                result => {
                                                                    match serde_wasm_bindgen::from_value::<Vec<crate::pages::project::SourceContent>>(result) {
                                                                        Ok(s) => {
                                                                            web_sys::console::log_1(&format!("✅ Successfully refreshed {} source files", s.len()).into());
                                                                            source_files.set(s);
                                                                        }
                                                                        Err(e) => {
                                                                            web_sys::console::log_1(&format!("❌ Failed to parse source files: {:?}", e).into());
                                                                            error_message.set(Some("Failed to refresh source files.".to_string()));
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        } else {
                                                            web_sys::console::log_1(&"⚠️ No project ID available".into());
                                                            tauri_unlisten(unlisten_handle).await;
                                                            drop(callback);
                                                            is_adding_files.set(false);
                                                        }
                                                    } else {
                                                        web_sys::console::log_1(&"ℹ️ No files selected (user cancelled)".into());
                                                        // Don't show error for cancelled dialog
                                                    }
                                                }
                                                Err(e) => {
                                                    web_sys::console::log_1(&format!("❌ Failed to parse pick_files result: {:?}", e).into());
                                                    web_sys::console::log_1(&format!("❌ Raw result: {:?}", result_clone).into());
                                                    error_message.set(Some("Failed to pick files.".to_string()));
                                                }
                                            }
                                        }
                                    }
                                });
                            }))
                        }}
                        on_delete_file={None::<Callback<SourceContent>>}
                        on_rename_file={{
                            let source_files = source_files.clone();
                            let project_id = project_id.clone();
                            Some(Callback::from(move |_source: SourceContent| {
                                // Refresh source files list after rename
                                let source_files = source_files.clone();
                                let project_id = (*project_id).clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                    match tauri_invoke("get_project_sources", args).await {
                                        result => {
                                            if let Ok(s) = serde_wasm_bindgen::from_value(result) {
                                                source_files.set(s);
                                            }
                                        }
                                    }
                                });
                            }))
                        }}
                    />

                    <AvailableFrames frame_directories={(*frame_directories).clone()} selected_frame_dir={(*selected_frame_dir).clone()} selected_frame_settings={(*selected_frame_settings).clone()} frames_collapsed={*frames_collapsed} on_toggle_collapsed={{
                            let frames_collapsed = frames_collapsed.clone();
                            Callback::from(move |_| {
                                frames_collapsed.set(!*frames_collapsed);
                            })
                        }}
                        on_select_frame_dir={{
                            let selected_frame_dir = selected_frame_dir.clone();
                            Callback::from(move |frame_dir: FrameDirectory| {
                                selected_frame_dir.set(Some(frame_dir));
                            })
                        }}
                        on_frame_settings_loaded={{
                            let selected_frame_settings = selected_frame_settings.clone();
                            let frame_speed = frame_speed.clone();
                            let current_conversion_id = current_conversion_id.clone();
                            Callback::from(move |data: Option<(ConversionSettings, Option<String>)>| {
                                match data {
                                    Some((settings, conversion_id)) => {
                                        selected_frame_settings.set(Some(settings.clone()));
                                        frame_speed.set(Some(settings.frame_speed));
                                        current_conversion_id.set(conversion_id);
                                    }
                                    None => {
                                        selected_frame_settings.set(None);
                                        frame_speed.set(None);
                                        current_conversion_id.set(None);
                                    }
                                }
                            })
                        }}
                        on_rename_frame={{
                            let frame_directories = frame_directories.clone();
                            let project_id = project_id.clone();
                            Some(Callback::from(move |(_folder_path, _new_name): (String, String)| {
                                // Refresh frame directories list after rename
                                let frame_directories = frame_directories.clone();
                                let project_id = (*project_id).clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                    match tauri_invoke("get_project_frames", args).await {
                                        result => {
                                            if let Ok(frames) = serde_wasm_bindgen::from_value(result) {
                                                frame_directories.set(frames);
                                            }
                                        }
                                    }
                                });
                            }))
                        }}
                    />

                    <AvailableCuts
                        cuts={(*video_cuts).clone()}
                        selected_cut={(*selected_cut).clone()}
                        cuts_collapsed={*cuts_collapsed}
                        on_toggle_collapsed={{
                            let cuts_collapsed = cuts_collapsed.clone();
                            Callback::from(move |_| {
                                cuts_collapsed.set(!*cuts_collapsed);
                            })
                        }}
                        on_select_cut={{
                            let selected_cut = selected_cut.clone();
                            let selected_source = selected_source.clone();
                            let asset_url = asset_url.clone();
                            let is_loading_media = is_loading_media.clone();
                            let url_cache = url_cache.clone();
                            let error_message = error_message.clone();
                            Callback::from(move |cut: VideoCut| {
                                selected_cut.set(Some(cut.clone()));

                                // Convert cut to SourceContent-like structure for the video player
                                let file_path = cut.file_path.clone();

                                // Check cache first
                                if let Some(cached_url) = url_cache.get(&file_path) {
                                    // Create a pseudo SourceContent from the cut
                                    // Use source_file_id (the original source's ID) for DB foreign key compatibility
                                    let source = SourceContent {
                                        id: cut.source_file_id.clone(),
                                        content_type: "Video".to_string(),
                                        project_id: cut.project_id.clone(),
                                        date_added: chrono::Utc::now(),
                                        size: cut.size,
                                        file_path: cut.file_path.clone(),
                                        custom_name: cut.custom_name.clone(),
                                    };
                                    selected_source.set(Some(source));
                                    asset_url.set(Some(cached_url.clone()));
                                    return;
                                }

                                // Not in cache, prepare media
                                let selected_source = selected_source.clone();
                                let asset_url = asset_url.clone();
                                let is_loading_media = is_loading_media.clone();
                                let url_cache = url_cache.clone();
                                let error_message = error_message.clone();
                                let cut_clone = cut.clone();

                                is_loading_media.set(true);

                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "path": file_path })).unwrap();
                                    match tauri_invoke("prepare_media", args).await {
                                        result => {
                                            if let Ok(prepared) = serde_wasm_bindgen::from_value::<PreparedMedia>(result) {
                                                let asset_url_str = app_convert_file_src(&prepared.cached_abs_path);

                                                // Store in cache
                                                let mut cache = (*url_cache).clone();
                                                cache.insert(cut_clone.file_path.clone(), asset_url_str.clone());
                                                url_cache.set(cache);

                                                // Create a pseudo SourceContent from the cut
                                                // Use source_file_id (the original source's ID) for DB foreign key compatibility
                                                let source = SourceContent {
                                                    id: cut_clone.source_file_id.clone(),
                                                    content_type: "Video".to_string(),
                                                    project_id: cut_clone.project_id.clone(),
                                                    date_added: chrono::Utc::now(),
                                                    size: cut_clone.size,
                                                    file_path: cut_clone.file_path.clone(),
                                                    custom_name: cut_clone.custom_name.clone(),
                                                };
                                                selected_source.set(Some(source));
                                                asset_url.set(Some(asset_url_str));
                                            } else {
                                                error_message.set(Some("Failed to prepare cut video.".to_string()));
                                            }
                                            is_loading_media.set(false);
                                        }
                                    }
                                });
                            })
                        }}
                        on_delete_cut={Some(on_delete_cut.clone())}
                        on_rename_cut={Some(on_rename_cut.clone())}
                    />

                    <Controls
                        selected_source={(*selected_source).clone()}
                        selected_frame_dir={(*selected_frame_dir).clone()}
                        controls_collapsed={*controls_collapsed}
                        on_toggle_collapsed={{
                            let controls_collapsed = controls_collapsed.clone();
                            Callback::from(move |_| {
                                controls_collapsed.set(!*controls_collapsed);
                            })
                        }}
                        is_playing={*is_playing}
                        on_is_playing_change={{
                            let is_playing = is_playing.clone();
                            Callback::from(move |val: bool| {
                                is_playing.set(val);
                            })
                        }}
                        should_reset={*should_reset}
                        on_should_reset_change={{
                            let should_reset = should_reset.clone();
                            Callback::from(move |val: bool| {
                                should_reset.set(val);
                            })
                        }}
                        synced_progress={*synced_progress}
                        on_synced_progress_change={{
                            let synced_progress = synced_progress.clone();
                            Callback::from(move |val: f64| {
                                synced_progress.set(val);
                            })
                        }}
                        seek_percentage={*seek_percentage}
                        on_seek_percentage_change={{
                            let seek_percentage = seek_percentage.clone();
                            Callback::from(move |val: Option<f64>| {
                                seek_percentage.set(val);
                            })
                        }}
                        frames_loading={*frames_loading}
                    />
                </div>

                <div class="main-content">
                    <h1>{ project.as_ref().map(|p| p.project_name.clone()).unwrap_or_else(|| "Loading Project...".into()) }</h1>

                    if let Some(error) = &*error_message {
                        <div class="alert alert-error">{error}</div>
                    }

                    if *is_adding_files && !file_progress_map.is_empty() {
                        <div class="progress-container">
                            <h3>{"Adding Files"}</h3>
                            <div class="progress-list">
                                {
                                    file_progress_map.iter().map(|(file_name, progress)| {
                                        let status_class = match progress.status.as_str() {
                                            "completed"     => "status-completed",
                                            "error"         => "status-error",
                                            "processing"    => "status-processing",
                                            _               => "status-pending"
                                        };

                                        let icon = match progress.status.as_str() {
                                            "completed"     => "✓",
                                            "error"         => "✗",
                                            "processing"    => "⟳",
                                            _               => "○"
                                        };

                                        html! {
                                            <div class={classes!("progress-item", status_class)} key={file_name.clone()}>
                                                <div class="progress-icon">{icon}</div>
                                                <div class="progress-info">
                                                    <div class="progress-filename">{file_name}</div>
                                                    <div class="progress-message">{&progress.message}</div>
                                                    if let Some(percentage) = progress.percentage {
                                                        <div class="progress-bar-container">
                                                            <div class="progress-bar" style={format!("width: {}%", percentage)}></div>
                                                        </div>
                                                    }
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Html>()
                                }
                            </div>
                            <p class="progress-note">{"Please wait while files are being processed..."}</p>
                        </div>
                    }

                    <div class="preview-container">
                        <div class="preview-column">
                            <div class="preview-label">{"Source Video"}</div>
                            <div class="square">
                                {
                                    if *is_loading_media {
                                        html! { <div class="loading">{"Loading media..."}</div> }
                                    } else if let (Some(source), Some(url)) = (&*selected_source, &*asset_url) {
                                        if source.content_type == "Image" {
                                            html! {
                                                <img src={url.clone()} alt="Source Image" loading="lazy" decoding="async" style="max-width:100%;max-height:100%;object-fit:contain;border-radius:8px;" />
                                            }
                                        } else if source.content_type == "Video" {
                                            html! { 
                                                <VideoPlayer
                                                src={url.clone()}
                                                class={classes!("source-video")}
                                                should_play={if *is_playing {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                seek_percentage={*seek_percentage}
                                                on_progress={{
                                                    let synced_progress = synced_progress.clone();
                                                    Callback::from(move |progress: f64| {
                                                        synced_progress.set(progress * 100.0);
                                                    })
                                                }}
                                            
                                                project_id={Some((*project_id).clone())}
                                                source_file_id={Some(source.id.clone())}
                                                source_file_path={Some(source.file_path.clone())}
                                            
                                                luminance={*luminance}
                                                font_ratio={*font_ratio}
                                                columns={*columns}
                                                fps={*fps}
                                            
                                                on_luminance_change={Some({
                                                    let luminance = luminance.clone();
                                                    Callback::from(move |v: u8| luminance.set(v))
                                                })}
                                                on_font_ratio_change={Some({
                                                    let font_ratio = font_ratio.clone();
                                                    Callback::from(move |v: f32| font_ratio.set(v))
                                                })}
                                                on_columns_change={Some({
                                                    let columns = columns.clone();
                                                    Callback::from(move |v: u32| columns.set(v))
                                                })}
                                                on_fps_change={Some({
                                                    let fps = fps.clone();
                                                    Callback::from(move |v: u32| fps.set(v))
                                                })}
                                            
                                                is_converting={Some(*is_converting)}
                                                on_is_converting_change={Some({
                                                    let is_converting = is_converting.clone();
                                                    Callback::from(move |v: bool| is_converting.set(v))
                                                })}
                                                conversion_message={(*conversion_message).clone()}
                                                on_conversion_message_change={Some({
                                                    let conversion_message = conversion_message.clone();
                                                    Callback::from(move |v: Option<String>| conversion_message.set(v))
                                                })}
                                                on_error_message_change={Some({
                                                    let error_message = error_message.clone();
                                                    Callback::from(move |v: Option<String>| error_message.set(v))
                                                })}
                                            
                                                on_refresh_frames={Some({
                                                    let frame_directories = frame_directories.clone();
                                                    let project_id = project_id.clone();
                                                    Callback::from(move |_| {
                                                        let frame_directories = frame_directories.clone();
                                                        let project_id = (*project_id).clone();
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                                            if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                                                                frame_directories.set(frames);
                                                            }
                                                        });
                                                    })
                                                })}

                                                custom_name={source.custom_name.clone()}

                                                on_cut_video={Some(on_cut_video.clone())}
                                                is_cutting={Some(*is_cutting)}
                                            />
                                            }
                                        } else {
                                            html! { <span>{"Unsupported file type"}</span> }
                                        }
                                    } else {
                                        html! { <span>{"Select a source file to preview"}</span> }
                                    }
                                }
                            </div>
                        </div>
                        
                        <div class="preview-column">
                            <div class="preview-label">{"Frames"}</div>
                            <div class="square">
                                {
                                    if frame_directories.is_empty() {
                                        html! { <span>{"No frames generated yet"}</span> }
                                    } else if let Some(frame_dir) = &*selected_frame_dir {
                                        html! {
                                            <AsciiFramesViewer
                                                directory_path={frame_dir.directory_path.clone()}
                                                fps={{
                                                    match *selected_speed {
                                                        crate::components::ascii_frames_viewer::SpeedSelection::Custom => {
                                                            (*frame_speed).unwrap_or(selected_frame_settings.as_ref().map(|s| s.fps).unwrap_or(*fps))
                                                        }
                                                        crate::components::ascii_frames_viewer::SpeedSelection::Base => {
                                                            selected_frame_settings.as_ref().map(|s| s.fps).unwrap_or(*fps)
                                                        }
                                                    }
                                                }}
                                                settings={(*selected_frame_settings).clone()}
                                                should_play={if *is_playing && !*frames_loading {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                seek_percentage={*seek_percentage}
                                                on_loading_changed={{
                                                    let frames_loading = frames_loading.clone();
                                                    Callback::from(move |is_loading: bool| {
                                                        frames_loading.set(is_loading);
                                                    })
                                                }}
                                                frame_speed={*frame_speed}
                                                on_frame_speed_change={{
                                                    let frame_speed = frame_speed.clone();
                                                    let current_conversion_id = current_conversion_id.clone();
                                                    let selected_frame_settings = selected_frame_settings.clone();
                                                    Callback::from(move |speed: u32| {
                                                        frame_speed.set(Some(speed));

                                                        // Update the selected_frame_settings to reflect the change
                                                        if let Some(mut settings) = (*selected_frame_settings).clone() {
                                                            settings.frame_speed = speed;
                                                            selected_frame_settings.set(Some(settings));
                                                        }

                                                        // Update the database if we have a conversion_id
                                                        if let Some(conversion_id) = &*current_conversion_id {
                                                            let conversion_id = conversion_id.clone();
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                let args = serde_wasm_bindgen::to_value(&serde_json::json!({"conversionId": conversion_id, "frameSpeed": speed})).unwrap();
                                                                let _ = tauri_invoke("update_conversion_frame_speed", args).await;
                                                            });
                                                        }
                                                    })
                                                }}
                                                selected_speed={(*selected_speed).clone()}
                                                on_speed_selection_change={{
                                                    let selected_speed = selected_speed.clone();
                                                    Callback::from(move |selection: crate::components::ascii_frames_viewer::SpeedSelection| {
                                                        selected_speed.set(selection);
                                                    })
                                                }}
                                            />
                                        }
                                    } else {
                                        html! { <span>{"Select a frame directory to preview"}</span> }
                                    }
                                }
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
