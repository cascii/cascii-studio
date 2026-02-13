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

// Active conversions: source_id -> percentage (u8)
// Names are looked up from source_files when rendering to avoid redundant storage

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Video,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PreparedMedia {
    pub cached_abs_path: String,
    pub media_kind: MediaKind,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ContentType {
    Image,
    Video,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceContent {
    pub id: String,
    pub content_type: ContentType,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PreviewSettings {
    pub luminance: u8,
    pub font_ratio: f32,
    pub columns: u32,
    pub fps: u32,
    pub color: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Preview {
    pub id: String,
    pub folder_name: String,
    pub folder_path: String,
    pub frame_count: i32,
    pub source_file_id: String,
    pub project_id: String,
    pub settings: PreviewSettings,
    pub creation_date: String,
    pub total_size: i64,
    pub custom_name: Option<String>,
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
    // Use Rc<RefCell<>> for conversions to allow mutation from async closures
    let active_conversions_ref = use_mut_ref(|| HashMap::<String, u8>::new());
    let conversions_update_trigger = use_state(|| 0u32); // Trigger re-renders when conversions change
    let conversion_message = use_state(|| Option::<String>::None);
    let conversion_success_folder = use_state(|| Option::<String>::None);
    let is_playing = use_state(|| false);
    let should_reset = use_state(|| false);
    let synced_progress = use_state(|| 0.0f64); // 0-100 percentage
    let seek_percentage = use_state(|| None::<f64>);
    let frames_loading = use_state(|| false);
    let frame_speed = use_state(|| None::<u32>);
    let current_conversion_id = use_state(|| None::<String>);
    let selected_speed = use_state(|| crate::components::ascii_frames_viewer::SpeedSelection::Custom);
    let loop_enabled = use_state(|| true);
    let color_frames_default = use_state(|| true);
    let extract_audio_default = use_state(|| false);

    // Load persisted loop setting once on mount.
    {
        let loop_enabled = loop_enabled.clone();
        use_effect_with((), move |_| {
            let loop_enabled = loop_enabled.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = tauri_invoke("get_loop_enabled", JsValue::NULL).await;
                if let Ok(enabled) = serde_wasm_bindgen::from_value::<bool>(result) {
                    loop_enabled.set(enabled);
                }
            });
            || ()
        });
    }

    // Collapsible section states
    let source_files_collapsed = use_state(|| false);
    let frames_collapsed = use_state(|| false);
    let cuts_collapsed = use_state(|| false);
    let controls_collapsed = use_state(|| false);

    // Video cuts state
    let video_cuts = use_state(|| Vec::<VideoCut>::new());
    let selected_cut = use_state(|| None::<VideoCut>);
    let is_cutting = use_state(|| false);

    // Previews state
    let previews = use_state(|| Vec::<Preview>::new());
    let selected_preview = use_state(|| None::<Preview>);
    let previews_collapsed = use_state(|| false);
    let preview_menu_open_id = use_state(|| None::<String>);

    {
        let project_id = project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
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

                // Fetch previews
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_previews", args).await {
                    result => {
                        if let Ok(p) = serde_wasm_bindgen::from_value(result) {
                            previews.set(p);
                        }
                        // Not critical, just log silently
                    }
                }
            });

            || ()
        });
    }

    // Storage for listener cleanup (prevents memory leaks)
    let progress_listener_handle = use_mut_ref(|| None::<JsValue>);
    let progress_listener_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);
    let complete_listener_handle = use_mut_ref(|| None::<JsValue>);
    let complete_listener_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);

    // Global listener for conversion progress events
    {
        let active_conversions_ref = active_conversions_ref.clone();
        let progress_listener_handle = progress_listener_handle.clone();
        let progress_listener_closure = progress_listener_closure.clone();

        use_effect_with((), move |_| {
            let active_conversions_ref = active_conversions_ref.clone();
            let progress_listener_handle = progress_listener_handle.clone();
            let progress_listener_closure_storage = progress_listener_closure.clone();

            // Create a callback that updates the active conversions map
            // Note: No re-render trigger here - polling handles UI updates
            let progress_callback = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                if let Ok(payload) = js_sys::Reflect::get(&event, &"payload".into()) {
                    let source_id = js_sys::Reflect::get(&payload, &"source_id".into())
                        .ok()
                        .and_then(|v| v.as_string());
                    let percentage = js_sys::Reflect::get(&payload, &"percentage".into())
                        .ok()
                        .and_then(|v| v.as_f64())
                        .map(|p| p as u8);

                    if let (Some(source_id), Some(percentage)) = (source_id, percentage) {
                        // Update the ref directly - NO re-render trigger here
                        // UI updates are handled by polling in a separate effect
                        active_conversions_ref.borrow_mut().insert(source_id, percentage);
                    }
                }
            });

            let js_callback = progress_callback.as_ref().unchecked_ref::<js_sys::Function>().clone();

            // Store closure to keep it alive (will be dropped on cleanup)
            *progress_listener_closure_storage.borrow_mut() = Some(progress_callback);

            // Set up the listener and store handle for cleanup
            let handle_storage = progress_listener_handle.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let unlisten = tauri_listen("conversion-progress", &js_callback).await;
                *handle_storage.borrow_mut() = Some(unlisten);
            });

            // Cleanup on unmount
            let progress_listener_handle = progress_listener_handle.clone();
            let progress_listener_closure = progress_listener_closure.clone();
            move || {
                // Unlisten and drop closure
                if let Some(unlisten) = progress_listener_handle.borrow_mut().take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        tauri_unlisten(unlisten).await;
                    });
                }
                progress_listener_closure.borrow_mut().take();
            }
        });
    }

    // Polling-based UI updates for conversion progress (decoupled from event frequency)
    // Updates UI at ~10fps max, regardless of how many progress events arrive
    {
        let active_conversions_ref = active_conversions_ref.clone();
        let conversions_update_trigger = conversions_update_trigger.clone();
        use_effect_with((), move |_| {
            let active_conversions_ref = active_conversions_ref.clone();
            let conversions_update_trigger = conversions_update_trigger.clone();

            let interval = gloo_timers::callback::Interval::new(100, move || {
                // Only trigger re-render if there are active conversions
                if !active_conversions_ref.borrow().is_empty() {
                    conversions_update_trigger.set(js_sys::Date::now() as u32);
                }
            });

            // Cleanup on unmount
            move || drop(interval)
        });
    }

    // Global listener for conversion-complete events
    {
        let active_conversions_ref = active_conversions_ref.clone();
        let conversions_update_trigger = conversions_update_trigger.clone();
        let conversion_message = conversion_message.clone();
        let conversion_success_folder = conversion_success_folder.clone();
        let error_message = error_message.clone();
        let frame_directories = frame_directories.clone();
        let project_id = project_id.clone();
        let complete_listener_handle = complete_listener_handle.clone();
        let complete_listener_closure = complete_listener_closure.clone();

        use_effect_with((), move |_| {
            let active_conversions_ref = active_conversions_ref.clone();
            let conversions_update_trigger = conversions_update_trigger.clone();
            let conversion_message = conversion_message.clone();
            let conversion_success_folder = conversion_success_folder.clone();
            let error_message = error_message.clone();
            let frame_directories = frame_directories.clone();
            let project_id = project_id.clone();
            let complete_listener_handle = complete_listener_handle.clone();
            let complete_listener_closure_storage = complete_listener_closure.clone();

            let complete_callback = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                if let Ok(payload) = js_sys::Reflect::get(&event, &"payload".into()) {
                    let source_id = js_sys::Reflect::get(&payload, &"source_id".into())
                        .ok()
                        .and_then(|v| v.as_string());
                    let success = js_sys::Reflect::get(&payload, &"success".into())
                        .ok()
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let message = js_sys::Reflect::get(&payload, &"message".into())
                        .ok()
                        .and_then(|v| v.as_string());

                    if let Some(source_id) = source_id {
                        web_sys::console::log_1(&format!("🔴 CONVERSION COMPLETE EVENT: {} (success={})", source_id, success).into());

                        // Remove from active conversions
                        active_conversions_ref.borrow_mut().remove(&source_id);
                        conversions_update_trigger.set(*conversions_update_trigger + 1);

                        if success {
                            if let Some(msg) = message {
                                // Parse folder path from "ASCII frames saved to: {path} ({frames} frames, {bytes} bytes)"
                                if let Some(start) = msg.find("saved to: ") {
                                    let after_prefix = &msg[start + 10..];
                                    if let Some(end) = after_prefix.find(" (") {
                                        let folder_path = after_prefix[..end].to_string();
                                        conversion_success_folder.set(Some(folder_path));
                                    }
                                }
                                conversion_message.set(Some(msg));
                            }

                            // Refresh frame directories
                            let frame_directories = frame_directories.clone();
                            let project_id = (*project_id).clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                                    frame_directories.set(frames);
                                }
                            });
                        } else {
                            if let Some(msg) = message {
                                error_message.set(Some(msg));
                            } else {
                                error_message.set(Some("Conversion failed".to_string()));
                            }
                        }
                    }
                }
            });

            let js_callback = complete_callback.as_ref().unchecked_ref::<js_sys::Function>().clone();

            // Store closure to keep it alive (will be dropped on cleanup)
            *complete_listener_closure_storage.borrow_mut() = Some(complete_callback);

            // Set up the listener and store handle for cleanup
            let handle_storage = complete_listener_handle.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let unlisten = tauri_listen("conversion-complete", &js_callback).await;
                *handle_storage.borrow_mut() = Some(unlisten);
            });

            // Cleanup on unmount
            let complete_listener_handle = complete_listener_handle.clone();
            let complete_listener_closure = complete_listener_closure.clone();
            move || {
                // Unlisten and drop closure
                if let Some(unlisten) = complete_listener_handle.borrow_mut().take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        tauri_unlisten(unlisten).await;
                    });
                }
                complete_listener_closure.borrow_mut().take();
            }
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
                        "cut_id": cut_id,
                        "file_path": file_path
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

    // Callback to delete a source file
    let on_delete_source_file = {
        let source_files = source_files.clone();
        let project_id = project_id.clone();
        let selected_source = selected_source.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();

        Callback::from(move |source: SourceContent| {
            let source_files = source_files.clone();
            let project_id = (*project_id).clone();
            let source_id = source.id.clone();
            let file_path = source.file_path.clone();
            let selected_source = selected_source.clone();
            let frame_directories = frame_directories.clone();
            let video_cuts = video_cuts.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "source_id": source_id.clone(),
                        "file_path": file_path
                    }
                })).unwrap();
                let _ = tauri_invoke("delete_source_file", args).await;

                // Clear selection if deleted source was selected
                if selected_source.as_ref().map(|s| s.id == source_id).unwrap_or(false) {
                    selected_source.set(None);
                }

                // Refresh source files
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(sources) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await) {
                    source_files.set(sources);
                }

                // Refresh frame directories (in case associated conversions were deleted)
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                    frame_directories.set(frames);
                }

                // Refresh cuts (in case associated cuts were deleted)
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await) {
                    video_cuts.set(cuts);
                }
            });
        })
    };

    // Callback to delete a frame directory
    let on_delete_frame = {
        let frame_directories = frame_directories.clone();
        let project_id = project_id.clone();
        let selected_frame_dir = selected_frame_dir.clone();

        Callback::from(move |frame_dir: FrameDirectory| {
            let frame_directories = frame_directories.clone();
            let project_id = (*project_id).clone();
            let directory_path = frame_dir.directory_path.clone();
            let selected_frame_dir = selected_frame_dir.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "directoryPath": directory_path.clone()
                })).unwrap();
                let _ = tauri_invoke("delete_frame_directory", args).await;

                // Clear selection if deleted frame dir was selected
                if selected_frame_dir.as_ref().map(|s| s.directory_path == directory_path).unwrap_or(false) {
                    selected_frame_dir.set(None);
                }

                // Refresh frame directories
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                    frame_directories.set(frames);
                }
            });
        })
    };

    // Callback to cut frame segment
    let on_cut_frames = {
        let selected_frame_dir = selected_frame_dir.clone();
        let frame_directories = frame_directories.clone();
        let project_id = project_id.clone();
        let conversion_message = conversion_message.clone();
        let conversion_success_folder = conversion_success_folder.clone();
        let error_message = error_message.clone();

        Callback::from(move |(start_index, end_index): (usize, usize)| {
            if let Some(frame_dir) = &*selected_frame_dir {
                let folder_path = frame_dir.directory_path.clone();
                let project_id = (*project_id).clone();
                let frame_directories = frame_directories.clone();
                let conversion_message = conversion_message.clone();
                let conversion_success_folder = conversion_success_folder.clone();
                let error_message = error_message.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "request": {
                            "folderPath": folder_path,
                            "startIndex": start_index,
                            "endIndex": end_index
                        }
                    })).unwrap();

                    match tauri_invoke("cut_frames", args).await {
                        result => {
                            match serde_wasm_bindgen::from_value::<String>(result) {
                                Ok(msg) => {
                                    web_sys::console::log_1(&format!("✅ Frames cut successfully: {}", msg).into());
                                    
                                    // Parse folder path from message
                                    if let Some(start) = msg.find("saved to: ") {
                                        let after_prefix = &msg[start + 10..];
                                        if let Some(end) = after_prefix.find(" (") {
                                            let folder_path = after_prefix[..end].to_string();
                                            conversion_success_folder.set(Some(folder_path));
                                        }
                                    }
                                    conversion_message.set(Some(msg));

                                    // Refresh frame directories
                                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                    if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                                        frame_directories.set(frames);
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(&format!("❌ Failed to cut frames: {:?}", e).into());
                                    error_message.set(Some("Failed to cut frames.".to_string()));
                                }
                            }
                        }
                    }
                });
            }
        })
    };

    // Callback to handle preview creation from VideoPlayer
    let on_preview_created = {
        let previews = previews.clone();
        let selected_preview = selected_preview.clone();
        let selected_frame_dir = selected_frame_dir.clone();
        let project_id = project_id.clone();

        Callback::from(move |_preview_value: serde_json::Value| {
            // Refresh previews list and select the new one
            let previews = previews.clone();
            let selected_preview = selected_preview.clone();
            let selected_frame_dir = selected_frame_dir.clone();
            let project_id = (*project_id).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) = serde_wasm_bindgen::from_value::<Vec<Preview>>(tauri_invoke("get_project_previews", args).await) {
                    // Find and select the newly created preview (it should be first since sorted by creation_date DESC)
                    if let Some(new_preview) = new_previews.first().cloned() {
                        selected_preview.set(Some(new_preview));
                        // Clear frame dir selection to show the preview
                        selected_frame_dir.set(None);
                    }
                    previews.set(new_previews);
                }
            });
        })
    };

    // Callback to delete a preview
    let on_delete_preview = {
        let previews = previews.clone();
        let selected_preview = selected_preview.clone();
        let project_id = project_id.clone();

        Callback::from(move |preview: Preview| {
            let previews = previews.clone();
            let preview_id = preview.id.clone();
            let folder_path = preview.folder_path.clone();
            let selected_preview = selected_preview.clone();
            let project_id = (*project_id).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "preview_id": preview_id.clone(),
                        "folder_path": folder_path
                    }
                })).unwrap();
                let _ = tauri_invoke("delete_preview", args).await;

                // Clear selection if deleted preview was selected
                if selected_preview.as_ref().map(|p| p.id == preview_id).unwrap_or(false) {
                    selected_preview.set(None);
                }

                // Refresh previews
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_previews", args).await) {
                    previews.set(new_previews);
                }
            });
        })
    };

    // Callback to rename a preview
    let _on_rename_preview = {
        let previews = previews.clone();
        let project_id = project_id.clone();

        Callback::from(move |(_preview_id, _new_name): (String, String)| {
            let previews = previews.clone();
            let project_id = (*project_id).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_previews", args).await) {
                    previews.set(new_previews);
                }
            });
        })
    };

    // Compute conversions HTML before the main html! macro
    // Read conversions_update_trigger to create re-render dependency
    let _trigger = *conversions_update_trigger;
    let conversions_html = {
        let conversions = active_conversions_ref.borrow();
        if !conversions.is_empty() {
            // Helper to look up source name by ID
            let get_source_name = |source_id: &str| -> String {
                source_files.iter()
                    .find(|s| s.id == source_id)
                    .map(|s| {
                        s.custom_name.clone().unwrap_or_else(|| {
                            std::path::Path::new(&s.file_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string()
                        })
                    })
                    .unwrap_or_else(|| "Unknown".to_string())
            };

            html! {
                <div class="conversions-container">
                    {conversions.iter().map(|(source_id, percentage)| {
                        let name = get_source_name(source_id);
                        html! {
                            <div class="conversion-progress" key={source_id.clone()}>
                                <span class="conversion-progress-text">{format!("Converting {}: {}%", name, percentage)}</span>
                                <div class="conversion-progress-bar"><div class="conversion-progress-fill" style={format!("width: {}%", percentage)} /></div>
                            </div>
                        }
                    }).collect::<Html>()}
                </div>
            }
        } else {
            html! {}
        }
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
                        on_delete_file={Some(on_delete_source_file.clone())}
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
                        on_open_file={Some(Callback::from(|source: SourceContent| {
                            let file_path = source.file_path.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                // Get the parent directory of the source file
                                if let Some(parent) = std::path::Path::new(&file_path).parent() {
                                    let folder_path = parent.to_string_lossy().to_string();
                                    let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                                    let _ = tauri_invoke("open_directory", args).await;
                                }
                            });
                        }))}
                    />

                    <AvailableFrames frame_directories={(*frame_directories).clone()} selected_frame_dir={(*selected_frame_dir).clone()} selected_frame_settings={(*selected_frame_settings).clone()} frames_collapsed={*frames_collapsed} on_toggle_collapsed={{
                            let frames_collapsed = frames_collapsed.clone();
                            Callback::from(move |_| {
                                frames_collapsed.set(!*frames_collapsed);
                            })
                        }}
                        on_select_frame_dir={{
                            let selected_frame_dir = selected_frame_dir.clone();
                            let selected_preview = selected_preview.clone();
                            Callback::from(move |frame_dir: FrameDirectory| {
                                selected_frame_dir.set(Some(frame_dir));
                                // Clear preview selection when selecting a frame dir
                                selected_preview.set(None);
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
                        on_delete_frame={Some(on_delete_frame.clone())}
                        on_open_frame={Some(Callback::from(|frame_dir: FrameDirectory| {
                            let folder_path = frame_dir.directory_path.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                                let _ = tauri_invoke("open_directory", args).await;
                            });
                        }))}
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
                                        content_type: ContentType::Video,
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
                                                    content_type: ContentType::Video,
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
                        on_open_cut={Some(Callback::from(|cut: VideoCut| {
                            let file_path = cut.file_path.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                // Get the parent directory of the cut file
                                if let Some(parent) = std::path::Path::new(&file_path).parent() {
                                    let folder_path = parent.to_string_lossy().to_string();
                                    let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                                    let _ = tauri_invoke("open_directory", args).await;
                                }
                            });
                        }))}
                    />

                    // Previews section
                    <div class="source-files-column">
                        <div class="collapsible-header" onclick={{
                            let previews_collapsed = previews_collapsed.clone();
                            Callback::from(move |_| previews_collapsed.set(!*previews_collapsed))
                        }}>
                            <yew_icons::Icon icon_id={yew_icons::IconId::LucideCamera} width={"16"} height={"16"} />
                            <span>{format!("Previews ({})", previews.len())}</span>
                            <span class="chevron-icon">{if *previews_collapsed { "+" } else { "-" }}</span>
                        </div>
                        if !*previews_collapsed {
                            <div class="source-list">
                                if previews.is_empty() {
                                    <div class="empty-message">{"No previews yet"}</div>
                                } else {
                                    { for previews.iter().map(|preview| {
                                        let is_selected = selected_preview.as_ref().map(|p| p.id == preview.id).unwrap_or(false);
                                        let is_menu_open = preview_menu_open_id.as_ref().map(|id| id == &preview.id).unwrap_or(false);
                                        let preview_clone = preview.clone();
                                        let preview_for_delete = preview.clone();
                                        let preview_for_open = preview.clone();
                                        let selected_preview = selected_preview.clone();
                                        let selected_frame_dir = selected_frame_dir.clone();
                                        let on_delete = on_delete_preview.clone();
                                        let preview_menu_open_id = preview_menu_open_id.clone();

                                        let display_name = preview.custom_name.clone()
                                            .unwrap_or_else(|| preview.folder_name.clone());

                                        html! {
                                            <div class={classes!("source-item", is_selected.then_some("selected"))}
                                                onclick={{
                                                    let preview = preview_clone.clone();
                                                    let selected_preview = selected_preview.clone();
                                                    let selected_frame_dir = selected_frame_dir.clone();
                                                    Callback::from(move |_| {
                                                        selected_preview.set(Some(preview.clone()));
                                                        // Clear frame dir selection to show the preview
                                                        selected_frame_dir.set(None);
                                                    })
                                                }}>
                                                <div class="source-item-name-wrapper">
                                                    <span class="source-item-name">{display_name}</span>
                                                </div>
                                                <div class="source-item-buttons">
                                                    <div class="item-menu-container">
                                                        <button class="source-item-btn menu-btn" type="button" title="More options" onclick={{
                                                            let preview_id = preview.id.clone();
                                                            let preview_menu_open_id = preview_menu_open_id.clone();
                                                            Callback::from(move |e: MouseEvent| {
                                                                e.stop_propagation();
                                                                if preview_menu_open_id.as_ref().map(|id| id == &preview_id).unwrap_or(false) {
                                                                    preview_menu_open_id.set(None);
                                                                } else {
                                                                    preview_menu_open_id.set(Some(preview_id.clone()));
                                                                }
                                                            })
                                                        }}>
                                                            <yew_icons::Icon icon_id={yew_icons::IconId::LucideMoreHorizontal} width={"14"} height={"14"} />
                                                        </button>
                                                        {if is_menu_open {
                                                            html! {
                                                                <div class="item-dropdown-menu">
                                                                    <button type="button" class="dropdown-menu-item" onclick={{
                                                                        let folder_path = preview_for_open.folder_path.clone();
                                                                        let preview_menu_open_id = preview_menu_open_id.clone();
                                                                        Callback::from(move |e: MouseEvent| {
                                                                            e.stop_propagation();
                                                                            preview_menu_open_id.set(None);
                                                                            let folder_path = folder_path.clone();
                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                                                                                let _ = tauri_invoke("open_directory", args).await;
                                                                            });
                                                                        })
                                                                    }}>
                                                                        <yew_icons::Icon icon_id={yew_icons::IconId::LucideFolderOpen} width={"14"} height={"14"} />
                                                                        <span>{"Open"}</span>
                                                                    </button>
                                                                    <button type="button" class="dropdown-menu-item delete" onclick={{
                                                                        let preview = preview_for_delete.clone();
                                                                        let on_delete = on_delete.clone();
                                                                        let preview_menu_open_id = preview_menu_open_id.clone();
                                                                        Callback::from(move |e: MouseEvent| {
                                                                            e.stop_propagation();
                                                                            preview_menu_open_id.set(None);
                                                                            on_delete.emit(preview.clone());
                                                                        })
                                                                    }}>
                                                                        <yew_icons::Icon icon_id={yew_icons::IconId::LucideTrash2} width={"14"} height={"14"} />
                                                                        <span>{"Delete"}</span>
                                                                    </button>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }}
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    })}
                                }
                            </div>
                        }
                    </div>

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
                        loop_enabled={*loop_enabled}
                        on_loop_change={{
                            let loop_enabled = loop_enabled.clone();
                            Callback::from(move |enabled: bool| {
                                loop_enabled.set(enabled);
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "enabled": enabled })).unwrap();
                                    let _ = tauri_invoke("set_loop_enabled", args).await;
                                });
                            })
                        }}
                    />

                    // Conversion progress indicators (multiple parallel conversions)
                    {conversions_html}

                    // Conversion success notification
                    if let Some(folder_path) = &*conversion_success_folder {
                        <div class="conversion-notification" style="z-index: 1000;">
                            <span class="conversion-notification-text">{"ASCII frames generated"}</span>
                            <div class="conversion-notification-actions">
                                <button
                                    class="nav-btn"
                                    type="button"
                                    title="Open folder"
                                    style="position: relative; z-index: 1001; margin-right: 5px;"
                                    onclick={{
                                        let folder_path = folder_path.clone();
                                        Callback::from(move |_| {
                                            let folder_path = folder_path.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                                                let _ = tauri_invoke("open_directory", args).await;
                                            });
                                        })
                                    }}
                                >
                                    <yew_icons::Icon icon_id={yew_icons::IconId::LucideFolderOpen} width={"16"} height={"16"} />
                                </button>
                                <button
                                    class="nav-btn"
                                    type="button"
                                    title="Dismiss"
                                    style="position: relative; z-index: 1001;"
                                    onclick={{
                                        let conversion_success_folder = conversion_success_folder.clone();
                                        let conversion_message = conversion_message.clone();
                                        Callback::from(move |_| {
                                            conversion_success_folder.set(None);
                                            conversion_message.set(None);
                                        })
                                    }}
                                >
                                    <yew_icons::Icon icon_id={yew_icons::IconId::LucideXCircle} width={"16"} height={"16"} />
                                </button>
                            </div>
                        </div>
                    }
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
                                        if source.content_type == ContentType::Image {
                                            html! {
                                                <img src={url.clone()} alt="Source Image" loading="lazy" decoding="async" style="max-width:100%;max-height:100%;object-fit:contain;border-radius:8px;" />
                                            }
                                        } else if source.content_type == ContentType::Video {
                                            html! { 
                                                <VideoPlayer
                                                src={url.clone()}
                                                class={classes!("source-video")}
                                                should_play={if *is_playing {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                loop_enabled={*loop_enabled}
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
                                            
                                                is_converting={Some(active_conversions_ref.borrow().contains_key(&source.id))}
                                                on_conversion_start={Some({
                                                    let active_conversions_ref = active_conversions_ref.clone();
                                                    let conversions_update_trigger = conversions_update_trigger.clone();
                                                    Callback::from(move |(source_id, name): (String, String)| {
                                                        web_sys::console::log_1(&format!("🟢 CONVERSION START: {} ({})", name, source_id).into());
                                                        active_conversions_ref.borrow_mut().insert(source_id.clone(), 0u8);
                                                        web_sys::console::log_1(&format!("📊 Active conversions: {}", active_conversions_ref.borrow().len()).into());
                                                        conversions_update_trigger.set(*conversions_update_trigger + 1);
                                                    })
                                                })}
                                                on_conversion_complete={Some({
                                                    let active_conversions_ref = active_conversions_ref.clone();
                                                    let conversions_update_trigger = conversions_update_trigger.clone();
                                                    Callback::from(move |source_id: String| {
                                                        web_sys::console::log_1(&format!("🔴 CONVERSION COMPLETE: {}", source_id).into());
                                                        active_conversions_ref.borrow_mut().remove(&source_id);
                                                        web_sys::console::log_1(&format!("📊 Active conversions: {}", active_conversions_ref.borrow().len()).into());
                                                        conversions_update_trigger.set(*conversions_update_trigger + 1);
                                                    })
                                                })}
                                                conversion_message={(*conversion_message).clone()}
                                                on_conversion_message_change={Some({
                                                    let conversion_message = conversion_message.clone();
                                                    let conversion_success_folder = conversion_success_folder.clone();
                                                    Callback::from(move |v: Option<String>| {
                                                        if let Some(ref msg) = v {
                                                            // Parse folder path from "ASCII frames saved to: {path} ({frames} frames, {bytes} bytes)"
                                                            if let Some(start) = msg.find("saved to: ") {
                                                                let after_prefix = &msg[start + 10..];
                                                                if let Some(end) = after_prefix.find(" (") {
                                                                    let folder_path = after_prefix[..end].to_string();
                                                                    conversion_success_folder.set(Some(folder_path));
                                                                }
                                                            }
                                                        } else {
                                                            conversion_success_folder.set(None);
                                                        }
                                                        conversion_message.set(v);
                                                    })
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

                                                color_frames_default={*color_frames_default}
                                                extract_audio_default={*extract_audio_default}

                                                on_preview_created={Some(on_preview_created.clone())}
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
                                    // Show selected preview if any
                                    if let Some(preview) = &*selected_preview {
                                        html! {
                                            <AsciiFramesViewer
                                                directory_path={preview.folder_path.clone()}
                                                fps={preview.settings.fps}
                                                settings={None}
                                                should_play={Some(false)}
                                                should_reset={false}
                                                seek_percentage={None}
                                                on_loading_changed={{
                                                    let frames_loading = frames_loading.clone();
                                                    Callback::from(move |is_loading: bool| {
                                                        frames_loading.set(is_loading);
                                                    })
                                                }}
                                                frame_speed={None}
                                                on_frame_speed_change={{
                                                    Callback::from(move |_speed: u32| {})
                                                }}
                                                selected_speed={crate::components::ascii_frames_viewer::SpeedSelection::Base}
                                                on_speed_selection_change={{
                                                    Callback::from(move |_selection: crate::components::ascii_frames_viewer::SpeedSelection| {})
                                                }}
                                                loop_enabled={false}
                                                on_cut_frames={None::<Callback<(usize, usize)>>}
                                                is_cutting={false}
                                            />
                                        }
                                    } else if frame_directories.is_empty() && previews.is_empty() {
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
                                                loop_enabled={*loop_enabled}
                                                on_cut_frames={Some(on_cut_frames.clone())}
                                                is_cutting={false}
                                            />
                                        }
                                    } else {
                                        html! { <span>{"Select a frame directory or preview"}</span> }
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
