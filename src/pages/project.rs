use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;

use super::open::Project;
use crate::components::ascii_frames_viewer::{AsciiFramesViewer, ConversionSettings};
use crate::components::explorer::{
    ExplorerLayout, ExplorerTree, ResourceRef, ResourcesTree, SidebarState, TreeNodeId,
};
use crate::components::settings::available_cuts::VideoCut;
use crate::components::settings::Controls;
use crate::components::tab_bar::{OpenTab, TabBar};
use crate::components::video_player::VideoPlayer;

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

#[wasm_bindgen(inline_js = r#"
const __viewerControlsSyncRaf = new WeakMap();

function readControlsHeight(container, selector) {
  const el = container.querySelector(selector);
  if (!el) return 0;
  const rect = el.getBoundingClientRect();
  const style = globalThis.getComputedStyle ? getComputedStyle(el) : null;
  const borderTop = style ? parseFloat(style.borderTopWidth || '0') : 0;
  const borderBottom = style ? parseFloat(style.borderBottomWidth || '0') : 0;
  const borderY = (Number.isFinite(borderTop) ? borderTop : 0) + (Number.isFinite(borderBottom) ? borderBottom : 0);
  const rectHeight = Math.ceil(rect.height || 0);
  const scrollHeight = Math.ceil((el.scrollHeight || 0) + borderY);
  const offsetHeight = Math.ceil(el.offsetHeight || 0);
  return Math.max(rectHeight, scrollHeight, offsetHeight);
}

export function syncViewerControlsHeight(container) {
  if (!container) return;
  const height = Math.max(
    readControlsHeight(container, '#video-controls'),
    readControlsHeight(container, '#frames-controls')
  );
  if (height > 0) {
    const next = `${height}px`;
    if (container.style.getPropertyValue('--viewer-controls-height') !== next) {
      container.style.setProperty('--viewer-controls-height', next);
    }
  } else {
    container.style.removeProperty('--viewer-controls-height');
  }
}

export function scheduleSyncViewerControlsHeight(container) {
  if (!container) return;
  if (__viewerControlsSyncRaf.has(container)) return;
  const rafId = requestAnimationFrame(() => {
    __viewerControlsSyncRaf.delete(container);
    syncViewerControlsHeight(container);
  });
  __viewerControlsSyncRaf.set(container, rafId);
}

export function cancelScheduledSyncViewerControlsHeight(container) {
  if (!container) return;
  const rafId = __viewerControlsSyncRaf.get(container);
  if (rafId) {
    cancelAnimationFrame(rafId);
    __viewerControlsSyncRaf.delete(container);
  }
}

export function observeResize(element, callback) {
  let rafId = 0;
  let lastWidth = 0;
  let lastHeight = 0;
  const observer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const { width, height } = entry.contentRect;
      lastWidth = width;
      lastHeight = height;
    }
    if (!rafId) {
      rafId = requestAnimationFrame(() => {
        rafId = 0;
        callback(lastWidth, lastHeight);
      });
    }
  });
  observer.observe(element);
  return { observer, get rafId() { return rafId; } };
}

export function disconnectObserver(observer) {
  if (!observer) return;
  if (observer.rafId) {
    cancelAnimationFrame(observer.rafId);
  }
  if (observer.observer?.disconnect) {
    observer.observer.disconnect();
    return;
  }
  if (observer.disconnect) {
    observer.disconnect();
  }
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = syncViewerControlsHeight)]
    fn sync_viewer_controls_height(container: &web_sys::Element);

    #[wasm_bindgen(js_name = scheduleSyncViewerControlsHeight)]
    fn schedule_sync_viewer_controls_height(container: &web_sys::Element);

    #[wasm_bindgen(js_name = cancelScheduledSyncViewerControlsHeight)]
    fn cancel_scheduled_sync_viewer_controls_height(container: &web_sys::Element);

    #[wasm_bindgen(js_name = observeResize)]
    fn observe_resize(element: &web_sys::Element, callback: &Closure<dyn Fn(f64, f64)>) -> JsValue;

    #[wasm_bindgen(js_name = disconnectObserver)]
    fn disconnect_observer(observer: &JsValue);
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

const UI_PROGRESS_MIN_INTERVAL_MS: f64 = 50.0;
const UI_PROGRESS_MIN_DELTA_PERCENT: f64 = 0.5;
const FRAME_SYNC_MIN_INTERVAL_MS: f64 = 33.0;
const FRAME_SYNC_MIN_DELTA: f64 = 0.01;

#[derive(Default)]
struct PlaybackSyncLimiter {
    last_progress_emit_ms: f64,
    last_progress_percent: Option<f64>,
    last_frame_sync_emit_ms: f64,
    last_frame_sync_value: Option<f64>,
}

fn file_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string()
}

fn without_extension(name: &str) -> String {
    std::path::Path::new(name)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(name)
        .to_string()
}

fn source_tab_label(source: &SourceContent) -> String {
    source
        .custom_name
        .clone()
        .unwrap_or_else(|| file_name_from_path(&source.file_path))
}

fn cut_tab_label(cut: &VideoCut) -> String {
    cut.custom_name
        .clone()
        .unwrap_or_else(|| file_name_from_path(&cut.file_path))
}

fn frame_dir_tab_label(frame_dir: &FrameDirectory) -> String {
    frame_dir.name.clone()
}

fn preview_tab_label(preview: &Preview) -> String {
    preview
        .custom_name
        .clone()
        .unwrap_or_else(|| preview.folder_name.clone())
}

fn tab_id_for_resource(resource: &ResourceRef) -> String {
    match resource {
        ResourceRef::SourceFile { source_id } => format!("tab:source:{}", source_id),
        ResourceRef::VideoCut { cut_id } => format!("tab:cut:{}", cut_id),
        ResourceRef::FrameDirectory { directory_path } => {
            format!("tab:framedir:{}", directory_path)
        }
        ResourceRef::Preview { preview_id } => format!("tab:preview:{}", preview_id),
    }
}

