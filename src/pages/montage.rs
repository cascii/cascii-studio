use gloo::events::EventListener;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

use super::open::Project;
use super::project::{FrameDirectory, PreparedMedia, Preview, SourceContent};
use super::project_cache::{
    get_project_sidebar_cache, set_project_sidebar_cache, ProjectSidebarCache,
};
use crate::components::ascii_frames_viewer::AsciiFramesViewer;
use crate::components::explorer::{
    hydrate_layout_from_project_content, project_content_from_layout, ExplorerLayout, ExplorerTree,
    ResourcesTree, TreeNodeId,
};
use crate::components::frame_media::{
    default_frame_render_mode, next_frame_render_mode, preload_frame_bundle,
    supported_frame_render_modes, FrameAssetMetadata, FrameRenderMode, PreloadedFrameBundle,
};
use crate::components::settings::available_cuts::VideoCut;
use crate::components::settings::{Controls, ToolsSection};
use crate::components::video_player::VideoPlayer;

#[wasm_bindgen(inline_js = r#"
export async function tauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);   // v2
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args); // v1
  throw new Error('Tauri invoke is not available on this page');
}

// Store drag data globally since DataTransfer can be unreliable in webviews
window.__dragData = null;
window.__pendingDrop = null;
window.__isPointerDragging = false;
window.__isPointerOverTimeline = false;
window.__dragGhostEl = null;
window.__lastPointerX = 0;
window.__lastPointerY = 0;
window.__justDroppedOnTimeline = false;
window.__dropTargetIndex = null;
window.__pendingDropIndex = null;
window.__dropIndicatorEl = null;

function ensureDragGhost() {
  if (window.__dragGhostEl) return window.__dragGhostEl;

  const el = document.createElement('div');
  el.className = 'pointer-drag-ghost';
  el.style.position = 'fixed';
  el.style.zIndex = '999999';
  el.style.pointerEvents = 'none';
  el.style.display = 'none';
  el.style.padding = '8px 12px';
  el.style.borderRadius = '6px';
  el.style.background = 'rgba(60, 60, 60, 0.92)';
  el.style.border = '1px solid rgba(255, 255, 255, 0.18)';
  el.style.color = '#f6f6f6';
  el.style.fontSize = '12px';
  el.style.maxWidth = '320px';
  el.style.whiteSpace = 'nowrap';
  el.style.overflow = 'hidden';
  el.style.textOverflow = 'ellipsis';
  el.style.boxShadow = '0 8px 24px rgba(0, 0, 0, 0.45)';
  el.style.backdropFilter = 'blur(6px)';
  el.style.left = '-9999px';
  el.style.top = '-9999px';
  document.body.appendChild(el);
  window.__dragGhostEl = el;
  return el;
}

function ensureDropIndicator() {
  if (window.__dropIndicatorEl) return window.__dropIndicatorEl;

  const el = document.createElement('div');
  el.className = 'drop-indicator';
  el.style.position = 'absolute';
  el.style.width = '3px';
  el.style.background = '#4a9eff';
  el.style.borderRadius = '2px';
  el.style.pointerEvents = 'none';
  el.style.display = 'none';
  el.style.zIndex = '1000';
  el.style.boxShadow = '0 0 8px rgba(74, 158, 255, 0.6)';
  document.body.appendChild(el);
  window.__dropIndicatorEl = el;
  return el;
}

function updateDragGhostContent() {
  const el = ensureDragGhost();
  try {
    const data = window.__dragData ? JSON.parse(window.__dragData) : null;
    const name = data?.name ?? 'Dragging...';
    el.textContent = name;
  } catch (_) {
    el.textContent = 'Dragging...';
  }
}

function showDragGhost() {
  const el = ensureDragGhost();
  updateDragGhostContent();
  el.style.display = 'block';
  moveDragGhost(window.__lastPointerX || 0, window.__lastPointerY || 0);
}

function hideDragGhost() {
  if (!window.__dragGhostEl) return;
  window.__dragGhostEl.style.display = 'none';
}

function hideDropIndicator() {
  if (!window.__dropIndicatorEl) return;
  window.__dropIndicatorEl.style.display = 'none';
  // Don't clear __dropTargetIndex here - it's needed by the drop handler
}

function moveDragGhost(x, y) {
  const el = ensureDragGhost();
  const offsetX = 12;
  const offsetY = 14;
  el.style.left = `${x + offsetX}px`;
  el.style.top = `${y + offsetY}px`;
}

function updateDropIndicator(x, y) {
  const track = document.querySelector('.timeline-track');
  const itemsRow = document.querySelector('.timeline-items-row');
  if (!track || !itemsRow) {
    hideDropIndicator();
    return;
  }

  const items = itemsRow.querySelectorAll('.timeline-item');
  if (items.length === 0) {
    hideDropIndicator();
    return;
  }

  const indicator = ensureDropIndicator();
  const trackRect = track.getBoundingClientRect();

  // Check if mouse is within track vertically
  if (y < trackRect.top || y > trackRect.bottom) {
    hideDropIndicator();
    return;
  }

  let targetIndex = items.length; // Default to end
  let indicatorX = 0;
  let indicatorTop = 0;
  let indicatorHeight = 0;

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    const rect = item.getBoundingClientRect();
    const midX = rect.left + rect.width / 2;

    if (x < midX) {
      targetIndex = i;
      indicatorX = rect.left - 4;
      indicatorTop = rect.top;
      indicatorHeight = rect.height;
      break;
    }

    // If we're past the last item's midpoint, place at end
    if (i === items.length - 1) {
      indicatorX = rect.right + 1;
      indicatorTop = rect.top;
      indicatorHeight = rect.height;
    }
  }

  // Don't show indicator if dragging timeline item to its own position or adjacent
  try {
    const data = window.__dragData ? JSON.parse(window.__dragData) : null;
    if (data && data.origin === 'timeline' && data.index !== undefined) {
      const fromIndex = data.index;
      if (targetIndex === fromIndex || targetIndex === fromIndex + 1) {
        hideDropIndicator();
        return;
      }
    }
  } catch (_) {}

  window.__dropTargetIndex = targetIndex;
  indicator.style.display = 'block';
  indicator.style.left = `${indicatorX}px`;
  indicator.style.top = `${indicatorTop}px`;
  indicator.style.height = `${indicatorHeight}px`;
}

export function setDragData(data) {
  window.__dragData = data;
  console.log('Drag data set:', data);
}

export function getDragData() {
  return window.__dragData;
}

export function clearDragData() {
  window.__dragData = null;
  window.__dropTargetIndex = null;
  hideDropIndicator();
}

export function getPendingDrop() {
  const data = window.__pendingDrop;
  window.__pendingDrop = null;
  return data;
}

export function getDropTargetIndex() {
  const idx = window.__pendingDropIndex;
  window.__pendingDropIndex = null;
  return idx;
}

export function consumeJustDropped() {
  const wasDropped = window.__justDroppedOnTimeline;
  window.__justDroppedOnTimeline = false;
  return wasDropped;
}

export function startPointerDrag() {
  window.__isPointerDragging = true;
  window.__isPointerOverTimeline = false;
  window.__dropTargetIndex = null;
  console.log('Pointer drag started');
  showDragGhost();
}

export function startPointerDragAt(x, y) {
  window.__lastPointerX = x;
  window.__lastPointerY = y;
  startPointerDrag();
}

// Set up listeners immediately when this module loads
(function() {
  console.log('Setting up drag listeners...');

  document.addEventListener('dragenter', function(e) {
    e.preventDefault();
  }, true);

  document.addEventListener('dragover', function(e) {
    e.preventDefault();
  }, true);

  document.addEventListener('drop', function(e) {
    e.preventDefault();
  }, true);

  document.addEventListener('dragend', function(e) {
    hideDropIndicator();
    hideDragGhost();
  }, true);

  // Pointer-based drag for webviews
  document.addEventListener('mousemove', function(e) {
    if (!window.__isPointerDragging || !window.__dragData) return;

    window.__lastPointerX = e.clientX;
    window.__lastPointerY = e.clientY;
    moveDragGhost(e.clientX, e.clientY);

    const container = document.querySelector('.timeline-container');
    if (!container) return;

    const rect = container.getBoundingClientRect();
    const isOver = e.clientX >= rect.left && e.clientX <= rect.right &&
                   e.clientY >= rect.top && e.clientY <= rect.bottom;

    if (isOver) {
      if (!window.__isPointerOverTimeline) {
        console.log('Pointer over timeline-container');
        window.__isPointerOverTimeline = true;
      }
      container.classList.add('drag-over');
      updateDropIndicator(e.clientX, e.clientY);
    } else {
      if (window.__isPointerOverTimeline) {
        console.log('Pointer left timeline-container');
        window.__isPointerOverTimeline = false;
      }
      container.classList.remove('drag-over');
      hideDropIndicator();
    }
  }, true);

  document.addEventListener('mouseup', function(e) {
    if (!window.__isPointerDragging) return;
    console.log('Pointer released');

    // Save the target index BEFORE hiding the indicator
    const savedTargetIndex = window.__dropTargetIndex;

    const container = document.querySelector('.timeline-container');
    if (container) container.classList.remove('drag-over');
    hideDragGhost();
    hideDropIndicator();

    if (window.__isPointerOverTimeline && window.__dragData) {
      console.log('Pointer drop on timeline, target index:', savedTargetIndex);
      console.log('Drag data:', window.__dragData);
      window.__pendingDrop = window.__dragData;
      window.__pendingDropIndex = savedTargetIndex;
      window.__justDroppedOnTimeline = true;
      console.log('Dispatching cascii:timeline-drop event');
      window.dispatchEvent(new CustomEvent('cascii:timeline-drop'));
      console.log('Event dispatched');
    } else {
      console.log('Drop NOT on timeline. isPointerOverTimeline:', window.__isPointerOverTimeline, 'dragData:', !!window.__dragData);
    }

    window.__dragData = null;
    window.__isPointerDragging = false;
    window.__isPointerOverTimeline = false;
    window.__dropTargetIndex = null;
  }, true);

  console.log('Drag listeners setup complete');
})();
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_name = setDragData)]
    fn set_drag_data(data: &str);

    #[wasm_bindgen(js_name = getDragData)]
    fn get_drag_data() -> Option<String>;

    #[wasm_bindgen(js_name = clearDragData)]
    fn clear_drag_data();

    #[wasm_bindgen(js_name = getPendingDrop)]
    fn get_pending_drop() -> Option<String>;

    #[wasm_bindgen(js_name = consumeJustDropped)]
    fn consume_just_dropped() -> bool;

    #[wasm_bindgen(js_name = getDropTargetIndex)]
    fn get_drop_target_index() -> Option<usize>;

    #[wasm_bindgen(js_name = startPointerDrag)]
    fn start_pointer_drag();

    #[wasm_bindgen(js_name = startPointerDragAt)]
    fn start_pointer_drag_at(x: i32, y: i32);
}

#[wasm_bindgen(inline_js = r#"
export function appConvertFileSrc(path) {
  if (window.__APP__convertFileSrc) {
    return window.__APP__convertFileSrc(path);
  }
  console.error('__APP__convertFileSrc not found');
  return path;
}

const __montageWarmVideos = new Map();