fn open_or_activate_tab(
    open_tabs: &UseStateHandle<Vec<OpenTab>>,
    active_tab_id: &UseStateHandle<Option<String>>,
    tab: OpenTab,
) {
    let mut next_tabs = (**open_tabs).clone();
    let mut changed = false;

    if let Some(existing_tab) = next_tabs.iter_mut().find(|t| t.id == tab.id) {
        if existing_tab.label != tab.label || existing_tab.resource != tab.resource {
            *existing_tab = tab.clone();
            changed = true;
        }
    } else {
        next_tabs.push(tab.clone());
        changed = true;
    }

    if changed {
        open_tabs.set(next_tabs);
    }

    if (*active_tab_id).as_deref() != Some(tab.id.as_str()) {
        active_tab_id.set(Some(tab.id));
    }
}

#[derive(Properties, PartialEq)]
pub struct ProjectPageProps {
    pub project_id: String,
    pub on_project_name_change: Callback<String>,
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
    let url_cache = use_state(|| HashMap::<String, String>::new()); // URL cache to avoid recomputing asset URLs
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
    let frames_sync_seek_percentage = use_state(|| None::<f64>);
    let playback_sync_limiter = use_mut_ref(PlaybackSyncLimiter::default);
    let frames_loading = use_state(|| false);
    let frame_speed = use_state(|| None::<u32>);
    let current_conversion_id = use_state(|| None::<String>);
    let selected_speed =
        use_state(|| crate::components::ascii_frames_viewer::SpeedSelection::Custom);
    let loop_enabled = use_state(|| true);
    let video_volume = use_state(|| 1.0f64);
    let video_is_muted = use_state(|| false);
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

    // In custom-speed mode, clear transient sync seek so frame playback uses only user speed.
    {
        let selected_speed = selected_speed.clone();
        let frames_sync_seek_percentage = frames_sync_seek_percentage.clone();
        use_effect_with((*selected_speed).clone(), move |speed| {
            if *speed == crate::components::ascii_frames_viewer::SpeedSelection::Custom {
                frames_sync_seek_percentage.set(None);
            }
            || ()
        });
    }

    let controls_collapsed = use_state(|| false);

    // Explorer sidebar state
    let sidebar_state = use_state(SidebarState::default);
    let explorer_layout = use_state(ExplorerLayout::default);
    let selected_node_id = use_state(|| None::<TreeNodeId>);
    let open_tabs = use_state(|| Vec::<OpenTab>::new());
    let active_tab_id = use_state(|| None::<String>);

    // Video cuts state
    let video_cuts = use_state(|| Vec::<VideoCut>::new());
    let selected_cut = use_state(|| None::<VideoCut>);
    let is_cutting = use_state(|| false);
    let is_preprocessing = use_state(|| false);

    // Previews state
    let previews = use_state(|| Vec::<Preview>::new());
    let selected_preview = use_state(|| None::<Preview>);
    let preview_container_ref = use_node_ref();

    {
        let preview_container_ref = preview_container_ref.clone();
        let has_video_preview = matches!(
            selected_source.as_ref().map(|s| &s.content_type),
            Some(ContentType::Video)
        );
        let has_frames_preview = selected_frame_dir.is_some();
        use_effect_with((has_video_preview, has_frames_preview), move |_| {
            let mut observers = Vec::<JsValue>::new();
            let mut on_resize = None::<Closure<dyn Fn(f64, f64)>>;

            if let Some(container) = preview_container_ref.cast::<web_sys::Element>() {
                schedule_sync_viewer_controls_height(&container);

                let container_for_callback = container.clone();
                let callback = Closure::wrap(Box::new(move |_: f64, _: f64| {
                    schedule_sync_viewer_controls_height(&container_for_callback);
                }) as Box<dyn Fn(f64, f64)>);

                if let Ok(Some(video_controls)) = container.query_selector("#video-controls") {
                    observers.push(observe_resize(&video_controls, &callback));
                }
                if let Ok(Some(frames_controls)) = container.query_selector("#frames-controls") {
                    observers.push(observe_resize(&frames_controls, &callback));
                }

                on_resize = Some(callback);
            }

            move || {
                for observer in &observers {
                    disconnect_observer(observer);
                }
                if let Some(container) = preview_container_ref.cast::<web_sys::Element>() {
                    cancel_scheduled_sync_viewer_controls_height(&container);
                }
                if let Some(callback) = on_resize {
                    drop(callback);
                }
            }
        });
    }

    {
        let project_id = project_id.clone();
        let project = project.clone();
        let on_project_name_change = props.on_project_name_change.clone();
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
                        if let Ok(p) = serde_wasm_bindgen::from_value::<Project>(result) {
                            on_project_name_change.emit(p.project_name.clone());
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
                        active_conversions_ref
                            .borrow_mut()
                            .insert(source_id, percentage);
                    }
                }
            });

            let js_callback = progress_callback
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone();

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
                        web_sys::console::log_1(
                            &format!(
                                "🔴 CONVERSION COMPLETE EVENT: {} (success={})",
                                source_id, success
                            )
                            .into(),
                        );

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
                                let args = serde_wasm_bindgen::to_value(
                                    &json!({ "projectId": project_id }),
                                )
                                .unwrap();
                                if let Ok(frames) = serde_wasm_bindgen::from_value(
                                    tauri_invoke("get_project_frames", args).await,
                                ) {
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

            let js_callback = complete_callback
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone();

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
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();

        Callback::from(move |source: SourceContent| {
            let resource = ResourceRef::SourceFile {
                source_id: source.id.clone(),
            };
            open_or_activate_tab(
                &open_tabs,
                &active_tab_id,
                OpenTab {
                    id: tab_id_for_resource(&resource),
                    resource,
                    label: source_tab_label(&source),
                },
            );

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
                        if let Ok(prepared) =
                            serde_wasm_bindgen::from_value::<PreparedMedia>(result)
                        {
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
                    }))
                    .unwrap();

                    match tauri_invoke("cut_video", args).await {
                        result => {
                            is_cutting.set(false);
                            if serde_wasm_bindgen::from_value::<VideoCut>(result.clone()).is_ok() {
                                // Refresh cuts list
                                let args = serde_wasm_bindgen::to_value(
                                    &json!({ "projectId": project_id }),
                                )
                                .unwrap();
                                if let Ok(cuts) = serde_wasm_bindgen::from_value(
                                    tauri_invoke("get_project_cuts", args).await,
                                ) {
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

    // Callback to preprocess video
    let on_preprocess_video = {
        let project_id = project_id.clone();
        let selected_source = selected_source.clone();
        let video_cuts = video_cuts.clone();
        let is_preprocessing = is_preprocessing.clone();
        let error_message = error_message.clone();

        Callback::from(move |(preset, custom_filter): (String, Option<String>)| {
            if let Some(source) = &*selected_source {
                let project_id = (*project_id).clone();
                let source_file_id = source.id.clone();
                let source_file_path = source.file_path.clone();
                let video_cuts = video_cuts.clone();
                let is_preprocessing = is_preprocessing.clone();
                let error_message = error_message.clone();

                is_preprocessing.set(true);

                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "args": {
                            "request": {
                                "source_file_path": source_file_path,
                                "project_id": project_id,
                                "source_file_id": source_file_id,
                                "preset": preset,
                                "custom_filter": custom_filter
                            }
                        }
                    }))
                    .unwrap();

                    match tauri_invoke("preprocess_video", args).await {
                        result => {
                            is_preprocessing.set(false);
                            if serde_wasm_bindgen::from_value::<VideoCut>(result.clone()).is_ok() {
                                // Refresh cuts list
                                let args = serde_wasm_bindgen::to_value(
                                    &json!({ "projectId": project_id }),
                                )
                                .unwrap();
                                if let Ok(cuts) = serde_wasm_bindgen::from_value(
                                    tauri_invoke("get_project_cuts", args).await,
                                ) {
                                    video_cuts.set(cuts);
                                }
                            } else {
                                error_message.set(Some("Failed to preprocess video".to_string()));
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
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();

        Callback::from(move |cut: VideoCut| {
            let video_cuts = video_cuts.clone();
            let project_id = (*project_id).clone();
            let cut_id = cut.id.clone();
            let file_path = cut.file_path.clone();
            let selected_cut = selected_cut.clone();
            let open_tabs = open_tabs.clone();
            let active_tab_id = active_tab_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "cut_id": cut_id,
                        "file_path": file_path
                    }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_cut", args).await;

                // Clear selection if deleted cut was selected
                if selected_cut
                    .as_ref()
                    .map(|s| s.id == cut_id)
                    .unwrap_or(false)
                {
                    selected_cut.set(None);
                }

                let tab_id = tab_id_for_resource(&ResourceRef::VideoCut {
                    cut_id: cut_id.clone(),
                });
                let mut tabs = (*open_tabs).clone();
                let original_len = tabs.len();
                tabs.retain(|tab| tab.id != tab_id);
                if tabs.len() != original_len {
                    open_tabs.set(tabs);
                    if (*active_tab_id).as_deref() == Some(tab_id.as_str()) {
                        active_tab_id.set(None);
                    }
                }

                // Refresh
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
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
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
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
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();

        Callback::from(move |source: SourceContent| {
            let source_files = source_files.clone();
            let project_id = (*project_id).clone();
            let source_id = source.id.clone();
            let file_path = source.file_path.clone();
            let selected_source = selected_source.clone();
            let frame_directories = frame_directories.clone();
            let video_cuts = video_cuts.clone();
            let open_tabs = open_tabs.clone();
            let active_tab_id = active_tab_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "source_id": source_id.clone(),
                        "file_path": file_path
                    }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_source_file", args).await;

                // Clear selection if deleted source was selected
                if selected_source
                    .as_ref()
                    .map(|s| s.id == source_id)
                    .unwrap_or(false)
                {
                    selected_source.set(None);
                }

                let tab_id = tab_id_for_resource(&ResourceRef::SourceFile {
                    source_id: source_id.clone(),
                });
                let mut tabs = (*open_tabs).clone();
                let original_len = tabs.len();
                tabs.retain(|tab| tab.id != tab_id);
                if tabs.len() != original_len {
                    open_tabs.set(tabs);
                    if (*active_tab_id).as_deref() == Some(tab_id.as_str()) {
                        active_tab_id.set(None);
                    }
                }

                // Refresh source files
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(sources) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await)
                {
                    source_files.set(sources);
                }

                // Refresh frame directories (in case associated conversions were deleted)
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(frames) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
                    frame_directories.set(frames);
                }

                // Refresh cuts (in case associated cuts were deleted)
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(cuts) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
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
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();

        Callback::from(move |frame_dir: FrameDirectory| {
            let frame_directories = frame_directories.clone();
            let project_id = (*project_id).clone();
            let directory_path = frame_dir.directory_path.clone();
            let selected_frame_dir = selected_frame_dir.clone();
            let open_tabs = open_tabs.clone();
            let active_tab_id = active_tab_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "directoryPath": directory_path.clone()
                }))
                .unwrap();
                let _ = tauri_invoke("delete_frame_directory", args).await;

                // Clear selection if deleted frame dir was selected
                if selected_frame_dir
                    .as_ref()
                    .map(|s| s.directory_path == directory_path)
                    .unwrap_or(false)
                {
                    selected_frame_dir.set(None);
                }

                let tab_id = tab_id_for_resource(&ResourceRef::FrameDirectory {
                    directory_path: directory_path.clone(),
                });
                let mut tabs = (*open_tabs).clone();
                let original_len = tabs.len();
                tabs.retain(|tab| tab.id != tab_id);
                if tabs.len() != original_len {
                    open_tabs.set(tabs);
                    if (*active_tab_id).as_deref() == Some(tab_id.as_str()) {
                        active_tab_id.set(None);
                    }
                }

                // Refresh frame directories
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(frames) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
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
                    }))
                    .unwrap();

                    match tauri_invoke("cut_frames", args).await {
                        result => {
                            match serde_wasm_bindgen::from_value::<String>(result) {
                                Ok(msg) => {
                                    web_sys::console::log_1(
                                        &format!("✅ Frames cut successfully: {}", msg).into(),
                                    );

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
                                    let args = serde_wasm_bindgen::to_value(
                                        &json!({ "projectId": project_id }),
                                    )
                                    .unwrap();
                                    if let Ok(frames) = serde_wasm_bindgen::from_value(
                                        tauri_invoke("get_project_frames", args).await,
                                    ) {
                                        frame_directories.set(frames);
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("❌ Failed to cut frames: {:?}", e).into(),
                                    );
                                    error_message.set(Some("Failed to cut frames.".to_string()));
                                }
                            }
                        }
                    }
                });
            }
        })
    };

    // Callback to crop frames
    let on_crop_frames = {
        let selected_frame_dir = selected_frame_dir.clone();
        let frame_directories = frame_directories.clone();
        let project_id = project_id.clone();
        let conversion_message = conversion_message.clone();
        let conversion_success_folder = conversion_success_folder.clone();
        let error_message = error_message.clone();

        Callback::from(
            move |(top, bottom, left, right): (usize, usize, usize, usize)| {
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
                                "top": top,
                                "bottom": bottom,
                                "left": left,
                                "right": right
                            }
                        }))
                        .unwrap();

                        match tauri_invoke("crop_frames", args).await {
                            result => match serde_wasm_bindgen::from_value::<String>(result) {
                                Ok(msg) => {
                                    web_sys::console::log_1(
                                        &format!("✅ Frames cropped successfully: {}", msg).into(),
                                    );

                                    if let Some(start) = msg.find("saved to: ") {
                                        let after_prefix = &msg[start + 10..];
                                        if let Some(end) = after_prefix.find(" (") {
                                            let folder_path = after_prefix[..end].to_string();
                                            conversion_success_folder.set(Some(folder_path));
                                        }
                                    }
                                    conversion_message.set(Some(msg));

                                    let args = serde_wasm_bindgen::to_value(
                                        &json!({ "projectId": project_id }),
                                    )
                                    .unwrap();
                                    if let Ok(frames) = serde_wasm_bindgen::from_value(
                                        tauri_invoke("get_project_frames", args).await,
                                    ) {
                                        frame_directories.set(frames);
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("❌ Failed to crop frames: {:?}", e).into(),
                                    );
                                    error_message.set(Some("Failed to crop frames.".to_string()));
                                }
                            },
                        }
                    });
                }
            },
        )
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
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) = serde_wasm_bindgen::from_value::<Vec<Preview>>(
                    tauri_invoke("get_project_previews", args).await,
                ) {
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
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();

        Callback::from(move |preview: Preview| {
            let previews = previews.clone();
            let preview_id = preview.id.clone();
            let folder_path = preview.folder_path.clone();
            let selected_preview = selected_preview.clone();
            let project_id = (*project_id).clone();
            let open_tabs = open_tabs.clone();
            let active_tab_id = active_tab_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "preview_id": preview_id.clone(),
                        "folder_path": folder_path
                    }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_preview", args).await;

                // Clear selection if deleted preview was selected
                if selected_preview
                    .as_ref()
                    .map(|p| p.id == preview_id)
                    .unwrap_or(false)
                {
                    selected_preview.set(None);
                }

                let tab_id = tab_id_for_resource(&ResourceRef::Preview {
                    preview_id: preview_id.clone(),
                });
                let mut tabs = (*open_tabs).clone();
                let original_len = tabs.len();
                tabs.retain(|tab| tab.id != tab_id);
                if tabs.len() != original_len {
                    open_tabs.set(tabs);
                    if (*active_tab_id).as_deref() == Some(tab_id.as_str()) {
                        active_tab_id.set(None);
                    }
                }

                // Refresh previews
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_previews", args).await)
                {
                    previews.set(new_previews);
                }
            });
        })
    };

    // Explorer sidebar: rename preview (refresh list)
    let on_rename_preview_explorer = {
        let previews = previews.clone();
        let project_id = project_id.clone();

        Callback::from(move |_preview: Preview| {
            let previews = previews.clone();
            let project_id = (*project_id).clone();

            wasm_bindgen_futures::spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(new_previews) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_previews", args).await)
                {
                    previews.set(new_previews);
                }
            });
        })
    };

    // Explorer sidebar: select frame dir (also clears preview + loads conversion settings)
    let on_select_frame_dir_explorer = {
        let selected_frame_dir = selected_frame_dir.clone();
        let selected_preview = selected_preview.clone();
        let selected_frame_settings = selected_frame_settings.clone();
        let frame_speed = frame_speed.clone();
        let current_conversion_id = current_conversion_id.clone();
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();
        Callback::from(move |frame_dir: FrameDirectory| {
            let resource = ResourceRef::FrameDirectory {
                directory_path: frame_dir.directory_path.clone(),
            };
            open_or_activate_tab(
                &open_tabs,
                &active_tab_id,
                OpenTab {
                    id: tab_id_for_resource(&resource),
                    resource,
                    label: frame_dir_tab_label(&frame_dir),
                },
            );

            let directory_path = frame_dir.directory_path.clone();
            selected_frame_dir.set(Some(frame_dir));
            selected_preview.set(None);

            // Fetch conversion settings for this frame directory
            let selected_frame_settings = selected_frame_settings.clone();
            let frame_speed = frame_speed.clone();
            let current_conversion_id = current_conversion_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "folderPath": directory_path })).unwrap();
                match tauri_invoke("get_conversion_by_folder_path", args).await {
                    result => {
                        if let Ok(Some(conversion)) =
                            serde_wasm_bindgen::from_value::<Option<serde_json::Value>>(result)
                        {
                            let conversion_id = conversion
                                .get("id")
                                .and_then(|id| id.as_str())
                                .map(|s| s.to_string());
                            if let Some(settings) = conversion.get("settings") {
                                if let Ok(conv_settings) =
                                    serde_json::from_value::<ConversionSettings>(settings.clone())
                                {
                                    frame_speed.set(Some(conv_settings.frame_speed));
                                    selected_frame_settings.set(Some(conv_settings));
                                    current_conversion_id.set(conversion_id);
                                    return;
                                }
                            }
                        }
                        selected_frame_settings.set(None);
                        frame_speed.set(None);
                        current_conversion_id.set(None);
                    }
                }
            });
        })
    };

    // Explorer sidebar: select preview (also clears frame dir)
    let on_select_preview_explorer = {
        let selected_preview = selected_preview.clone();
        let selected_frame_dir = selected_frame_dir.clone();
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();
        Callback::from(move |preview: Preview| {
            let resource = ResourceRef::Preview {
                preview_id: preview.id.clone(),
            };
            open_or_activate_tab(
                &open_tabs,
                &active_tab_id,
                OpenTab {
                    id: tab_id_for_resource(&resource),
                    resource,
                    label: preview_tab_label(&preview),
                },
            );

            selected_preview.set(Some(preview));
            selected_frame_dir.set(None);
        })
    };

    // Explorer sidebar: select cut (prepare media for video player)
    let on_select_cut_explorer = {
        let selected_cut = selected_cut.clone();
        let selected_source = selected_source.clone();
        let asset_url = asset_url.clone();
        let is_loading_media = is_loading_media.clone();
        let url_cache = url_cache.clone();
        let error_message = error_message.clone();
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();
        Callback::from(move |cut: VideoCut| {
            let resource = ResourceRef::VideoCut {
                cut_id: cut.id.clone(),
            };
            open_or_activate_tab(
                &open_tabs,
                &active_tab_id,
                OpenTab {
                    id: tab_id_for_resource(&resource),
                    resource,
                    label: cut_tab_label(&cut),
                },
            );

            selected_cut.set(Some(cut.clone()));
            let file_path = cut.file_path.clone();
            if let Some(cached_url) = url_cache.get(&file_path) {
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
            let selected_source = selected_source.clone();
            let asset_url = asset_url.clone();
            let is_loading_media = is_loading_media.clone();
            let url_cache = url_cache.clone();
            let error_message = error_message.clone();
            is_loading_media.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "path": file_path })).unwrap();
                match tauri_invoke("prepare_media", args).await {
                    result => {
                        if let Ok(prepared) =
                            serde_wasm_bindgen::from_value::<PreparedMedia>(result)
                        {
                            let asset_url_str = app_convert_file_src(&prepared.cached_abs_path);
                            let mut cache = (*url_cache).clone();
                            cache.insert(cut.file_path.clone(), asset_url_str.clone());
                            url_cache.set(cache);
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
                            asset_url.set(Some(asset_url_str));
                        } else {
                            error_message.set(Some("Failed to prepare cut video.".to_string()));
                        }
                        is_loading_media.set(false);
                    }
                }
            });
        })
    };

    // Explorer sidebar: rename source (refresh list)
    let on_rename_source_explorer = {
        let source_files = source_files.clone();
        let project_id = project_id.clone();
        Callback::from(move |_source: SourceContent| {
            let source_files = source_files.clone();
            let project_id = (*project_id).clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(s) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await)
                {
                    source_files.set(s);
                }
            });
        })
    };

    // Explorer sidebar: rename frame (refresh list)
    let on_rename_frame_explorer = {
        let frame_directories = frame_directories.clone();
        let project_id = project_id.clone();
        Callback::from(move |_frame: FrameDirectory| {
            let frame_directories = frame_directories.clone();
            let project_id = (*project_id).clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(frames) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
                    frame_directories.set(frames);
                }
            });
        })
    };

    // Explorer sidebar: rename cut (wraps existing on_rename_cut which expects (String, String))
    let on_rename_cut_explorer = {
        let on_rename_cut = on_rename_cut.clone();
        Callback::from(move |cut: VideoCut| {
            on_rename_cut.emit((cut.id.clone(), String::new()));
        })
    };

    // Explorer sidebar: open folder callbacks
    let on_open_source_explorer = Callback::from(|source: SourceContent| {
        let file_path = source.file_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let folder_path = parent.to_string_lossy().to_string();
                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                let _ = tauri_invoke("open_directory", args).await;
            }
        });
    });

    let on_open_frame_explorer = Callback::from(|frame_dir: FrameDirectory| {
        let folder_path = frame_dir.directory_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
            let _ = tauri_invoke("open_directory", args).await;
        });
    });

    let on_open_cut_explorer = Callback::from(|cut: VideoCut| {
        let file_path = cut.file_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let folder_path = parent.to_string_lossy().to_string();
                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                let _ = tauri_invoke("open_directory", args).await;
            }
        });
    });

    let on_open_preview_explorer = Callback::from(|preview: Preview| {
        let folder_path = preview.folder_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
            let _ = tauri_invoke("open_directory", args).await;
        });
    });

    // Explorer sidebar: add files callback
    let on_add_files_explorer = {
        let project_id = project_id.clone();
        let source_files = source_files.clone();
        let error_message = error_message.clone();
        let is_adding_files = is_adding_files.clone();
        let file_progress_map = file_progress_map.clone();
        Callback::from(move |_| {
            let project_id = project_id.clone();
            let source_files = source_files.clone();
            let error_message = error_message.clone();
            let is_adding_files = is_adding_files.clone();
            let file_progress_map = file_progress_map.clone();
            wasm_bindgen_futures::spawn_local(async move {
                error_message.set(None);
                match tauri_invoke("pick_files", JsValue::NULL).await {
                    result => match serde_wasm_bindgen::from_value::<Vec<String>>(result) {
                        Ok(file_paths) => {
                            if !file_paths.is_empty() {
                                let mut initial_map = HashMap::new();
                                for file_path in file_paths.iter() {
                                    let file_name = std::path::Path::new(file_path)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    initial_map.insert(
                                        file_name.clone(),
                                        FileProgress {
                                            file_name,
                                            status: "pending".to_string(),
                                            message: "Waiting...".to_string(),
                                            percentage: None,
                                        },
                                    );
                                }
                                file_progress_map.set(initial_map);
                                is_adding_files.set(true);
                                let file_progress_map_clone = file_progress_map.clone();
                                let callback: Closure<dyn Fn(JsValue)> =
                                    Closure::new(move |event: JsValue| {
                                        if let Ok(payload) =
                                            js_sys::Reflect::get(&event, &"payload".into())
                                        {
                                            if let Ok(progress) =
                                                serde_wasm_bindgen::from_value::<FileProgress>(
                                                    payload,
                                                )
                                            {
                                                let mut map = (*file_progress_map_clone).clone();
                                                map.insert(progress.file_name.clone(), progress);
                                                file_progress_map_clone.set(map);
                                            }
                                        }
                                    });
                                let unlisten_handle = tauri_listen(
                                    "file-progress",
                                    callback.as_ref().unchecked_ref(),
                                )
                                .await;
                                if !project_id.is_empty() {
                                    let invoke_args = AddSourceFilesArgs {
                                        request: AddSourceFilesRequest {
                                            project_id: (*project_id).to_string(),
                                            file_paths,
                                        },
                                    };
                                    let add_files_args = serde_wasm_bindgen::to_value(
                                        &json!({ "args": invoke_args }),
                                    )
                                    .unwrap();
                                    let _ = tauri_invoke("add_source_files", add_files_args).await;
                                    tauri_unlisten(unlisten_handle).await;
                                    drop(callback);
                                    is_adding_files.set(false);
                                    let args = serde_wasm_bindgen::to_value(
                                        &json!({ "projectId": *project_id }),
                                    )
                                    .unwrap();
                                    if let Ok(s) = serde_wasm_bindgen::from_value(
                                        tauri_invoke("get_project_sources", args).await,
                                    ) {
                                        source_files.set(s);
                                    }
                                } else {
                                    tauri_unlisten(unlisten_handle).await;
                                    drop(callback);
                                    is_adding_files.set(false);
                                }
                            }
                        }
                        Err(_) => {
                            error_message.set(Some("Failed to pick files.".to_string()));
                        }
                    },
                }
            });
        })
    };

    // Explorer sidebar: toggle section callback
    let on_toggle_section = {
        let sidebar_state = sidebar_state.clone();
        Callback::from(move |section: String| {
            let mut state = (*sidebar_state).clone();
            match section.as_str() {
                "resources" => state.resources_expanded = !state.resources_expanded,
                "explorer" => state.explorer_expanded = !state.explorer_expanded,
                "controls" => state.controls_expanded = !state.controls_expanded,
                "res:source_files" => state.source_files_expanded = !state.source_files_expanded,
                "res:original_files" => {
                    state.original_files_expanded = !state.original_files_expanded
                }
                "res:cuts" => state.cuts_expanded = !state.cuts_expanded,
                "res:frames" => state.frames_expanded = !state.frames_expanded,
                "res:source_frames" => state.source_frames_expanded = !state.source_frames_expanded,
                "res:frame_cuts" => state.frame_cuts_expanded = !state.frame_cuts_expanded,
                "res:previews" => state.previews_expanded = !state.previews_expanded,
                _ => {}
            }
            sidebar_state.set(state);
        })
    };

    // Explorer sidebar: layout change callback (persists to DB)
    let on_explorer_layout_change = {
        let explorer_layout = explorer_layout.clone();
        let project_id = project_id.clone();
        Callback::from(move |new_layout: ExplorerLayout| {
            explorer_layout.set(new_layout.clone());
            let project_id = (*project_id).clone();
            wasm_bindgen_futures::spawn_local(async move {
                let layout_json = serde_json::to_string(&new_layout).unwrap_or_default();
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "project_id": project_id,
                        "layout_json": layout_json
                    }
                }))
                .unwrap();
                let _ = tauri_invoke("save_explorer_layout", args).await;
            });
        })
    };

    let on_select_tab = {
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
        let on_select_source = on_select_source.clone();
        let on_select_frame_dir_explorer = on_select_frame_dir_explorer.clone();
        let on_select_cut_explorer = on_select_cut_explorer.clone();
        let on_select_preview_explorer = on_select_preview_explorer.clone();

        Callback::from(move |tab_id: String| {
            if (*active_tab_id).as_deref() != Some(tab_id.as_str()) {
                active_tab_id.set(Some(tab_id.clone()));
            }

            if let Some(tab) = open_tabs.iter().find(|t| t.id == tab_id).cloned() {
                match tab.resource {
                    ResourceRef::SourceFile { source_id } => {
                        if let Some(source) =
                            source_files.iter().find(|s| s.id == source_id).cloned()
                        {
                            on_select_source.emit(source);
                        }
                    }
                    ResourceRef::VideoCut { cut_id } => {
                        if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id).cloned() {
                            on_select_cut_explorer.emit(cut);
                        }
                    }
                    ResourceRef::FrameDirectory { directory_path } => {
                        if let Some(frame_dir) = frame_directories
                            .iter()
                            .find(|f| f.directory_path == directory_path)
                            .cloned()
                        {
                            on_select_frame_dir_explorer.emit(frame_dir);
                        }
                    }
                    ResourceRef::Preview { preview_id } => {
                        if let Some(preview) = previews.iter().find(|p| p.id == preview_id).cloned()
                        {
                            on_select_preview_explorer.emit(preview);
                        }
                    }
                }
            }
        })
    };

    let on_close_tab = {
        let open_tabs = open_tabs.clone();
        let active_tab_id = active_tab_id.clone();
        let selected_source = selected_source.clone();
        let selected_cut = selected_cut.clone();
        let selected_frame_dir = selected_frame_dir.clone();
        let selected_preview = selected_preview.clone();
        let asset_url = asset_url.clone();

        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
        let on_select_source = on_select_source.clone();
        let on_select_frame_dir_explorer = on_select_frame_dir_explorer.clone();
        let on_select_cut_explorer = on_select_cut_explorer.clone();
        let on_select_preview_explorer = on_select_preview_explorer.clone();

        Callback::from(move |tab_id: String| {
            let current_tabs = (*open_tabs).clone();
            let Some(closed_index) = current_tabs.iter().position(|t| t.id == tab_id) else {
                return;
            };

            let mut updated_tabs = current_tabs.clone();
            updated_tabs.remove(closed_index);
            open_tabs.set(updated_tabs.clone());

            let current_active = (*active_tab_id).clone();
            let next_active = if current_active.as_deref() == Some(tab_id.as_str()) {
                if updated_tabs.is_empty() {
                    None
                } else {
                    let next_index = if closed_index < updated_tabs.len() {
                        closed_index
                    } else {
                        updated_tabs.len() - 1
                    };
                    Some(updated_tabs[next_index].id.clone())
                }
            } else {
                current_active.filter(|id| updated_tabs.iter().any(|t| &t.id == id))
            };

            active_tab_id.set(next_active.clone());

            if let Some(next_id) = next_active {
                if let Some(tab) = updated_tabs.iter().find(|t| t.id == next_id).cloned() {
                    match tab.resource {
                        ResourceRef::SourceFile { source_id } => {
                            if let Some(source) =
                                source_files.iter().find(|s| s.id == source_id).cloned()
                            {
                                on_select_source.emit(source);
                            }
                        }
                        ResourceRef::VideoCut { cut_id } => {
                            if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id).cloned() {
                                on_select_cut_explorer.emit(cut);
                            }
                        }
                        ResourceRef::FrameDirectory { directory_path } => {
                            if let Some(frame_dir) = frame_directories
                                .iter()
                                .find(|f| f.directory_path == directory_path)
                                .cloned()
                            {
                                on_select_frame_dir_explorer.emit(frame_dir);
                            }
                        }
                        ResourceRef::Preview { preview_id } => {
                            if let Some(preview) =
                                previews.iter().find(|p| p.id == preview_id).cloned()
                            {
                                on_select_preview_explorer.emit(preview);
                            }
                        }
                    }
                }
            } else {
                selected_source.set(None);
                selected_cut.set(None);
                selected_frame_dir.set(None);
                selected_preview.set(None);
                asset_url.set(None);
            }
        })
    };

    let on_reorder_tabs = {
        let open_tabs = open_tabs.clone();
        Callback::from(move |ordered_ids: Vec<String>| {
            let current_tabs = (*open_tabs).clone();
            if ordered_ids.len() != current_tabs.len() {
                return;
            }

            let mut reordered_tabs = Vec::with_capacity(current_tabs.len());
            for id in &ordered_ids {
                if let Some(tab) = current_tabs.iter().find(|tab| &tab.id == id) {
                    reordered_tabs.push(tab.clone());
                }
            }

            if reordered_tabs.len() != current_tabs.len() {
                return;
            }

            if reordered_tabs != current_tabs {
                open_tabs.set(reordered_tabs);
            }
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
                source_files
                    .iter()
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

    let source_preview_label = selected_source.as_ref().map(|source| {
        let source_name = source
            .custom_name
            .clone()
            .unwrap_or_else(|| file_name_from_path(&source.file_path));
        format!("SOURCE VIDEO: {}", without_extension(&source_name))
    });

    let frames_preview_label = if let Some(preview) = &*selected_preview {
        let name = preview
            .custom_name
            .clone()
            .unwrap_or_else(|| preview.folder_name.clone());
        Some(format!("FRAMES: {}", name))
    } else if let Some(frame_dir) = &*selected_frame_dir {
        Some(format!("FRAMES: {}", frame_dir.name))
    } else {
        None
    };

    html! {
        <div id="project-page" class="container project-page">
            <div id="project-layout" class="project-layout">
                <div id="project-explorer-sidebar" class="explorer-sidebar">
                    <div id="project-sidebar-scroll" class="explorer-sidebar__scroll-area">
                        <ResourcesTree
                            source_files={(*source_files).clone()}
                            video_cuts={(*video_cuts).clone()}
                            frame_directories={(*frame_directories).clone()}
                            previews={(*previews).clone()}
                            sidebar_state={(*sidebar_state).clone()}
                            selected_node_id={(*selected_node_id).clone()}
                            on_toggle_section={on_toggle_section.clone()}
                            on_select_source={on_select_source.clone()}
                            on_select_frame_dir={on_select_frame_dir_explorer.clone()}
                            on_select_cut={on_select_cut_explorer.clone()}
                            on_select_preview={on_select_preview_explorer.clone()}
                            on_delete_source={on_delete_source_file.clone()}
                            on_delete_frame={on_delete_frame.clone()}
                            on_delete_cut={on_delete_cut.clone()}
                            on_delete_preview={on_delete_preview.clone()}
                            on_rename_source={on_rename_source_explorer.clone()}
                            on_rename_frame={on_rename_frame_explorer.clone()}
                            on_rename_cut={on_rename_cut_explorer.clone()}
                            on_rename_preview={on_rename_preview_explorer.clone()}
                            on_open_source={on_open_source_explorer.clone()}
                            on_open_frame={on_open_frame_explorer.clone()}
                            on_open_cut={on_open_cut_explorer.clone()}
                            on_open_preview={on_open_preview_explorer.clone()}
                            on_add_files={Some(on_add_files_explorer.clone())}
                        />
                        <ExplorerTree
                            explorer_layout={(*explorer_layout).clone()}
                            source_files={(*source_files).clone()}
                            video_cuts={(*video_cuts).clone()}
                            frame_directories={(*frame_directories).clone()}
                            previews={(*previews).clone()}
                            is_expanded={sidebar_state.explorer_expanded}
                            selected_node_id={(*selected_node_id).clone()}
                            on_toggle_section={{
                                let on_toggle = on_toggle_section.clone();
                                Callback::from(move |_| on_toggle.emit("explorer".to_string()))
                            }}
                            on_layout_change={on_explorer_layout_change.clone()}
                            on_select_source={on_select_source.clone()}
                            on_select_frame_dir={on_select_frame_dir_explorer.clone()}
                            on_select_cut={on_select_cut_explorer.clone()}
                            on_select_preview={on_select_preview_explorer.clone()}
                        />
                        // Conversion progress indicators
                        {conversions_html}

                        // Conversion success notification
                        if let Some(folder_path) = &*conversion_success_folder {
                            <div id="project-conversion-notification" class="conversion-notification" style="z-index: 1000;">
                                <span class="conversion-notification-text">{"ASCII frames generated"}</span>
                                <div class="conversion-notification-actions">
                                    <button
                                        id="project-conversion-open-btn"
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
                                        id="project-conversion-dismiss-btn"
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
                    <div id="project-sidebar-bottom" class="explorer-sidebar__bottom">
                        // Controls as a collapsible section (like Outline in VS Code)
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
                            volume={*video_volume}
                            is_muted={*video_is_muted}
                            on_volume_change={{
                                let video_volume = video_volume.clone();
                                Callback::from(move |value: f64| {
                                    video_volume.set(value.clamp(0.0, 1.0));
                                })
                            }}
                            on_is_muted_change={{
                                let video_is_muted = video_is_muted.clone();
                                Callback::from(move |muted: bool| {
                                    video_is_muted.set(muted);
                                })
                            }}
                        />
                    </div>
                </div>

                <div id="project-main-content" class="main-content">
                    if let Some(error) = &*error_message {
                        <div id="project-error-alert" class="alert alert-error">{error}</div>
                    }

                    if *is_adding_files && !file_progress_map.is_empty() {
                        <div id="project-add-files-progress" class="progress-container">
                            <h3>{"Adding Files"}</h3>
                            <div id="project-add-files-list" class="progress-list">
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

                    if !open_tabs.is_empty() {
                        <TabBar
                            tabs={(*open_tabs).clone()}
                            active_tab_id={(*active_tab_id).clone()}
                            on_select_tab={on_select_tab.clone()}
                            on_close_tab={on_close_tab.clone()}
                            on_reorder_tabs={on_reorder_tabs.clone()}
                        />
                    }

                    <div id="project-preview-container" class="preview-container" ref={preview_container_ref.clone()}>
                        <div id="project-source-column" class="preview-column">
                            if let Some(label) = &source_preview_label {
                                <div id="project-source-label" class="preview-label">{label.clone()}</div>
                            }
                            <div id="project-source-square" class="square">
                                {
                                    if *is_loading_media {
                                        html! { <div id="project-source-loading" class="loading">{"Loading media..."}</div> }
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
                                                volume={*video_volume}
                                                is_muted={*video_is_muted}
                                                seek_percentage={*seek_percentage}
                                                on_progress={{
                                                    let synced_progress = synced_progress.clone();
                                                    let frames_sync_seek_percentage = frames_sync_seek_percentage.clone();
                                                    let selected_speed = selected_speed.clone();
                                                    let selected_frame_dir = selected_frame_dir.clone();
                                                    let is_playing = is_playing.clone();
                                                    let playback_sync_limiter = playback_sync_limiter.clone();
                                                    Callback::from(move |progress: f64| {
                                                        let clamped_progress = progress.clamp(0.0, 1.0);
                                                        let progress_percent = clamped_progress * 100.0;
                                                        let now = js_sys::Date::now();

                                                        let mut next_progress = None::<f64>;
                                                        let mut next_frame_sync = None::<f64>;

                                                        {
                                                            let mut limiter = playback_sync_limiter.borrow_mut();

                                                            let should_emit_progress = limiter
                                                                .last_progress_percent
                                                                .map(|last| {
                                                                    (progress_percent - last).abs()
                                                                        >= UI_PROGRESS_MIN_DELTA_PERCENT
                                                                        || now - limiter.last_progress_emit_ms
                                                                            >= UI_PROGRESS_MIN_INTERVAL_MS
                                                                        || clamped_progress <= 0.0
                                                                        || clamped_progress >= 1.0
                                                                })
                                                                .unwrap_or(true);

                                                            if should_emit_progress {
                                                                limiter.last_progress_emit_ms = now;
                                                                limiter.last_progress_percent = Some(progress_percent);
                                                                next_progress = Some(progress_percent);
                                                            }

                                                            let should_sync_frames = *is_playing
                                                                && *selected_speed
                                                                    == crate::components::ascii_frames_viewer::SpeedSelection::Base
                                                                && selected_frame_dir.is_some();

                                                            if should_sync_frames {
                                                                let should_emit_frame_sync = limiter
                                                                    .last_frame_sync_value
                                                                    .map(|last| {
                                                                        (clamped_progress - last).abs()
                                                                            >= FRAME_SYNC_MIN_DELTA
                                                                            || now - limiter.last_frame_sync_emit_ms
                                                                                >= FRAME_SYNC_MIN_INTERVAL_MS
                                                                            || clamped_progress <= 0.0
                                                                            || clamped_progress >= 1.0
                                                                    })
                                                                    .unwrap_or(true);

                                                                if should_emit_frame_sync {
                                                                    limiter.last_frame_sync_emit_ms = now;
                                                                    limiter.last_frame_sync_value = Some(clamped_progress);
                                                                    next_frame_sync = Some(clamped_progress);
                                                                }
                                                            }
                                                        }

                                                        if let Some(next) = next_progress {
                                                            if (*synced_progress - next).abs() > f64::EPSILON {
                                                                synced_progress.set(next);
                                                            }
                                                        }

                                                        if let Some(next) = next_frame_sync {
                                                            let should_set = frames_sync_seek_percentage
                                                                .as_ref()
                                                                .map(|current| {
                                                                    (*current - next).abs()
                                                                        >= FRAME_SYNC_MIN_DELTA
                                                                })
                                                                .unwrap_or(true);
                                                            if should_set {
                                                                frames_sync_seek_percentage.set(Some(next));
                                                            }
                                                        }
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

                                                on_preprocess_video={Some(on_preprocess_video.clone())}
                                                is_preprocessing={Some(*is_preprocessing)}

                                                color_frames_default={*color_frames_default}
                                                extract_audio_default={*extract_audio_default}

                                                on_preview_created={Some(on_preview_created.clone())}
                                            />
                                            }
                                        } else {
                                            html! { <span>{"Unsupported file type"}</span> }
                                        }
                                    } else {
                                        html! { <span id="project-source-empty">{"Select a source file to preview"}</span> }
                                    }
                                }
                            </div>
                        </div>

                        <div id="project-frames-column" class="preview-column">
                            if let Some(label) = &frames_preview_label {
                                <div id="project-frames-label" class="preview-label">{label.clone()}</div>
                            }
                            <div id="project-frames-square" class="square">
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
                                        html! { <span id="project-frames-none">{"No frames generated yet"}</span> }
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
                                                seek_percentage={{
                                                    if *selected_speed == crate::components::ascii_frames_viewer::SpeedSelection::Base {
                                                        (*frames_sync_seek_percentage).or(*seek_percentage)
                                                    } else {
                                                        *seek_percentage
                                                    }
                                                }}
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
                                                on_crop_frames={Some(on_crop_frames.clone())}
                                                is_cropping={false}
                                            />
                                        }
                                    } else {
                                        html! { <span id="project-frames-empty">{"Select a frame directory or preview"}</span> }
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