export function warmVideoAsset(url) {
  return new Promise((resolve) => {
    let video = __montageWarmVideos.get(url);
    if (!video) {
      video = document.createElement('video');
      video.preload = 'auto';
      video.muted = true;
      video.src = url;
      __montageWarmVideos.set(url, video);
    }

    let settled = false;
    const finish = (duration) => {
      if (settled) return;
      settled = true;
      video.removeEventListener('loadedmetadata', onReady);
      video.removeEventListener('canplay', onReady);
      video.removeEventListener('error', onError);
      resolve(Number.isFinite(duration) ? duration : null);
    };

    const onReady = () => finish(video.duration);
    const onError = () => finish(null);

    video.addEventListener('loadedmetadata', onReady, { once: true });
    video.addEventListener('canplay', onReady, { once: true });
    video.addEventListener('error', onError, { once: true });

    try {
      video.load();
    } catch (_) {
      finish(null);
    }
  });
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = appConvertFileSrc)]
    fn app_convert_file_src(path: &str) -> String;

    #[wasm_bindgen(js_name = warmVideoAsset)]
    async fn warm_video_asset(asset_url: &str) -> JsValue;
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayableItem {
    Video {
        clip_id: String,
        asset_url: String,
    },
    Frames {
        clip_id: String,
        directory_path: String,
        fps: u32,
        frame_render_mode: FrameRenderMode,
        preloaded_bundle: Rc<PreloadedFrameBundle>,
    },
}

#[derive(Properties, PartialEq)]
struct MontageVideoStillProps {
    pub src: String,
}

#[function_component(MontageVideoStill)]
fn montage_video_still(props: &MontageVideoStillProps) -> Html {
    let video_ref = use_node_ref();

    {
        let video_ref = video_ref.clone();
        let src = props.src.clone();
        use_effect_with(src, move |_| {
            if let Some(video) = video_ref.cast::<web_sys::HtmlVideoElement>() {
                video.set_muted(true);
                let _ = video.pause();
                let _ = video.set_current_time(0.0);
            }

            || ()
        });
    }

    let on_loaded_metadata = {
        let video_ref = video_ref.clone();
        Callback::from(move |_| {
            if let Some(video) = video_ref.cast::<web_sys::HtmlVideoElement>() {
                video.set_muted(true);
                let _ = video.pause();
                if video.current_time() <= 0.0 {
                    let _ = video.set_current_time(0.001);
                }
            }
        })
    };

    let on_seeked = {
        let video_ref = video_ref.clone();
        Callback::from(move |_| {
            if let Some(video) = video_ref.cast::<web_sys::HtmlVideoElement>() {
                let _ = video.pause();
            }
        })
    };

    html! {
        <video
            ref={video_ref}
            class="montage-overview-video"
            src={props.src.clone()}
            muted=true
            playsinline=true
            preload="auto"
            onloadedmetadata={on_loaded_metadata}
            onseeked={on_seeked}
        />
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineMediaType {
    Video,
    Frames,
    Frame,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineResourceKind {
    Source,
    Cut,
    AsciiConversion,
    Preview,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineClipItem {
    pub clip_id: String,
    pub name: String,
    pub media_type: TimelineMediaType,
    pub resource_kind: TimelineResourceKind,
    pub actual_resource_id: String,
    pub frame_render_mode: Option<FrameRenderMode>,
    pub length_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedTimelineInfo {
    pub timeline_id: String,
    pub project_id: String,
    pub creation_date: String,
    pub last_updated: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedTimelineClip {
    pub clip_id: String,
    pub project_id: String,
    pub timeline_id: String,
    pub order_index: i32,
    pub media_type: TimelineMediaType,
    pub resource_kind: TimelineResourceKind,
    pub actual_resource_id: String,
    pub frame_render_mode: Option<FrameRenderMode>,
    pub length_seconds: f64,
    pub creation_date: String,
    pub last_updated: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PersistedProjectTimeline {
    pub timeline: Option<PersistedTimelineInfo>,
    pub clips: Vec<PersistedTimelineClip>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveTimelineClip {
    pub clip_id: String,
    pub media_type: TimelineMediaType,
    pub resource_kind: TimelineResourceKind,
    pub actual_resource_id: String,
    pub frame_render_mode: Option<FrameRenderMode>,
    pub length_seconds: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PreloadStatus {
    Pending,
    Loading,
    Ready,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClipPreloadState {
    pub signature: String,
    pub status: PreloadStatus,
    pub video_asset_url: Option<String>,
    pub frame_bundle: Option<Rc<PreloadedFrameBundle>>,
    pub playback_fps: Option<u32>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct DragData {
    origin: String,    // "sidebar" or "timeline"
    item_type: String, // "source", "frame", "cut" (for sidebar)
    id: String,
    name: String,
    index: Option<usize>, // for timeline
}

fn file_name_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn frame_asset_metadata_from_directory(frame_dir: &FrameDirectory) -> FrameAssetMetadata {
    FrameAssetMetadata {
        directory_path: frame_dir.directory_path.clone(),
        fps: frame_dir.fps,
        frame_speed: frame_dir.frame_speed,
        frame_count: frame_dir.frame_count,
        color_enabled: frame_dir.color,
        output_mode: frame_dir.output_mode.clone(),
        foreground_color: frame_dir.foreground_color.clone(),
        background_color: frame_dir.background_color.clone(),
        has_text_frames: frame_dir.has_text_frames,
        has_color_frames: frame_dir.has_color_frames,
    }
}

fn frame_asset_metadata_from_preview(preview: &Preview) -> FrameAssetMetadata {
    FrameAssetMetadata {
        directory_path: preview.folder_path.clone(),
        fps: preview.settings.fps,
        frame_speed: preview.settings.fps,
        frame_count: preview.frame_count,
        color_enabled: preview.settings.color,
        output_mode: preview.settings.output_mode.clone(),
        foreground_color: preview.settings.foreground_color.clone(),
        background_color: preview.settings.background_color.clone(),
        has_text_frames: true,
        has_color_frames: preview.settings.output_mode != "text-only",
    }
}

fn frame_length_seconds(metadata: &FrameAssetMetadata, media_type: &TimelineMediaType) -> f64 {
    if matches!(media_type, TimelineMediaType::Frame) {
        return 1.0;
    }

    let playback_fps = if metadata.frame_speed > 0 {
        metadata.frame_speed
    } else if metadata.fps > 0 {
        metadata.fps
    } else {
        1
    };

    (metadata.frame_count.max(1) as f64 / playback_fps.max(1) as f64).max(0.01)
}

fn make_clip_signature(clip: &TimelineClipItem) -> String {
    format!(
        "{}::{:?}::{:?}::{:?}",
        clip.actual_resource_id, clip.resource_kind, clip.media_type, clip.frame_render_mode
    )
}

fn frame_clip_resource_id(frame_dir: &FrameDirectory) -> String {
    if !frame_dir.conversion_id.trim().is_empty() {
        frame_dir.conversion_id.clone()
    } else {
        frame_dir.directory_path.clone()
    }
}

fn hydrate_persisted_clip(
    clip: &PersistedTimelineClip,
    source_files: &[SourceContent],
    frame_directories: &[FrameDirectory],
    video_cuts: &[VideoCut],
    previews: &[Preview],
) -> Option<TimelineClipItem> {
    match clip.resource_kind {
        TimelineResourceKind::Source => {
            let source = source_files
                .iter()
                .find(|source| source.id == clip.actual_resource_id)?;
            Some(TimelineClipItem {
                clip_id: clip.clip_id.clone(),
                name: source
                    .custom_name
                    .clone()
                    .unwrap_or_else(|| file_name_from_path(&source.file_path)),
                media_type: TimelineMediaType::Video,
                resource_kind: TimelineResourceKind::Source,
                actual_resource_id: source.id.clone(),
                frame_render_mode: None,
                length_seconds: clip.length_seconds.max(0.0),
            })
        }
        TimelineResourceKind::Cut => {
            let cut = video_cuts
                .iter()
                .find(|cut| cut.id == clip.actual_resource_id)?;
            Some(TimelineClipItem {
                clip_id: clip.clip_id.clone(),
                name: cut
                    .custom_name
                    .clone()
                    .unwrap_or_else(|| file_name_from_path(&cut.file_path)),
                media_type: TimelineMediaType::Video,
                resource_kind: TimelineResourceKind::Cut,
                actual_resource_id: cut.id.clone(),
                frame_render_mode: None,
                length_seconds: if clip.length_seconds > 0.0 {
                    clip.length_seconds
                } else {
                    cut.duration.max(0.01)
                },
            })
        }
        TimelineResourceKind::AsciiConversion => {
            let frame_dir = frame_directories.iter().find(|frame_dir| {
                frame_clip_resource_id(frame_dir) == clip.actual_resource_id
                    || frame_dir.directory_path == clip.actual_resource_id
            })?;
            let metadata = frame_asset_metadata_from_directory(frame_dir);
            let frame_render_mode = clip
                .frame_render_mode
                .clone()
                .filter(|mode| supported_frame_render_modes(&metadata).contains(mode))
                .or_else(|| default_frame_render_mode(&metadata));
            Some(TimelineClipItem {
                clip_id: clip.clip_id.clone(),
                name: frame_dir.name.clone(),
                media_type: TimelineMediaType::Frames,
                resource_kind: TimelineResourceKind::AsciiConversion,
                actual_resource_id: frame_clip_resource_id(frame_dir),
                frame_render_mode,
                length_seconds: if clip.length_seconds > 0.0 {
                    clip.length_seconds
                } else {
                    frame_length_seconds(&metadata, &TimelineMediaType::Frames)
                },
            })
        }
        TimelineResourceKind::Preview => {
            let preview = previews
                .iter()
                .find(|preview| preview.id == clip.actual_resource_id)?;
            let metadata = frame_asset_metadata_from_preview(preview);
            let frame_render_mode = clip
                .frame_render_mode
                .clone()
                .filter(|mode| supported_frame_render_modes(&metadata).contains(mode))
                .or_else(|| default_frame_render_mode(&metadata));
            Some(TimelineClipItem {
                clip_id: clip.clip_id.clone(),
                name: preview
                    .custom_name
                    .clone()
                    .unwrap_or_else(|| preview.folder_name.clone()),
                media_type: TimelineMediaType::Frame,
                resource_kind: TimelineResourceKind::Preview,
                actual_resource_id: preview.id.clone(),
                frame_render_mode,
                length_seconds: if clip.length_seconds > 0.0 {
                    clip.length_seconds
                } else {
                    frame_length_seconds(&metadata, &TimelineMediaType::Frame)
                },
            })
        }
    }
}

fn reconcile_timeline_clip(
    clip: &TimelineClipItem,
    source_files: &[SourceContent],
    frame_directories: &[FrameDirectory],
    video_cuts: &[VideoCut],
    previews: &[Preview],
) -> Option<TimelineClipItem> {
    let clip_snapshot = PersistedTimelineClip {
        clip_id: clip.clip_id.clone(),
        project_id: String::new(),
        timeline_id: String::new(),
        order_index: 0,
        media_type: clip.media_type.clone(),
        resource_kind: clip.resource_kind.clone(),
        actual_resource_id: clip.actual_resource_id.clone(),
        frame_render_mode: clip.frame_render_mode.clone(),
        length_seconds: clip.length_seconds,
        creation_date: String::new(),
        last_updated: String::new(),
    };
    hydrate_persisted_clip(
        &clip_snapshot,
        source_files,
        frame_directories,
        video_cuts,
        previews,
    )
}

fn build_source_clip(source: &SourceContent) -> TimelineClipItem {
    TimelineClipItem {
        clip_id: make_unique_clip_id(&source.id),
        name: source
            .custom_name
            .clone()
            .unwrap_or_else(|| file_name_from_path(&source.file_path)),
        media_type: TimelineMediaType::Video,
        resource_kind: TimelineResourceKind::Source,
        actual_resource_id: source.id.clone(),
        frame_render_mode: None,
        length_seconds: 0.0,
    }
}

fn build_cut_clip(cut: &VideoCut) -> TimelineClipItem {
    TimelineClipItem {
        clip_id: make_unique_clip_id(&cut.id),
        name: cut
            .custom_name
            .clone()
            .unwrap_or_else(|| file_name_from_path(&cut.file_path)),
        media_type: TimelineMediaType::Video,
        resource_kind: TimelineResourceKind::Cut,
        actual_resource_id: cut.id.clone(),
        frame_render_mode: None,
        length_seconds: cut.duration.max(0.01),
    }
}

fn build_frame_directory_clip(frame_dir: &FrameDirectory) -> TimelineClipItem {
    let metadata = frame_asset_metadata_from_directory(frame_dir);
    TimelineClipItem {
        clip_id: make_unique_clip_id(&frame_clip_resource_id(frame_dir)),
        name: frame_dir.name.clone(),
        media_type: TimelineMediaType::Frames,
        resource_kind: TimelineResourceKind::AsciiConversion,
        actual_resource_id: frame_clip_resource_id(frame_dir),
        frame_render_mode: default_frame_render_mode(&metadata),
        length_seconds: frame_length_seconds(&metadata, &TimelineMediaType::Frames),
    }
}

fn build_preview_clip(preview: &Preview) -> TimelineClipItem {
    let metadata = frame_asset_metadata_from_preview(preview);
    TimelineClipItem {
        clip_id: make_unique_clip_id(&preview.id),
        name: preview
            .custom_name
            .clone()
            .unwrap_or_else(|| preview.folder_name.clone()),
        media_type: TimelineMediaType::Frame,
        resource_kind: TimelineResourceKind::Preview,
        actual_resource_id: preview.id.clone(),
        frame_render_mode: default_frame_render_mode(&metadata),
        length_seconds: frame_length_seconds(&metadata, &TimelineMediaType::Frame),
    }
}

fn bw_frame_mode_icon() -> Html {
    Html::from_html_unchecked(AttrValue::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 16 2.536-7.328a1.02 1.02 0 1 1 1.928 0L22 16"/><path d="M15.697 14h5.606"/><path d="m2 16 4.039-9.69a.5.5 0 0 1 .923 0L11 16"/><path d="M3.304 13h6.392"/></svg>"#,
    ))
}

fn frame_mode_icon(mode: Option<&FrameRenderMode>) -> Html {
    match mode {
        Some(FrameRenderMode::BwText) => bw_frame_mode_icon(),
        Some(FrameRenderMode::StyledText) => {
            html! { <Icon icon_id={IconId::LucideBrush} width={"14"} height={"14"} /> }
        }
        Some(FrameRenderMode::ColorFrames) => {
            html! { <Icon icon_id={IconId::LucideBrush} width={"14"} height={"14"} /> }
        }
        None => html! { <span>{"--"}</span> },
    }
}

fn dom_id_fragment(value: &str) -> String {
    let mut fragment = String::with_capacity(value.len());
    let mut last_was_dash = false;

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            fragment.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            fragment.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = fragment.trim_matches('-');
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed.to_string()
    }
}

fn make_unique_clip_id(original_id: &str) -> String {
    let ts = js_sys::Date::now();
    let rand = (js_sys::Math::random() * 1_000_000_000_f64).floor() as u32;
    format!("timeline-{}-{}-{}", original_id, ts, rand)
}

#[derive(Properties, PartialEq)]
pub struct MontagePageProps {
    pub project_id: String,
    pub on_project_name_change: Callback<String>,
    pub explorer_on_left: bool,
    #[prop_or_default]
    pub on_navigate: Option<Callback<&'static str>>,
}

#[function_component(MontagePage)]
pub fn montage_page(props: &MontagePageProps) -> Html {
    let cached_sidebar_data =
        get_project_sidebar_cache(&props.project_id).filter(|data| data.project.is_some());
    let cached_project = cached_sidebar_data
        .as_ref()
        .and_then(|data| data.project.clone());
    let cached_source_files = cached_sidebar_data
        .as_ref()
        .map(|data| data.source_files.clone())
        .unwrap_or_default();
    let cached_frame_directories = cached_sidebar_data
        .as_ref()
        .map(|data| data.frame_directories.clone())
        .unwrap_or_default();
    let cached_video_cuts = cached_sidebar_data
        .as_ref()
        .map(|data| data.video_cuts.clone())
        .unwrap_or_default();
    let cached_previews = cached_sidebar_data
        .as_ref()
        .map(|data| data.previews.clone())
        .unwrap_or_default();
    let cached_sidebar_state = cached_sidebar_data
        .as_ref()
        .map(|data| data.sidebar_state.clone())
        .unwrap_or_default();
    let cached_explorer_layout = cached_sidebar_data
        .as_ref()
        .map(|data| data.explorer_layout.clone())
        .unwrap_or_else(|| ExplorerLayout {
            project_id: props.project_id.clone(),
            root_items: Vec::new(),
        });

    let project = use_state(move || cached_project.clone());
    let source_files = use_state(move || cached_source_files.clone());
    let frame_directories = use_state(move || cached_frame_directories.clone());
    let video_cuts = use_state(move || cached_video_cuts.clone());
    let previews = use_state(move || cached_previews.clone());
    let error_message = use_state(|| Option::<String>::None);
    let selected_source = use_state(|| None::<SourceContent>);
    let selected_frame_dir = use_state(|| None::<FrameDirectory>);
    let controls_collapsed = use_state(|| false);
    let is_playing = use_state(|| false);
    let should_reset = use_state(|| false);
    let synced_progress = use_state(|| 0.0f64);
    let seek_percentage = use_state(|| None::<f64>);
    let viewer_loading = use_state(|| false);
    let loop_enabled = use_state(|| true);
    let video_volume = use_state(|| 1.0f64);
    let video_is_muted = use_state(|| false);
    let resources_loaded = use_state(|| cached_sidebar_data.is_some());

    // Explorer sidebar state
    let sidebar_state = use_state(move || cached_sidebar_state.clone());
    let explorer_layout = use_state(move || cached_explorer_layout.clone());
    let selected_node_id = use_state(|| None::<TreeNodeId>);

    // Timeline state
    let timeline_items = use_state(|| Vec::<TimelineClipItem>::new());
    let timeline_id = use_state(|| None::<String>);
    let persisted_timeline = use_state(|| None::<PersistedProjectTimeline>);
    let timeline_loaded = use_state(|| false);
    let timeline_initialized = use_state(|| false);
    let clip_preloads = use_state(HashMap::<String, ClipPreloadState>::new);

    // Playback orchestration state
    let active_timeline_index = use_state(|| None::<usize>);
    let active_playable = use_state(|| None::<PlayableItem>);
    let url_cache = use_state(|| HashMap::<String, String>::new());
    let preload_generation = use_mut_ref(|| 0u64);

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

    let resolve_and_activate = {
        let active_playable = active_playable.clone();
        let active_timeline_index = active_timeline_index.clone();
        let is_playing = is_playing.clone();
        let timeline_items = timeline_items.clone();
        let loop_enabled = loop_enabled.clone();
        Rc::new(move |index: usize| {
            let items = (*timeline_items).clone();
            if items.get(index).is_some() {
                active_timeline_index.set(Some(index));
                return;
            }

            if *loop_enabled && !items.is_empty() {
                active_timeline_index.set(Some(0));
            } else {
                is_playing.set(false);
                active_timeline_index.set(None);
                active_playable.set(None);
            }
        })
    };

    {
        let active_timeline_index = *active_timeline_index;
        let timeline_items = (*timeline_items).clone();
        let clip_preloads = (*clip_preloads).clone();
        let active_playable = active_playable.clone();

        use_effect_with(
            (active_timeline_index, timeline_items, clip_preloads),
            move |(active_timeline_index, timeline_items, clip_preloads)| {
                if let Some(index) = active_timeline_index {
                    if let Some(item) = timeline_items.get(*index) {
                        if let Some(preload) = clip_preloads.get(&item.clip_id) {
                            if preload.status == PreloadStatus::Ready {
                                match item.media_type {
                                    TimelineMediaType::Video => {
                                        if let Some(asset_url) = preload.video_asset_url.clone() {
                                            active_playable.set(Some(PlayableItem::Video {
                                                clip_id: item.clip_id.clone(),
                                                asset_url,
                                            }));
                                        } else {
                                            active_playable.set(None);
                                        }
                                    }
                                    TimelineMediaType::Frames | TimelineMediaType::Frame => {
                                        if let Some(preloaded_bundle) = preload.frame_bundle.clone()
                                        {
                                            active_playable.set(Some(PlayableItem::Frames {
                                                clip_id: item.clip_id.clone(),
                                                directory_path: preloaded_bundle
                                                    .directory_path
                                                    .clone(),
                                                fps: preload.playback_fps.unwrap_or(24),
                                                frame_render_mode: item
                                                    .frame_render_mode
                                                    .clone()
                                                    .unwrap_or(FrameRenderMode::BwText),
                                                preloaded_bundle,
                                            }));
                                        } else {
                                            active_playable.set(None);
                                        }
                                    }
                                }
                            } else {
                                active_playable.set(None);
                            }
                        } else {
                            active_playable.set(None);
                        }
                    } else {
                        active_playable.set(None);
                    }
                } else {
                    active_playable.set(None);
                }

                || ()
            },
        );
    }

    // on_item_ended: advance to next timeline item
    let on_item_ended = {
        let resolve_and_activate = resolve_and_activate.clone();
        let active_timeline_index = active_timeline_index.clone();
        let timeline_items = timeline_items.clone();
        let loop_enabled = loop_enabled.clone();
        let is_playing = is_playing.clone();
        let active_playable = active_playable.clone();
        Callback::from(move |_: ()| {
            let current = (*active_timeline_index).unwrap_or(0);
            let next = current + 1;
            let total = timeline_items.len();
            if next < total {
                resolve_and_activate(next);
            } else if *loop_enabled && total > 0 {
                // Clear active_playable so the component remounts from scratch
                active_playable.set(None);
                active_timeline_index.set(Some(0));
                // Will be resolved by the effect above
            } else {
                is_playing.set(false);
                active_timeline_index.set(None);
                active_playable.set(None);
            }
        })
    };

    // on_item_progress: update global progress across whole timeline
    let on_item_progress = {
        let active_timeline_index = active_timeline_index.clone();
        let timeline_items = timeline_items.clone();
        let synced_progress = synced_progress.clone();
        let is_playing = is_playing.clone();
        Callback::from(move |local_progress: f64| {
            if !*is_playing {
                return;
            }
            let total = timeline_items.len();
            if total == 0 {
                return;
            }
            let current = (*active_timeline_index).unwrap_or(0);
            let global = (current as f64 + local_progress) / total as f64 * 100.0;
            synced_progress.set(global.clamp(0.0, 100.0));
        })
    };

    // Handle play/pause: when play is pressed and no active item, start from 0
    {
        let playing = *is_playing;
        let active_timeline_index = active_timeline_index.clone();
        let resolve_and_activate = resolve_and_activate.clone();
        let timeline_items = timeline_items.clone();
        let prev_playing = use_mut_ref(|| false);
        use_effect_with(playing, move |playing| {
            let was_playing = *prev_playing.borrow();
            *prev_playing.borrow_mut() = *playing;
            if *playing && !was_playing {
                // Starting playback
                if (*active_timeline_index).is_none() && !timeline_items.is_empty() {
                    resolve_and_activate(0);
                }
            }
            || ()
        });
    }

    // Handle reset
    {
        let should_reset_val = *should_reset;
        let active_timeline_index = active_timeline_index.clone();
        let active_playable = active_playable.clone();
        let synced_progress = synced_progress.clone();
        let prev_reset = use_mut_ref(|| false);
        use_effect_with(should_reset_val, move |reset| {
            let was_reset = *prev_reset.borrow();
            *prev_reset.borrow_mut() = *reset;
            if *reset && !was_reset {
                active_timeline_index.set(None);
                active_playable.set(None);
                synced_progress.set(0.0);
            }
            || ()
        });
    }

    // Handle global seek from progress slider
    {
        let seek_val = *seek_percentage;
        let active_timeline_index = active_timeline_index.clone();
        let resolve_and_activate = resolve_and_activate.clone();
        let timeline_items = timeline_items.clone();
        let seek_percentage = seek_percentage.clone();
        let prev_seek = use_mut_ref(|| None::<f64>);
        use_effect_with(seek_val, move |seek| {
            if let Some(pct) = seek {
                let prev = *prev_seek.borrow();
                if prev != Some(*pct) {
                    *prev_seek.borrow_mut() = Some(*pct);
                    let total = timeline_items.len();
                    if total > 0 {
                        let scaled = *pct * total as f64;
                        let index = (scaled.floor() as usize).min(total - 1);
                        let current_active = *active_timeline_index;
                        if current_active != Some(index) {
                            resolve_and_activate(index);
                        }
                        // The local seek within the item will be handled by the component's own seek_percentage
                        // We pass the fractional part as the local seek
                        let local_seek = scaled - index as f64;
                        seek_percentage.set(Some(local_seek.clamp(0.0, 1.0)));
                    }
                }
            } else {
                *prev_seek.borrow_mut() = None;
            }
            || ()
        });
    }

    // Load project details and data
    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let on_project_name_change = props.on_project_name_change.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
        let error_message = error_message.clone();
        let sidebar_state = sidebar_state.clone();
        let resources_loaded = resources_loaded.clone();
        let explorer_layout = explorer_layout.clone();

        use_effect_with(project_id.clone(), move |id| {
            resources_loaded.set(false);
            if let Some(cached_data) =
                get_project_sidebar_cache(id).filter(|data| data.project.is_some())
            {
                let cached_project = cached_data.project.clone();
                if *project != cached_project {
                    project.set(cached_project.clone());
                }
                if *source_files != cached_data.source_files {
                    source_files.set(cached_data.source_files.clone());
                }
                if *frame_directories != cached_data.frame_directories {
                    frame_directories.set(cached_data.frame_directories.clone());
                }
                if *video_cuts != cached_data.video_cuts {
                    video_cuts.set(cached_data.video_cuts.clone());
                }
                if *previews != cached_data.previews {
                    previews.set(cached_data.previews.clone());
                }
                if *sidebar_state != cached_data.sidebar_state {
                    sidebar_state.set(cached_data.sidebar_state.clone());
                }
                if *explorer_layout != cached_data.explorer_layout {
                    explorer_layout.set(cached_data.explorer_layout.clone());
                }
                if let Some(cached_project) = cached_project {
                    on_project_name_change.emit(cached_project.project_name);
                }
                resources_loaded.set(true);
            } else {
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
                                error_message
                                    .set(Some("Failed to fetch project details.".to_string()));
                            }
                        }
                    }

                    // Fetch source files
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    let fetched_sources = if let Ok(sources) =
                        serde_wasm_bindgen::from_value::<Vec<SourceContent>>(
                            tauri_invoke("get_project_sources", args).await,
                        ) {
                        source_files.set(sources.clone());
                        sources
                    } else {
                        Vec::new()
                    };

                    // Fetch frame directories
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    let fetched_frames = if let Ok(frames) =
                        serde_wasm_bindgen::from_value::<Vec<FrameDirectory>>(
                            tauri_invoke("get_project_frames", args).await,
                        ) {
                        frame_directories.set(frames.clone());
                        frames
                    } else {
                        Vec::new()
                    };

                    // Fetch video cuts
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    let fetched_cuts = if let Ok(cuts) =
                        serde_wasm_bindgen::from_value::<Vec<VideoCut>>(
                            tauri_invoke("get_project_cuts", args).await,
                        ) {
                        video_cuts.set(cuts.clone());
                        cuts
                    } else {
                        Vec::new()
                    };

                    // Fetch previews
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    let fetched_previews = if let Ok(p) =
                        serde_wasm_bindgen::from_value::<Vec<Preview>>(
                            tauri_invoke("get_project_previews", args).await,
                        ) {
                        previews.set(p.clone());
                        p
                    } else {
                        Vec::new()
                    };

                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    let fetched_project_content =
                        serde_wasm_bindgen::from_value::<
                            Vec<crate::components::explorer::ProjectContentEntry>,
                        >(tauri_invoke("get_project_content", args).await)
                        .unwrap_or_else(|_| Vec::new());
                    explorer_layout.set(hydrate_layout_from_project_content(
                        &id,
                        &fetched_project_content,
                        &fetched_sources,
                        &fetched_cuts,
                        &fetched_frames,
                        &fetched_previews,
                    ));
                    resources_loaded.set(true);
                });
            }

            || ()
        });
    }

    {
        let project_id = props.project_id.clone();
        let project = (*project).clone();
        let source_files = (*source_files).clone();
        let frame_directories = (*frame_directories).clone();
        let video_cuts = (*video_cuts).clone();
        let previews = (*previews).clone();
        let sidebar_state = (*sidebar_state).clone();
        let explorer_layout = (*explorer_layout).clone();
        use_effect_with(
            (
                project_id,
                project,
                source_files,
                frame_directories,
                video_cuts,
                previews,
                sidebar_state,
                explorer_layout,
            ),
            move |(
                project_id,
                project,
                source_files,
                frame_directories,
                video_cuts,
                previews,
                sidebar_state,
                explorer_layout,
            )| {
                set_project_sidebar_cache(
                    project_id,
                    ProjectSidebarCache {
                        project: project.clone(),
                        source_files: source_files.clone(),
                        frame_directories: frame_directories.clone(),
                        video_cuts: video_cuts.clone(),
                        previews: previews.clone(),
                        sidebar_state: sidebar_state.clone(),
                        explorer_layout: explorer_layout.clone(),
                    },
                );
                || ()
            },
        );
    }

    {
        let project_id = props.project_id.clone();
        let persisted_timeline = persisted_timeline.clone();
        let timeline_id = timeline_id.clone();
        let timeline_loaded = timeline_loaded.clone();
        let timeline_initialized = timeline_initialized.clone();
        let timeline_items = timeline_items.clone();
        let active_timeline_index = active_timeline_index.clone();
        let active_playable = active_playable.clone();
        let clip_preloads = clip_preloads.clone();

        use_effect_with(project_id.clone(), move |project_id| {
            timeline_loaded.set(false);
            timeline_initialized.set(false);
            persisted_timeline.set(None);
            timeline_id.set(None);
            timeline_items.set(Vec::new());
            active_timeline_index.set(None);
            active_playable.set(None);
            clip_preloads.set(HashMap::new());

            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                let loaded_timeline = serde_wasm_bindgen::from_value::<PersistedProjectTimeline>(
                    tauri_invoke("get_active_project_timeline", args).await,
                )
                .unwrap_or(PersistedProjectTimeline {
                    timeline: None,
                    clips: Vec::new(),
                });

                timeline_id.set(
                    loaded_timeline
                        .timeline
                        .as_ref()
                        .map(|timeline| timeline.timeline_id.clone()),
                );
                persisted_timeline.set(Some(loaded_timeline));
                timeline_loaded.set(true);
            });

            || ()
        });
    }

    {
        let timeline_loaded_value = *timeline_loaded;
        let resources_loaded_value = *resources_loaded;
        let timeline_initialized_value = *timeline_initialized;
        let persisted_timeline_value = (*persisted_timeline).clone();
        let timeline_items = timeline_items.clone();
        let timeline_initialized = timeline_initialized.clone();
        let source_files = (*source_files).clone();
        let frame_directories = (*frame_directories).clone();
        let video_cuts = (*video_cuts).clone();
        let previews = (*previews).clone();

        use_effect_with(
            (
                timeline_loaded_value,
                resources_loaded_value,
                timeline_initialized_value,
                persisted_timeline_value,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            ),
            move |(
                timeline_loaded_value,
                resources_loaded_value,
                timeline_initialized_value,
                persisted_timeline_value,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            )| {
                if *timeline_loaded_value && *resources_loaded_value && !*timeline_initialized_value
                {
                    let hydrated_items = persisted_timeline_value
                        .as_ref()
                        .map(|timeline| {
                            timeline
                                .clips
                                .iter()
                                .filter_map(|clip| {
                                    hydrate_persisted_clip(
                                        clip,
                                        source_files,
                                        frame_directories,
                                        video_cuts,
                                        previews,
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    timeline_items.set(hydrated_items);
                    timeline_initialized.set(true);
                }
                || ()
            },
        );
    }

    {
        let timeline_initialized_value = *timeline_initialized;
        let source_files = (*source_files).clone();
        let frame_directories = (*frame_directories).clone();
        let video_cuts = (*video_cuts).clone();
        let previews = (*previews).clone();
        let timeline_items = timeline_items.clone();
        let active_timeline_index = active_timeline_index.clone();
        let active_playable = active_playable.clone();

        use_effect_with(
            (
                timeline_initialized_value,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            ),
            move |(
                timeline_initialized_value,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            )| {
                if *timeline_initialized_value {
                    let current_items = (*timeline_items).clone();
                    let reconciled_items = current_items
                        .iter()
                        .filter_map(|clip| {
                            reconcile_timeline_clip(
                                clip,
                                source_files,
                                frame_directories,
                                video_cuts,
                                previews,
                            )
                        })
                        .collect::<Vec<_>>();

                    if reconciled_items != current_items {
                        let reconciled_len = reconciled_items.len();
                        timeline_items.set(reconciled_items);
                        if reconciled_len == 0 {
                            active_timeline_index.set(None);
                            active_playable.set(None);
                        } else if let Some(active_index) = *active_timeline_index {
                            if active_index >= reconciled_len {
                                active_timeline_index.set(Some(reconciled_len - 1));
                            }
                        }
                    }
                }

                || ()
            },
        );
    }

    {
        let timeline_initialized_value = *timeline_initialized;
        let project_id = props.project_id.clone();
        let timeline_items_snapshot = (*timeline_items).clone();
        let timeline_id = timeline_id.clone();

        use_effect_with(
            (
                timeline_initialized_value,
                project_id.clone(),
                timeline_items_snapshot,
            ),
            move |(timeline_initialized_value, project_id, timeline_items_snapshot)| {
                if *timeline_initialized_value {
                    let current_timeline_id = (*timeline_id).clone();
                    if !(timeline_items_snapshot.is_empty() && current_timeline_id.is_none()) {
                        let clips = timeline_items_snapshot
                            .iter()
                            .map(|clip| SaveTimelineClip {
                                clip_id: clip.clip_id.clone(),
                                media_type: clip.media_type.clone(),
                                resource_kind: clip.resource_kind.clone(),
                                actual_resource_id: clip.actual_resource_id.clone(),
                                frame_render_mode: clip.frame_render_mode.clone(),
                                length_seconds: clip.length_seconds,
                            })
                            .collect::<Vec<_>>();

                        let project_id = project_id.clone();
                        let timeline_id = timeline_id.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            let args = serde_wasm_bindgen::to_value(&json!({
                                "request": {
                                    "project_id": project_id,
                                    "timeline_id": current_timeline_id,
                                    "clips": clips,
                                }
                            }))
                            .unwrap();

                            if let Ok(saved_timeline) =
                                serde_wasm_bindgen::from_value::<PersistedProjectTimeline>(
                                    tauri_invoke("save_project_timeline", args).await,
                                )
                            {
                                timeline_id.set(
                                    saved_timeline
                                        .timeline
                                        .as_ref()
                                        .map(|timeline| timeline.timeline_id.clone()),
                                );
                            }
                        });
                    }
                }

                || ()
            },
        );
    }

    // Add item to timeline helper
    let add_to_timeline = {
        let timeline_items = timeline_items.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
        Rc::new(
            move |item_type: &str, id: String, name: String, insert_at: Option<usize>| {
                let new_clip = match item_type {
                    "source" => (*source_files)
                        .iter()
                        .find(|source| source.id == id)
                        .map(build_source_clip),
                    "frame" => (*frame_directories)
                        .iter()
                        .find(|frame_dir| {
                            frame_clip_resource_id(frame_dir) == id
                                || frame_dir.directory_path == id
                        })
                        .map(build_frame_directory_clip)
                        .or_else(|| {
                            (*previews)
                                .iter()
                                .find(|preview| preview.id == id)
                                .map(build_preview_clip)
                        }),
                    "cut" => (*video_cuts)
                        .iter()
                        .find(|cut| cut.id == id)
                        .map(build_cut_clip),
                    _ => None,
                };

                let Some(mut new_clip) = new_clip else {
                    web_sys::console::log_1(
                        &format!("Unknown or missing item for timeline add: {}", item_type).into(),
                    );
                    return;
                };

                if !name.trim().is_empty() {
                    new_clip.name = name;
                }

                let mut items = (*timeline_items).clone();
                if let Some(index) = insert_at {
                    if index <= items.len() {
                        items.insert(index, new_clip);
                    } else {
                        items.push(new_clip);
                    }
                } else {
                    items.push(new_clip);
                }
                timeline_items.set(items);
            },
        )
    };

    // Use a ref to always have the latest timeline_items available for the event listener
    let timeline_items_ref = use_mut_ref(|| Vec::<TimelineClipItem>::new());
    let clip_preloads_ref = use_mut_ref(HashMap::<String, ClipPreloadState>::new);
    // Keep the ref in sync with state on each render
    *timeline_items_ref.borrow_mut() = (*timeline_items).clone();
    *clip_preloads_ref.borrow_mut() = (*clip_preloads).clone();

    {
        let timeline_initialized_value = *timeline_initialized;
        let resources_loaded_value = *resources_loaded;
        let timeline_items_snapshot = (*timeline_items).clone();
        let source_files = (*source_files).clone();
        let frame_directories = (*frame_directories).clone();
        let video_cuts = (*video_cuts).clone();
        let previews = (*previews).clone();
        let clip_preloads = clip_preloads.clone();
        let clip_preloads_ref = clip_preloads_ref.clone();
        let preload_generation = preload_generation.clone();
        let url_cache = url_cache.clone();
        let timeline_items = timeline_items.clone();
        let timeline_items_ref = timeline_items_ref.clone();

        use_effect_with(
            (
                timeline_initialized_value,
                resources_loaded_value,
                timeline_items_snapshot,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            ),
            move |(
                timeline_initialized_value,
                resources_loaded_value,
                timeline_items_snapshot,
                source_files,
                frame_directories,
                video_cuts,
                previews,
            )| {
                if *timeline_initialized_value && *resources_loaded_value {
                    let generation = preload_generation.borrow().wrapping_add(1);
                    *preload_generation.borrow_mut() = generation;

                    let existing_states = (*clip_preloads).clone();
                    let mut next_states = HashMap::new();
                    for clip in timeline_items_snapshot.iter() {
                        let signature = make_clip_signature(clip);
                        if let Some(existing_state) = existing_states.get(&clip.clip_id) {
                            if existing_state.signature == signature
                                && existing_state.status == PreloadStatus::Ready
                            {
                                next_states.insert(clip.clip_id.clone(), existing_state.clone());
                                continue;
                            }
                        }

                        next_states.insert(
                            clip.clip_id.clone(),
                            ClipPreloadState {
                                signature,
                                status: PreloadStatus::Loading,
                                video_asset_url: None,
                                frame_bundle: None,
                                playback_fps: None,
                                error: None,
                            },
                        );
                    }
                    clip_preloads.set(next_states.clone());

                    if !timeline_items_snapshot.is_empty() {
                        let clip_preloads = clip_preloads.clone();
                        let clip_preloads_ref = clip_preloads_ref.clone();
                        let url_cache = url_cache.clone();
                        let timeline_items = timeline_items.clone();
                        let timeline_items_ref = timeline_items_ref.clone();
                        let source_files = source_files.clone();
                        let frame_directories = frame_directories.clone();
                        let video_cuts = video_cuts.clone();
                        let previews = previews.clone();
                        let preload_generation_ref = preload_generation.clone();
                        let clips_to_load = timeline_items_snapshot.clone();

                        wasm_bindgen_futures::spawn_local(async move {
                            for clip in clips_to_load {
                                if *preload_generation_ref.borrow() != generation {
                                    return;
                                }

                                let signature = make_clip_signature(&clip);
                                let existing_state =
                                    clip_preloads_ref.borrow().get(&clip.clip_id).cloned();
                                if matches!(
                                    existing_state,
                                    Some(ClipPreloadState {
                                        status: PreloadStatus::Ready,
                                        signature: ref existing_signature,
                                        ..
                                    }) if *existing_signature == signature
                                ) {
                                    continue;
                                }

                                let preload_result: Result<
                                    (ClipPreloadState, Option<f64>),
                                    String,
                                > = match clip.resource_kind {
                                    TimelineResourceKind::Source => {
                                        if let Some(source) = source_files
                                            .iter()
                                            .find(|source| source.id == clip.actual_resource_id)
                                            .cloned()
                                        {
                                            let asset_url_result = if let Some(cached_url) =
                                                url_cache.get(&source.file_path)
                                            {
                                                Ok(cached_url.clone())
                                            } else {
                                                let args = serde_wasm_bindgen::to_value(&json!({
                                                    "path": source.file_path
                                                }))
                                                .unwrap();
                                                match serde_wasm_bindgen::from_value::<PreparedMedia>(
                                                    tauri_invoke("prepare_media", args).await,
                                                ) {
                                                    Ok(prepared) => {
                                                        let asset_url = app_convert_file_src(
                                                            &prepared.cached_abs_path,
                                                        );
                                                        let mut next_url_cache =
                                                            (*url_cache).clone();
                                                        next_url_cache.insert(
                                                            source.file_path.clone(),
                                                            asset_url.clone(),
                                                        );
                                                        url_cache.set(next_url_cache);
                                                        Ok(asset_url)
                                                    }
                                                    Err(error) => Err(format!(
                                                        "Failed to prepare source media: {:?}",
                                                        error
                                                    )),
                                                }
                                            };

                                            match asset_url_result {
                                                Ok(asset_url) => {
                                                    let duration =
                                                    serde_wasm_bindgen::from_value::<Option<f64>>(
                                                        warm_video_asset(&asset_url).await,
                                                    )
                                                    .unwrap_or(None);
                                                    Ok((
                                                        ClipPreloadState {
                                                            signature: signature.clone(),
                                                            status: PreloadStatus::Ready,
                                                            video_asset_url: Some(asset_url),
                                                            frame_bundle: None,
                                                            playback_fps: None,
                                                            error: None,
                                                        },
                                                        duration.filter(|duration| {
                                                            duration.is_finite() && *duration > 0.0
                                                        }),
                                                    ))
                                                }
                                                Err(error) => Err(error),
                                            }
                                        } else {
                                            Err("Source clip not found.".to_string())
                                        }
                                    }
                                    TimelineResourceKind::Cut => {
                                        if let Some(cut) = video_cuts
                                            .iter()
                                            .find(|cut| cut.id == clip.actual_resource_id)
                                            .cloned()
                                        {
                                            let asset_url_result = if let Some(cached_url) =
                                                url_cache.get(&cut.file_path)
                                            {
                                                Ok(cached_url.clone())
                                            } else {
                                                let args = serde_wasm_bindgen::to_value(&json!({
                                                    "path": cut.file_path
                                                }))
                                                .unwrap();
                                                match serde_wasm_bindgen::from_value::<PreparedMedia>(
                                                    tauri_invoke("prepare_media", args).await,
                                                ) {
                                                    Ok(prepared) => {
                                                        let asset_url = app_convert_file_src(
                                                            &prepared.cached_abs_path,
                                                        );
                                                        let mut next_url_cache =
                                                            (*url_cache).clone();
                                                        next_url_cache.insert(
                                                            cut.file_path.clone(),
                                                            asset_url.clone(),
                                                        );
                                                        url_cache.set(next_url_cache);
                                                        Ok(asset_url)
                                                    }
                                                    Err(error) => Err(format!(
                                                        "Failed to prepare cut media: {:?}",
                                                        error
                                                    )),
                                                }
                                            };

                                            match asset_url_result {
                                                Ok(asset_url) => {
                                                    let _ = warm_video_asset(&asset_url).await;
                                                    Ok((
                                                        ClipPreloadState {
                                                            signature: signature.clone(),
                                                            status: PreloadStatus::Ready,
                                                            video_asset_url: Some(asset_url),
                                                            frame_bundle: None,
                                                            playback_fps: None,
                                                            error: None,
                                                        },
                                                        Some(cut.duration.max(0.01)),
                                                    ))
                                                }
                                                Err(error) => Err(error),
                                            }
                                        } else {
                                            Err("Cut clip not found.".to_string())
                                        }
                                    }
                                    TimelineResourceKind::AsciiConversion => {
                                        if let Some(frame_dir) = frame_directories
                                            .iter()
                                            .find(|frame_dir| {
                                                frame_clip_resource_id(frame_dir)
                                                    == clip.actual_resource_id
                                                    || frame_dir.directory_path
                                                        == clip.actual_resource_id
                                            })
                                            .cloned()
                                        {
                                            let metadata =
                                                frame_asset_metadata_from_directory(&frame_dir);
                                            if let Some(render_mode) =
                                                clip.frame_render_mode.clone()
                                            {
                                                let bundle = Rc::new(
                                                    match preload_frame_bundle(
                                                        &metadata,
                                                        render_mode,
                                                    )
                                                    .await
                                                    {
                                                        Ok(bundle) => bundle,
                                                        Err(error) => {
                                                            let mut next_preloads =
                                                                clip_preloads_ref.borrow().clone();
                                                            next_preloads.insert(
                                                                clip.clip_id.clone(),
                                                                ClipPreloadState {
                                                                    signature: signature.clone(),
                                                                    status: PreloadStatus::Error,
                                                                    video_asset_url: None,
                                                                    frame_bundle: None,
                                                                    playback_fps: None,
                                                                    error: Some(error),
                                                                },
                                                            );
                                                            clip_preloads.set(next_preloads);
                                                            continue;
                                                        }
                                                    },
                                                );
                                                Ok((
                                                    ClipPreloadState {
                                                        signature: signature.clone(),
                                                        status: PreloadStatus::Ready,
                                                        video_asset_url: None,
                                                        frame_bundle: Some(bundle),
                                                        playback_fps: Some(
                                                            if metadata.frame_speed > 0 {
                                                                metadata.frame_speed
                                                            } else {
                                                                metadata.fps.max(1)
                                                            },
                                                        ),
                                                        error: None,
                                                    },
                                                    Some(frame_length_seconds(
                                                        &metadata,
                                                        &TimelineMediaType::Frames,
                                                    )),
                                                ))
                                            } else {
                                                Err("Missing frame render mode for frames clip."
                                                    .to_string())
                                            }
                                        } else {
                                            Err("Frames clip not found.".to_string())
                                        }
                                    }
                                    TimelineResourceKind::Preview => {
                                        if let Some(preview) = previews
                                            .iter()
                                            .find(|preview| preview.id == clip.actual_resource_id)
                                            .cloned()
                                        {
                                            let metadata =
                                                frame_asset_metadata_from_preview(&preview);
                                            if let Some(render_mode) =
                                                clip.frame_render_mode.clone()
                                            {
                                                let bundle = Rc::new(
                                                    match preload_frame_bundle(
                                                        &metadata,
                                                        render_mode,
                                                    )
                                                    .await
                                                    {
                                                        Ok(bundle) => bundle,
                                                        Err(error) => {
                                                            let mut next_preloads =
                                                                clip_preloads_ref.borrow().clone();
                                                            next_preloads.insert(
                                                                clip.clip_id.clone(),
                                                                ClipPreloadState {
                                                                    signature: signature.clone(),
                                                                    status: PreloadStatus::Error,
                                                                    video_asset_url: None,
                                                                    frame_bundle: None,
                                                                    playback_fps: None,
                                                                    error: Some(error),
                                                                },
                                                            );
                                                            clip_preloads.set(next_preloads);
                                                            continue;
                                                        }
                                                    },
                                                );
                                                Ok((
                                                    ClipPreloadState {
                                                        signature: signature.clone(),
                                                        status: PreloadStatus::Ready,
                                                        video_asset_url: None,
                                                        frame_bundle: Some(bundle),
                                                        playback_fps: Some(metadata.fps.max(1)),
                                                        error: None,
                                                    },
                                                    Some(frame_length_seconds(
                                                        &metadata,
                                                        &TimelineMediaType::Frame,
                                                    )),
                                                ))
                                            } else {
                                                Err("Missing frame render mode for preview clip."
                                                    .to_string())
                                            }
                                        } else {
                                            Err("Preview clip not found.".to_string())
                                        }
                                    }
                                };

                                let next_state = match preload_result {
                                    Ok((state, maybe_duration)) => {
                                        if let Some(duration) = maybe_duration {
                                            let mut items = timeline_items_ref.borrow().clone();
                                            if let Some(item) = items
                                                .iter_mut()
                                                .find(|item| item.clip_id == clip.clip_id)
                                            {
                                                if (item.length_seconds - duration).abs() > 0.01 {
                                                    item.length_seconds = duration;
                                                    timeline_items.set(items);
                                                }
                                            }
                                        }
                                        state
                                    }
                                    Err(error) => ClipPreloadState {
                                        signature: signature.clone(),
                                        status: PreloadStatus::Error,
                                        video_asset_url: None,
                                        frame_bundle: None,
                                        playback_fps: None,
                                        error: Some(error),
                                    },
                                };

                                if *preload_generation_ref.borrow() != generation {
                                    return;
                                }

                                let mut next_preloads = clip_preloads_ref.borrow().clone();
                                if let Some(existing_state) = next_preloads.get(&clip.clip_id) {
                                    if existing_state.signature != signature {
                                        continue;
                                    }
                                }
                                next_preloads.insert(clip.clip_id.clone(), next_state);
                                clip_preloads.set(next_preloads);
                            }
                        });
                    }
                }

                || ()
            },
        );
    }

    // Listen for pointer-based drops coming from JS and apply them to timeline state
    {
        let timeline_items = timeline_items.clone();
        let timeline_items_ref = timeline_items_ref.clone();
        let add_to_timeline = add_to_timeline.clone();
        use_effect_with((), move |_| {
            let timeline_items = timeline_items.clone();
            let timeline_items_ref = timeline_items_ref.clone();
            let add_to_timeline = add_to_timeline.clone();
            let window = web_sys::window().expect("window exists");
            let listener = EventListener::new(&window, "cascii:timeline-drop", move |_| {
                web_sys::console::log_1(&"=== Rust received cascii:timeline-drop ===".into());
                let target_index = get_drop_target_index();
                web_sys::console::log_1(&format!("Drop target index: {:?}", target_index).into());

                if let Some(data_str) = get_pending_drop() {
                    web_sys::console::log_1(&format!("Pending drop data: {}", data_str).into());
                    match serde_json::from_str::<DragData>(&data_str) {
                        Ok(drag_data) => {
                            web_sys::console::log_1(
                                &format!(
                                    "Parsed drag data - origin: {}, index: {:?}",
                                    drag_data.origin, drag_data.index
                                )
                                .into(),
                            );
                            let mut items = timeline_items_ref.borrow().clone();
                            web_sys::console::log_1(
                                &format!("Current items count: {}", items.len()).into(),
                            );

                            if drag_data.origin == "sidebar" {
                                add_to_timeline(
                                    &drag_data.item_type,
                                    drag_data.id,
                                    drag_data.name,
                                    target_index,
                                );
                            } else if drag_data.origin == "timeline" {
                                // Reordering existing timeline item
                                if let Some(from_index) = drag_data.index {
                                    if let Some(to_index) = target_index {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Moving item from {} to {}",
                                                from_index, to_index
                                            )
                                            .into(),
                                        );
                                        if from_index < items.len() {
                                            let item = items.remove(from_index);
                                            // Adjust target index after removal
                                            let adjusted_to = if to_index > from_index {
                                                (to_index - 1).min(items.len())
                                            } else {
                                                to_index.min(items.len())
                                            };
                                            items.insert(adjusted_to, item);
                                            timeline_items.set(items);
                                        }
                                    } else {
                                        web_sys::console::log_1(
                                            &"No target index for timeline reorder".into(),
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Failed to parse drag data: {:?}", e).into(),
                            );
                        }
                    }
                } else {
                    web_sys::console::log_1(&"No pending drop data".into());
                }
            });
            || drop(listener)
        });
    }

    // Remove item from timeline
    let on_remove_timeline_item = {
        let timeline_items = timeline_items.clone();
        let active_timeline_index = active_timeline_index.clone();
        let active_playable = active_playable.clone();
        let is_playing = is_playing.clone();
        Callback::from(move |index: usize| {
            let mut items = (*timeline_items).clone();
            if index < items.len() {
                items.remove(index);
                // Adjust active index if needed
                if let Some(active) = *active_timeline_index {
                    if index == active {
                        // Removed the active item — stop playback
                        is_playing.set(false);
                        active_timeline_index.set(None);
                        active_playable.set(None);
                    } else if index < active {
                        active_timeline_index.set(Some(active - 1));
                    }
                }
                timeline_items.set(items);
            }
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

    // Select callbacks — add selected item to timeline
    let on_select_source = {
        let add_to_timeline = add_to_timeline.clone();
        let selected_source = selected_source.clone();
        Callback::from(move |source: SourceContent| {
            selected_source.set(Some(source.clone()));
            let name = source
                .custom_name
                .clone()
                .unwrap_or_else(|| file_name_from_path(&source.file_path));
            add_to_timeline("source", source.id.clone(), name, None);
        })
    };

    let on_select_frame_dir = {
        let add_to_timeline = add_to_timeline.clone();
        let selected_frame_dir = selected_frame_dir.clone();
        Callback::from(move |frame_dir: FrameDirectory| {
            selected_frame_dir.set(Some(frame_dir.clone()));
            add_to_timeline(
                "frame",
                frame_clip_resource_id(&frame_dir),
                frame_dir.name.clone(),
                None,
            );
        })
    };

    let on_select_cut = {
        let add_to_timeline = add_to_timeline.clone();
        Callback::from(move |cut: VideoCut| {
            let name = cut
                .custom_name
                .clone()
                .unwrap_or_else(|| file_name_from_path(&cut.file_path));
            add_to_timeline("cut", cut.id.clone(), name, None);
        })
    };

    let on_select_preview = {
        let add_to_timeline = add_to_timeline.clone();
        Callback::from(move |preview: Preview| {
            let name = preview
                .custom_name
                .clone()
                .unwrap_or_else(|| preview.folder_name.clone());
            add_to_timeline("frame", preview.id.clone(), name, None);
        })
    };

    // Delete callbacks
    let on_delete_source = {
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |source: SourceContent| {
            let source_files = source_files.clone();
            let frame_directories = frame_directories.clone();
            let video_cuts = video_cuts.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "source_id": source.id, "file_path": source.file_path }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_source_file", args).await;
                // Refresh all data
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(s) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await)
                {
                    source_files.set(s);
                }
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(f) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
                    frame_directories.set(f);
                }
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(c) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
                    video_cuts.set(c);
                }
            });
        })
    };

    let on_delete_frame = {
        let frame_directories = frame_directories.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |frame_dir: FrameDirectory| {
            let frame_directories = frame_directories.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "directoryPath": frame_dir.directory_path
                }))
                .unwrap();
                let _ = tauri_invoke("delete_frame_directory", args).await;
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(f) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
                    frame_directories.set(f);
                }
            });
        })
    };

    let on_delete_cut = {
        let video_cuts = video_cuts.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |cut: VideoCut| {
            let video_cuts = video_cuts.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "cut_id": cut.id, "file_path": cut.file_path }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_cut", args).await;
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(c) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
                    video_cuts.set(c);
                }
            });
        })
    };

    let on_delete_preview = {
        let previews = previews.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |preview: Preview| {
            let previews = previews.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "preview_id": preview.id, "folder_path": preview.folder_path }
                }))
                .unwrap();
                let _ = tauri_invoke("delete_preview", args).await;
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(p) = serde_wasm_bindgen::from_value::<Vec<Preview>>(
                    tauri_invoke("get_project_previews", args).await,
                ) {
                    previews.set(p);
                }
            });
        })
    };

    // Rename callbacks
    let on_rename_source = {
        let source_files = source_files.clone();
        let project_id = props.project_id.clone();
        Callback::from(
            move |(source, custom_name): (SourceContent, Option<String>)| {
                let source_files = source_files.clone();
                let project_id = project_id.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "sourceId": source.id, "customName": custom_name
                    }))
                    .unwrap();
                    let _ = tauri_invoke("rename_source_file", args).await;
                    let args =
                        serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                    if let Ok(s) = serde_wasm_bindgen::from_value(
                        tauri_invoke("get_project_sources", args).await,
                    ) {
                        source_files.set(s);
                    }
                });
            },
        )
    };

    let on_rename_frame = {
        let frame_directories = frame_directories.clone();
        let project_id = props.project_id.clone();
        Callback::from(
            move |(frame_dir, custom_name): (FrameDirectory, Option<String>)| {
                let frame_directories = frame_directories.clone();
                let project_id = project_id.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "folderPath": frame_dir.directory_path, "customName": custom_name }
                }))
                .unwrap();
                    let _ = tauri_invoke("update_frame_custom_name", args).await;
                    let args =
                        serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                    if let Ok(f) = serde_wasm_bindgen::from_value(
                        tauri_invoke("get_project_frames", args).await,
                    ) {
                        frame_directories.set(f);
                    }
                });
            },
        )
    };

    let on_rename_cut = {
        let video_cuts = video_cuts.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |(cut, custom_name): (VideoCut, Option<String>)| {
            let video_cuts = video_cuts.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "cutId": cut.id, "customName": custom_name }
                }))
                .unwrap();
                let _ = tauri_invoke("rename_cut", args).await;
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(c) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                {
                    video_cuts.set(c);
                }
            });
        })
    };

    let on_rename_preview = {
        let previews = previews.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |(preview, custom_name): (Preview, Option<String>)| {
            let previews = previews.clone();
            let project_id = project_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": { "previewId": preview.id, "customName": custom_name }
                }))
                .unwrap();
                let _ = tauri_invoke("rename_preview", args).await;
                let args =
                    serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                if let Ok(p) = serde_wasm_bindgen::from_value::<Vec<Preview>>(
                    tauri_invoke("get_project_previews", args).await,
                ) {
                    previews.set(p);
                }
            });
        })
    };

    let on_open_source = Callback::from(|source: SourceContent| {
        let file_path = source.file_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let folder_path = parent.to_string_lossy().to_string();
                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                let _ = tauri_invoke("open_directory", args).await;
            }
        });
    });

    let on_open_frame = Callback::from(|frame_dir: FrameDirectory| {
        let folder_path = frame_dir.directory_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
            let _ = tauri_invoke("open_directory", args).await;
        });
    });

    let on_open_cut = Callback::from(|cut: VideoCut| {
        let file_path = cut.file_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(parent) = std::path::Path::new(&file_path).parent() {
                let folder_path = parent.to_string_lossy().to_string();
                let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
                let _ = tauri_invoke("open_directory", args).await;
            }
        });
    });

    let on_open_preview = Callback::from(|preview: Preview| {
        let folder_path = preview.folder_path.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "path": folder_path })).unwrap();
            let _ = tauri_invoke("open_directory", args).await;
        });
    });

    let on_add_files_explorer = {
        let project_id = props.project_id.clone();
        let source_files = source_files.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            let project_id = project_id.clone();
            let source_files = source_files.clone();
            let error_message = error_message.clone();
            wasm_bindgen_futures::spawn_local(async move {
                error_message.set(None);
                match tauri_invoke("pick_files", JsValue::NULL).await {
                    result => match serde_wasm_bindgen::from_value::<Vec<String>>(result) {
                        Ok(file_paths) => {
                            if file_paths.is_empty() {
                                return;
                            }

                            let add_files_args = serde_wasm_bindgen::to_value(&json!({
                                "args": {
                                    "request": {
                                        "project_id": project_id,
                                        "file_paths": file_paths
                                    }
                                }
                            }))
                            .unwrap();
                            let _ = tauri_invoke("add_source_files", add_files_args).await;

                            let args =
                                serde_wasm_bindgen::to_value(&json!({ "projectId": project_id }))
                                    .unwrap();
                            if let Ok(sources) = serde_wasm_bindgen::from_value::<Vec<SourceContent>>(
                                tauri_invoke("get_project_sources", args).await,
                            ) {
                                source_files.set(sources);
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

    // Explorer layout change callback
    let on_explorer_layout_change = {
        let explorer_layout = explorer_layout.clone();
        let project_id = props.project_id.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let previews = previews.clone();
        Callback::from(move |new_layout: ExplorerLayout| {
            explorer_layout.set(new_layout.clone());
            let project_id = project_id.clone();
            let entries = project_content_from_layout(
                &new_layout,
                &source_files,
                &video_cuts,
                &frame_directories,
                &previews,
            );
            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({
                    "request": {
                        "project_id": project_id,
                        "entries": entries
                    }
                }))
                .unwrap();
                let _ = tauri_invoke("save_project_content", args).await;
            });
        })
    };

    // Pointer-based drag handler for timeline items (for reordering)
    let on_timeline_item_pointer_down = |index: usize, name: String| {
        Callback::from(move |e: MouseEvent| {
            // Only start drag on left mouse button
            if e.button() != 0 {
                return;
            }
            // Don't start drag if clicking on the remove button
            if let Some(target) = e.target() {
                if let Some(element) = target.dyn_ref::<web_sys::Element>() {
                    if element
                        .closest(".timeline-item-remove")
                        .ok()
                        .flatten()
                        .is_some()
                    {
                        return;
                    }
                }
            }
            web_sys::console::log_1(&format!("Timeline item pointer down: index={}", index).into());
            let data = DragData {
                origin: "timeline".to_string(),
                item_type: "".to_string(),
                id: "".to_string(),
                name: name.clone(),
                index: Some(index),
            };
            if let Ok(json_str) = serde_json::to_string(&data) {
                set_drag_data(&json_str);
                start_pointer_drag_at(e.client_x(), e.client_y());
            }
        })
    };

    let preload_ready_count = timeline_items
        .iter()
        .filter(|clip| {
            matches!(
                clip_preloads.get(&clip.clip_id).map(|state| &state.status),
                Some(PreloadStatus::Ready)
            )
        })
        .count();
    let preload_total_count = timeline_items.len();
    let first_preload_error = timeline_items.iter().find_map(|clip| {
        clip_preloads
            .get(&clip.clip_id)
            .and_then(|state| state.error.clone())
    });
    let preload_blocked = !timeline_items.is_empty()
        && timeline_items.iter().any(|clip| {
            !matches!(
                clip_preloads.get(&clip.clip_id).map(|state| &state.status),
                Some(PreloadStatus::Ready)
            )
        });

    {
        let is_playing = is_playing.clone();
        let preload_blocked = preload_blocked;
        use_effect_with(preload_blocked, move |blocked| {
            if *blocked && *is_playing {
                is_playing.set(false);
            }
            || ()
        });
    }

    html! {
        <div id="montage-page" class="container montage-page">
            <div
                id="montage-layout"
                class={classes!(
                    "montage-layout",
                    props.explorer_on_left.then_some("montage-layout--explorer-left")
                )}
            >
                <div id="montage-explorer-sidebar" class="explorer-sidebar">
                    <div id="montage-sidebar-scroll" class="explorer-sidebar__scroll-area">
                        <ResourcesTree
                            source_files={(*source_files).clone()}
                            video_cuts={(*video_cuts).clone()}
                            frame_directories={(*frame_directories).clone()}
                            previews={(*previews).clone()}
                            sidebar_state={(*sidebar_state).clone()}
                            selected_node_id={(*selected_node_id).clone()}
                            on_toggle_section={on_toggle_section.clone()}
                            on_select_source={on_select_source.clone()}
                            on_select_frame_dir={on_select_frame_dir.clone()}
                            on_select_cut={on_select_cut.clone()}
                            on_select_preview={on_select_preview.clone()}
                            on_delete_source={on_delete_source.clone()}
                            on_delete_frame={on_delete_frame.clone()}
                            on_delete_cut={on_delete_cut.clone()}
                            on_delete_preview={on_delete_preview.clone()}
                            on_rename_source={on_rename_source.clone()}
                            on_rename_frame={on_rename_frame.clone()}
                            on_rename_cut={on_rename_cut.clone()}
                            on_rename_preview={on_rename_preview.clone()}
                            on_open_source={on_open_source.clone()}
                            on_open_frame={on_open_frame.clone()}
                            on_open_cut={on_open_cut.clone()}
                            on_open_preview={on_open_preview.clone()}
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
                            on_select_frame_dir={on_select_frame_dir.clone()}
                            on_select_cut={on_select_cut.clone()}
                            on_select_preview={on_select_preview.clone()}
                        />
                    </div>
                    <div id="montage-sidebar-bottom" class="explorer-sidebar__bottom">
                        <Controls
                            selected_source={(*selected_source).clone()}
                            selected_frame_dir={(*selected_frame_dir).clone()}
                            controls_collapsed={*controls_collapsed}
                            montage_mode={true}
                            has_timeline_items={!timeline_items.is_empty()}
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
                            frames_loading={preload_blocked || *viewer_loading}
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
                        if let Some(ref on_navigate) = props.on_navigate {
                            <ToolsSection on_navigate={on_navigate.clone()} current_page={"montage"} />
                        }
                    </div>
                </div>

                <div id="montage-main-content" class="main-content">
                    <h1 id="montage-heading">{ project.as_ref().map(|p| format!("Montage: {}", p.project_name)).unwrap_or_else(|| "Loading Montage...".into()) }</h1>

                    if let Some(error) = &*error_message {
                        <div id="montage-error-alert" class="alert alert-error">{error}</div>
                    }

                    <div
                        id="montage-workspace"
                        class="montage-workspace"
                    >
                        {
                            if !*is_playing && !timeline_items.is_empty() {
                                let overview_count = timeline_items.len().min(4);
                                let overview_class = classes!(
                                    "montage-workspace-overview",
                                    match overview_count {
                                        1 => Some("montage-workspace-overview--1"),
                                        2 => Some("montage-workspace-overview--2"),
                                        _ => Some("montage-workspace-overview--4"),
                                    }
                                );
                                html! {
                                    <div id="montage-workspace-overview" class={overview_class}>
                                        {timeline_items.iter().take(4).map(|item| {
                                            let tile_id = format!(
                                                "montage-workspace-overview-tile-{}",
                                                dom_id_fragment(&item.clip_id)
                                            );
                                            let tile_preview_id = format!("{}-preview", tile_id);
                                            let preload_state = clip_preloads.get(&item.clip_id).cloned();

                                            html! {
                                                <div
                                                    id={tile_id.clone()}
                                                    class="montage-overview-tile"
                                                    key={item.clip_id.clone()}
                                                    title={item.name.clone()}
                                                >
                                                    <div id={tile_preview_id} class="montage-overview-tile-preview">
                                                        {
                                                            match preload_state {
                                                                Some(preload) if preload.status == PreloadStatus::Ready => {
                                                                    match item.media_type {
                                                                        TimelineMediaType::Video => {
                                                                            if let Some(asset_url) = preload.video_asset_url.clone() {
                                                                                html! {
                                                                                    <MontageVideoStill
                                                                                        key={format!("overview-video-{}", item.clip_id)}
                                                                                        src={asset_url}
                                                                                    />
                                                                                }
                                                                            } else {
                                                                                html! { <span class="montage-overview-placeholder">{"Video unavailable"}</span> }
                                                                            }
                                                                        }
                                                                        TimelineMediaType::Frames | TimelineMediaType::Frame => {
                                                                            if let Some(preloaded_bundle) = preload.frame_bundle.clone() {
                                                                                html! {
                                                                                    <AsciiFramesViewer
                                                                                        key={format!("overview-frames-{}", item.clip_id)}
                                                                                        directory_path={preloaded_bundle.directory_path.clone()}
                                                                                        fps={preload.playback_fps.unwrap_or(24)}
                                                                                        settings={None::<crate::components::ascii_frames_viewer::ConversionSettings>}
                                                                                        should_play={Some(false)}
                                                                                        should_reset={false}
                                                                                        loop_enabled={false}
                                                                                        frame_render_mode={Some(item.frame_render_mode.clone().unwrap_or(FrameRenderMode::BwText))}
                                                                                        frame_colors={Some(preloaded_bundle.frame_colors.clone())}
                                                                                        preloaded_bundle={Some(preloaded_bundle)}
                                                                                    />
                                                                                }
                                                                            } else {
                                                                                html! { <span class="montage-overview-placeholder">{"Frames unavailable"}</span> }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Some(preload) if preload.status == PreloadStatus::Error => html! {
                                                                    <span class="montage-overview-placeholder">
                                                                        {preload.error.unwrap_or_else(|| "Preview unavailable".to_string())}
                                                                    </span>
                                                                },
                                                                _ => html! {
                                                                    <span class="montage-overview-placeholder">{"Loading preview..."}</span>
                                                                },
                                                            }
                                                        }
                                                    </div>
                                                </div>
                                            }
                                        }).collect::<Html>()}
                                    </div>
                                }
                            } else {
                                match &*active_playable {
                                    Some(PlayableItem::Video {clip_id, asset_url}) => html! {
                                        <div id="montage-workspace-video-pane" class="montage-workspace-pane">
                                            <VideoPlayer
                                                key={format!("video-{}", clip_id)}
                                                src={asset_url.clone()}
                                                should_play={if *is_playing {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                loop_enabled={false}
                                                volume={*video_volume}
                                                is_muted={*video_is_muted}
                                                on_progress={on_item_progress.clone()}
                                                on_ended={on_item_ended.clone()}
                                            />
                                        </div>
                                    },
                                    Some(PlayableItem::Frames {clip_id, directory_path, fps, frame_render_mode, preloaded_bundle}) => html! {
                                        <div id="montage-workspace-frames-pane" class="montage-workspace-pane">
                                            <AsciiFramesViewer
                                                key={format!("frames-{}", clip_id)}
                                                directory_path={directory_path.clone()}
                                                fps={*fps}
                                                settings={None::<crate::components::ascii_frames_viewer::ConversionSettings>}
                                                should_play={if *is_playing {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                loop_enabled={false}
                                                on_ended={on_item_ended.clone()}
                                                on_progress={on_item_progress.clone()}
                                                on_loading_changed={{
                                                    let viewer_loading = viewer_loading.clone();
                                                    Callback::from(move |loading: bool| {
                                                        viewer_loading.set(loading);
                                                    })
                                                }}
                                                frame_render_mode={Some(frame_render_mode.clone())}
                                                frame_colors={Some(preloaded_bundle.frame_colors.clone())}
                                                preloaded_bundle={Some(preloaded_bundle.clone())}
                                            />
                                        </div>
                                    },
                                    None => html! {
                                        <p id="montage-workspace-empty-state">{
                                            first_preload_error
                                                .clone()
                                                .unwrap_or_else(|| "Preview area".to_string())
                                        }</p>
                                    },
                                }
                            }
                        }
                    </div>

                    // Timeline axis - drag events handled by JavaScript
                    <div id="montage-timeline-container" class="timeline-container">
                        <div id="montage-timeline-header" class="timeline-header">
                            <span id="montage-timeline-title" class="timeline-title">{"Timeline"}</span>
                            if preload_total_count > 0 {
                                <span id="montage-timeline-status" class="timeline-hint">
                                    {
                                        if let Some(error) = first_preload_error.clone() {
                                            error
                                        } else if preload_blocked {
                                            format!("Loading {}/{}...", preload_ready_count, preload_total_count)
                                        } else {
                                            format!("Ready ({}/{})", preload_ready_count, preload_total_count)
                                        }
                                    }
                                </span>
                            }
                        </div>
                        if !timeline_items.is_empty() {
                            <div id="montage-timeline-progress" class="timeline-progress">
                                <input
                                    id="montage-timeline-progress-slider"
                                    type="range"
                                    min="0"
                                    max="100"
                                    step="0.1"
                                    value={synced_progress.to_string()}
                                    oninput={{
                                        let synced_progress = synced_progress.clone();
                                        let seek_percentage = seek_percentage.clone();
                                        Callback::from(move |e: web_sys::InputEvent| {
                                            if let Some(target) = e.target() {
                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                    let pct = input.value_as_number();
                                                    synced_progress.set(pct);
                                                    seek_percentage.set(Some(pct / 100.0));
                                                }
                                            }
                                        })
                                    }}
                                    title="Timeline progress"
                                    disabled={preload_blocked}
                                />
                            </div>
                        }
                        <div id="montage-timeline-track" class="timeline-track">
                            if timeline_items.is_empty() {
                                <div id="montage-timeline-placeholder" class="timeline-placeholder">
                                    {"Click items in the sidebar to add them here"}
                                </div>
                            } else {
                                <div id="montage-timeline-items-row" class="timeline-items-row">
                                    { timeline_items.iter().enumerate().map(|(index, item)| {
                                        let item_dom_id = format!(
                                            "montage-timeline-item-{}",
                                            dom_id_fragment(&item.clip_id)
                                        );
                                        let item_topline_id = format!("{}-header", item_dom_id);
                                        let item_icon_id = format!("{}-icon", item_dom_id);
                                        let item_name_id = format!("{}-name", item_dom_id);
                                        let item_mode_id = format!("{}-mode-btn", item_dom_id);
                                        let item_remove_id = format!("{}-remove-btn", item_dom_id);
                                        let type_class = match item.resource_kind {
                                            TimelineResourceKind::Source => "source",
                                            TimelineResourceKind::AsciiConversion => "ascii",
                                            TimelineResourceKind::Cut => "cut",
                                            TimelineResourceKind::Preview => "preview",
                                        };
                                        let type_icon = match item.resource_kind {
                                            TimelineResourceKind::Source => IconId::LucideFileVideo,
                                            TimelineResourceKind::Cut => IconId::LucideScissors,
                                            TimelineResourceKind::AsciiConversion => IconId::LucideImage,
                                            TimelineResourceKind::Preview => IconId::LucideCamera,
                                        };
                                        let preload_class = clip_preloads
                                            .get(&item.clip_id)
                                            .map(|state| match state.status {
                                                PreloadStatus::Loading | PreloadStatus::Pending => "loading",
                                                PreloadStatus::Error => "error",
                                                PreloadStatus::Ready => "",
                                            })
                                            .unwrap_or("loading");
                                        let frame_metadata = match item.resource_kind {
                                            TimelineResourceKind::AsciiConversion => frame_directories
                                                .iter()
                                                .find(|frame_dir| {
                                                    frame_clip_resource_id(frame_dir) == item.actual_resource_id
                                                        || frame_dir.directory_path == item.actual_resource_id
                                                })
                                                .map(frame_asset_metadata_from_directory),
                                            TimelineResourceKind::Preview => previews
                                                .iter()
                                                .find(|preview| preview.id == item.actual_resource_id)
                                                .map(frame_asset_metadata_from_preview),
                                            _ => None,
                                        };
                                        let supported_modes = frame_metadata
                                            .as_ref()
                                            .map(supported_frame_render_modes)
                                            .unwrap_or_default();
                                        let current_frame_mode = item
                                            .frame_render_mode
                                            .clone()
                                            .or_else(|| frame_metadata.as_ref().and_then(default_frame_render_mode));
                                        let is_active = *active_timeline_index == Some(index);
                                        let item_class = classes!(
                                            "timeline-item",
                                            type_class,
                                            (!preload_class.is_empty()).then_some(preload_class),
                                            is_active.then_some("active")
                                        );
                                        let on_remove = on_remove_timeline_item.clone();
                                        let item_name = item.name.clone();
                                        let on_toggle_frame_mode = {
                                            let timeline_items = timeline_items.clone();
                                            let is_playing = is_playing.clone();
                                            let metadata = frame_metadata.clone();
                                            let current_frame_mode = current_frame_mode.clone();
                                            Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation();
                                                let Some(metadata) = metadata.clone() else {
                                                    return;
                                                };
                                                let Some(current_mode) = current_frame_mode.clone() else {
                                                    return;
                                                };
                                                if let Some(next_mode) =
                                                    next_frame_render_mode(&metadata, &current_mode)
                                                {
                                                    let mut items = (*timeline_items).clone();
                                                    if let Some(item) = items.get_mut(index) {
                                                        item.frame_render_mode = Some(next_mode);
                                                    }
                                                    is_playing.set(false);
                                                    timeline_items.set(items);
                                                }
                                            })
                                        };

                                        html! {
                                            <div
                                                id={item_dom_id.clone()}
                                                class={item_class}
                                                key={item.clip_id.clone()}
                                                data-clip-id={item.clip_id.clone()}
                                                data-resource-id={item.actual_resource_id.clone()}
                                                data-resource-kind={type_class}
                                                onmousedown={on_timeline_item_pointer_down(index, item_name)}
                                                title={item.name.clone()}
                                            >
                                                <div id={item_topline_id} class="timeline-item-topline">
                                                    <span id={item_icon_id} class="timeline-item-icon">
                                                        <Icon icon_id={type_icon} width={"14"} height={"14"} />
                                                    </span>
                                                    <span id={item_name_id} class="timeline-item-name">{&item.name}</span>
                                                </div>
                                                if !supported_modes.is_empty() {
                                                    <button
                                                        id={item_mode_id}
                                                        class={classes!(
                                                            "timeline-item-mode",
                                                            (supported_modes.len() <= 1).then_some("disabled"),
                                                            current_frame_mode
                                                                .as_ref()
                                                                .filter(|mode| !matches!(mode, FrameRenderMode::BwText))
                                                                .map(|_| "active")
                                                        )}
                                                        type="button"
                                                        onclick={on_toggle_frame_mode}
                                                        disabled={supported_modes.len() <= 1}
                                                        title={current_frame_mode.as_ref().map(FrameRenderMode::title).unwrap_or("Frame mode")}
                                                    >
                                                        {frame_mode_icon(current_frame_mode.as_ref())}
                                                    </button>
                                                }
                                                <button
                                                    id={item_remove_id}
                                                    class="timeline-item-remove"
                                                    onclick={Callback::from(move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        on_remove.emit(index);
                                                    })}
                                                    title="Remove">
                                                    <Icon icon_id={IconId::LucideXCircle} width={"14"} height={"14"} />
                                                </button>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                </div>
                            }
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
