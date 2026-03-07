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
use super::project::{FrameDirectory, PreparedMedia, Preview, PreviewSettings, SourceContent};
use super::project_cache::{
    get_project_sidebar_cache, set_project_sidebar_cache, ProjectSidebarCache,
};
use crate::components::ascii_frames_viewer::AsciiFramesViewer;
use crate::components::explorer::{ExplorerLayout, ExplorerTree, ResourcesTree, TreeNodeId};
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
"#)]
extern "C" {
    #[wasm_bindgen(js_name = appConvertFileSrc)]
    fn app_convert_file_src(path: &str) -> String;
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayableItem {
    Video { asset_url: String },
    Frames { directory_path: String, fps: u32, settings: Option<PreviewSettings> },
}

// Timeline item types
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineItemType {
    Source,
    AsciiConversion,
    VideoCut,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineItem {
    pub id: String,
    pub name: String,
    pub item_type: TimelineItemType,
    pub original_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DragData {
    origin: String,    // "sidebar" or "timeline"
    item_type: String, // "source", "frame", "cut" (for sidebar)
    id: String,
    name: String,
    index: Option<usize>, // for timeline
}

fn calculate_workspace_height(
    main_content: &web_sys::Element,
    workspace: &web_sys::Element,
    timeline: &web_sys::Element,
    media_aspect_ratio: Option<f64>,
) -> Option<f64> {
    let main_rect = main_content.get_bounding_client_rect();
    let workspace_rect = workspace.get_bounding_client_rect();
    let timeline_rect = timeline.get_bounding_client_rect();
    let gap_between_sections = (timeline_rect.top() - workspace_rect.bottom()).max(0.0);

    let available_height =
        (main_rect.bottom() - workspace_rect.top() - timeline_rect.height() - gap_between_sections
            - 2.0)
            .max(0.0);
    let available_width = workspace_rect.width().max(0.0);

    if available_height <= 0.0 || available_width <= 0.0 {
        return Some(0.0);
    }

    let target_height = media_aspect_ratio
        .filter(|ratio| ratio.is_finite() && *ratio > 0.0)
        .map(|ratio| available_height.min(available_width / ratio))
        .unwrap_or(available_height);

    Some(target_height.max(0.0).floor())
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
    let cached_sidebar_data = get_project_sidebar_cache(&props.project_id);
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
    let frames_loading = use_state(|| false);
    let loop_enabled = use_state(|| true);
    let video_volume = use_state(|| 1.0f64);
    let video_is_muted = use_state(|| false);

    // Explorer sidebar state
    let sidebar_state = use_state(move || cached_sidebar_state.clone());
    let explorer_layout = use_state(ExplorerLayout::default);
    let selected_node_id = use_state(|| None::<TreeNodeId>);

    // Timeline state
    let timeline_items = use_state(|| Vec::<TimelineItem>::new());

    // Playback orchestration state
    let active_timeline_index = use_state(|| None::<usize>);
    let active_playable = use_state(|| None::<PlayableItem>);
    let url_cache = use_state(|| HashMap::<String, String>::new());
    let active_media_aspect_ratio = use_state(|| None::<f64>);
    let workspace_height = use_state(|| None::<f64>);
    let montage_main_content_ref = use_node_ref();
    let montage_workspace_ref = use_node_ref();
    let montage_timeline_ref = use_node_ref();

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

    let recalculate_workspace_height = {
        let montage_main_content_ref = montage_main_content_ref.clone();
        let montage_workspace_ref = montage_workspace_ref.clone();
        let montage_timeline_ref = montage_timeline_ref.clone();
        let workspace_height = workspace_height.clone();
        let active_media_aspect_ratio = active_media_aspect_ratio.clone();

        Rc::new(move || {
            let next_height = montage_main_content_ref
                .cast::<web_sys::Element>()
                .and_then(|main_content| {
                    montage_workspace_ref
                        .cast::<web_sys::Element>()
                        .map(|workspace| (main_content, workspace))
                })
                .and_then(|(main_content, workspace)| {
                    montage_timeline_ref
                        .cast::<web_sys::Element>()
                        .map(|timeline| (main_content, workspace, timeline))
                })
                .and_then(|(main_content, workspace, timeline)| {
                    calculate_workspace_height(
                        &main_content,
                        &workspace,
                        &timeline,
                        *active_media_aspect_ratio,
                    )
                });
            workspace_height.set(next_height);
        })
    };

    {
        let active_playable = active_playable.clone();
        let active_media_aspect_ratio = active_media_aspect_ratio.clone();

        use_effect_with((*active_playable).clone(), move |_| {
            active_media_aspect_ratio.set(None);
            || ()
        });
    }

    {
        let recalculate_workspace_height = recalculate_workspace_height.clone();
        let timeline_len = (*timeline_items).len();
        let has_error = (*error_message).is_some();
        let playable_present = (*active_playable).is_some();
        let media_aspect_ratio = *active_media_aspect_ratio;

        use_effect_with(
            (timeline_len, has_error, playable_present, media_aspect_ratio),
            move |_| {
                recalculate_workspace_height();
                || ()
            },
        );
    }

    {
        let recalculate_workspace_height = recalculate_workspace_height.clone();

        use_effect_with((), move |_| {
            recalculate_workspace_height();
            let listener = web_sys::window().map(|window| {
                let recalculate_workspace_height = recalculate_workspace_height.clone();
                EventListener::new(&window, "resize", move |_| {
                    recalculate_workspace_height();
                })
            });

            move || {
                drop(listener);
            }
        });
    }

    // Resolve a timeline item to a PlayableItem and activate it
    let resolve_and_activate = {
        let active_playable = active_playable.clone();
        let active_timeline_index = active_timeline_index.clone();
        let source_files = source_files.clone();
        let video_cuts = video_cuts.clone();
        let frame_directories = frame_directories.clone();
        let previews = previews.clone();
        let url_cache = url_cache.clone();
        let is_playing = is_playing.clone();
        let timeline_items = timeline_items.clone();
        let loop_enabled = loop_enabled.clone();
        Rc::new(move |index: usize| {
            let items = (*timeline_items).clone();
            let Some(item) = items.get(index).cloned() else {
                // Past the end — loop or stop
                if *loop_enabled && !items.is_empty() {
                    active_timeline_index.set(Some(0));
                    // Re-resolve index 0 in the next render cycle via effect
                } else {
                    is_playing.set(false);
                    active_timeline_index.set(None);
                    active_playable.set(None);
                }
                return;
            };
            active_timeline_index.set(Some(index));

            match item.item_type {
                TimelineItemType::Source => {
                    let source = (*source_files).iter().find(|s| s.id == item.original_id).cloned();
                    if let Some(source) = source {
                        // Check url cache
                        if let Some(cached_url) = url_cache.get(&source.file_path) {
                            active_playable.set(Some(PlayableItem::Video {asset_url: cached_url.clone()}));
                        } else {
                            let active_playable = active_playable.clone();
                            let url_cache = url_cache.clone();
                            let file_path = source.file_path.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&json!({"path": file_path})).unwrap();
                                let result = tauri_invoke("prepare_media", args).await;
                                if let Ok(prepared) = serde_wasm_bindgen::from_value::<PreparedMedia>(result) {
                                    let asset_url = app_convert_file_src(&prepared.cached_abs_path);
                                    let mut cache = (*url_cache).clone();
                                    cache.insert(file_path, asset_url.clone());
                                    url_cache.set(cache);
                                    active_playable.set(Some(PlayableItem::Video {asset_url}));
                                }
                            });
                        }
                    }
                }
                TimelineItemType::VideoCut => {
                    let cut = (*video_cuts).iter().find(|c| c.id == item.original_id).cloned();
                    if let Some(cut) = cut {
                        if let Some(cached_url) = url_cache.get(&cut.file_path) {
                            active_playable.set(Some(PlayableItem::Video {asset_url: cached_url.clone()}));
                        } else {
                            let active_playable = active_playable.clone();
                            let url_cache = url_cache.clone();
                            let file_path = cut.file_path.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                let args = serde_wasm_bindgen::to_value(&json!({"path": file_path})).unwrap();
                                let result = tauri_invoke("prepare_media", args).await;
                                if let Ok(prepared) = serde_wasm_bindgen::from_value::<PreparedMedia>(result) {
                                    let asset_url = app_convert_file_src(&prepared.cached_abs_path);
                                    let mut cache = (*url_cache).clone();
                                    cache.insert(file_path, asset_url.clone());
                                    url_cache.set(cache);
                                    active_playable.set(Some(PlayableItem::Video {asset_url}));
                                }
                            });
                        }
                    }
                }
                TimelineItemType::AsciiConversion => {
                    // Try as frame directory first (original_id = directory_path)
                    if let Some(fd) = (*frame_directories).iter().find(|f| f.directory_path == item.original_id) {
                        active_playable.set(Some(PlayableItem::Frames {directory_path: fd.directory_path.clone(), fps: 24, settings: None}));
                    }
                    // Try as preview (original_id = preview.id)
                    else if let Some(preview) = (*previews).iter().find(|p| p.id == item.original_id) {
                        active_playable.set(Some(PlayableItem::Frames {directory_path: preview.folder_path.clone(), fps: preview.settings.fps, settings: Some(preview.settings.clone())}));
                    }
                    // Fallback: try original_id as a direct directory path
                    else {
                        active_playable.set(Some(PlayableItem::Frames {directory_path: item.original_id.clone(), fps: 24, settings: None}));
                    }
                }
            }
        })
    };

    // When active_timeline_index changes and we need to resolve (e.g. after loop wrap)
    {
        let resolve_and_activate = resolve_and_activate.clone();
        let active_idx = *active_timeline_index;
        let active_playable_val = (*active_playable).clone();
        use_effect_with((active_idx, active_playable_val.is_none()), move |(idx, needs_resolve)| {
            if let Some(index) = idx {
                if *needs_resolve {
                    resolve_and_activate(*index);
                }
            }
            || ()
        });
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
        Callback::from(move |local_progress: f64| {
            let total = timeline_items.len();
            if total == 0 { return; }
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

        use_effect_with(project_id.clone(), move |id| {
            if let Some(cached_data) = get_project_sidebar_cache(id) {
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
                if let Some(cached_project) = cached_project {
                    on_project_name_change.emit(cached_project.project_name);
                }
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
                    if let Ok(sources) = serde_wasm_bindgen::from_value(
                        tauri_invoke("get_project_sources", args).await,
                    ) {
                        source_files.set(sources);
                    }

                    // Fetch frame directories
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    if let Ok(frames) = serde_wasm_bindgen::from_value(
                        tauri_invoke("get_project_frames", args).await,
                    ) {
                        frame_directories.set(frames);
                    }

                    // Fetch video cuts
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    if let Ok(cuts) =
                        serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await)
                    {
                        video_cuts.set(cuts);
                    }

                    // Fetch previews
                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                    if let Ok(p) = serde_wasm_bindgen::from_value::<Vec<Preview>>(
                        tauri_invoke("get_project_previews", args).await,
                    ) {
                        previews.set(p);
                    }
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
        use_effect_with(
            (
                project_id,
                project,
                source_files,
                frame_directories,
                video_cuts,
                previews,
                sidebar_state,
            ),
            move |(
                project_id,
                project,
                source_files,
                frame_directories,
                video_cuts,
                previews,
                sidebar_state,
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
                    },
                );
                || ()
            },
        );
    }

    // Helper to get display name from file path
    fn get_file_name(path: &str) -> String {
        std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string()
    }

    fn make_unique_timeline_item_id(original_id: &str) -> String {
        let ts = js_sys::Date::now();
        let rand = (js_sys::Math::random() * 1_000_000_000_f64).floor() as u32;
        format!("timeline-{}-{}-{}", original_id, ts, rand)
    }

    // Add item to timeline helper
    let add_to_timeline = {
        let timeline_items = timeline_items.clone();
        Rc::new(
            move |item_type: &str, id: String, name: String, insert_at: Option<usize>| {
                web_sys::console::log_1(
                    &format!("Adding to timeline: type={}, name={}", item_type, name).into(),
                );
                let type_enum = match item_type {
                    "source" => TimelineItemType::Source,
                    "frame" => TimelineItemType::AsciiConversion,
                    "cut" => TimelineItemType::VideoCut,
                    _ => {
                        web_sys::console::log_1(
                            &format!("Unknown item type: {}", item_type).into(),
                        );
                        return;
                    }
                };

                let mut items = (*timeline_items).clone();
                let new_item = TimelineItem {
                    id: make_unique_timeline_item_id(&id),
                    name,
                    item_type: type_enum,
                    original_id: id,
                };

                if let Some(index) = insert_at {
                    if index <= items.len() {
                        items.insert(index, new_item);
                    } else {
                        items.push(new_item);
                    }
                } else {
                    items.push(new_item);
                }
                timeline_items.set(items);
            },
        )
    };

    // Use a ref to always have the latest timeline_items available for the event listener
    let timeline_items_ref = use_mut_ref(|| Vec::<TimelineItem>::new());
    // Keep the ref in sync with state on each render
    *timeline_items_ref.borrow_mut() = (*timeline_items).clone();

    // Listen for pointer-based drops coming from JS and apply them to timeline state
    {
        let timeline_items = timeline_items.clone();
        let timeline_items_ref = timeline_items_ref.clone();
        use_effect_with((), move |_| {
            let timeline_items = timeline_items.clone();
            let timeline_items_ref = timeline_items_ref.clone();
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
                                // Adding new item from sidebar
                                let type_enum = match drag_data.item_type.as_str() {
                                    "source" => TimelineItemType::Source,
                                    "frame" => TimelineItemType::AsciiConversion,
                                    "cut" => TimelineItemType::VideoCut,
                                    _ => return,
                                };

                                let new_item = TimelineItem {
                                    id: make_unique_timeline_item_id(&drag_data.id),
                                    name: drag_data.name,
                                    item_type: type_enum,
                                    original_id: drag_data.id,
                                };

                                if let Some(idx) = target_index {
                                    if idx <= items.len() {
                                        items.insert(idx, new_item);
                                    } else {
                                        items.push(new_item);
                                    }
                                } else {
                                    items.push(new_item);
                                }
                                timeline_items.set(items);
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
                .unwrap_or_else(|| get_file_name(&source.file_path));
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
                frame_dir.directory_path.clone(),
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
                .unwrap_or_else(|| get_file_name(&cut.file_path));
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

    let on_workspace_aspect_ratio_change = {
        let active_media_aspect_ratio = active_media_aspect_ratio.clone();

        Callback::from(move |ratio: Option<f64>| {
            active_media_aspect_ratio.set(ratio.filter(|value| value.is_finite() && *value > 0.0));
        })
    };

    let workspace_style = (*workspace_height)
        .map(|height| format!("height: {:.0}px; max-height: {:.0}px;", height, height))
        .unwrap_or_default();

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
        Callback::from(move |new_layout: ExplorerLayout| {
            explorer_layout.set(new_layout.clone());
            let project_id = project_id.clone();
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
                        if let Some(ref on_navigate) = props.on_navigate {
                            <ToolsSection on_navigate={on_navigate.clone()} current_page={"montage"} />
                        }
                    </div>
                </div>

                <div id="montage-main-content" class="main-content" ref={montage_main_content_ref.clone()}>
                    <h1 id="montage-heading">{ project.as_ref().map(|p| format!("Montage: {}", p.project_name)).unwrap_or_else(|| "Loading Montage...".into()) }</h1>

                    if let Some(error) = &*error_message {
                        <div id="montage-error-alert" class="alert alert-error">{error}</div>
                    }

                    <div
                        id="montage-workspace"
                        class="montage-workspace"
                        ref={montage_workspace_ref.clone()}
                        style={workspace_style}
                    >
                        {
                            match &*active_playable {
                                Some(PlayableItem::Video {asset_url}) => html! {
                                    <VideoPlayer
                                        key={(*active_timeline_index).map(|i| format!("video-{}", i)).unwrap_or_default()}
                                        src={asset_url.clone()}
                                        should_play={if *is_playing {Some(true)} else {Some(false)}}
                                        should_reset={*should_reset}
                                        loop_enabled={false}
                                        volume={*video_volume}
                                        is_muted={*video_is_muted}
                                        on_progress={on_item_progress.clone()}
                                        on_ended={on_item_ended.clone()}
                                        on_aspect_ratio_change={Some(on_workspace_aspect_ratio_change.clone())}
                                    />
                                },
                                Some(PlayableItem::Frames {directory_path, fps, settings: _}) => html! {
                                    <AsciiFramesViewer
                                        key={(*active_timeline_index).map(|i| format!("frames-{}", i)).unwrap_or_default()}
                                        directory_path={directory_path.clone()}
                                        fps={*fps}
                                        settings={None::<crate::components::ascii_frames_viewer::ConversionSettings>}
                                        should_play={if *is_playing {Some(true)} else {Some(false)}}
                                        should_reset={*should_reset}
                                        loop_enabled={false}
                                        on_ended={on_item_ended.clone()}
                                        on_progress={on_item_progress.clone()}
                                        on_loading_changed={{
                                            let frames_loading = frames_loading.clone();
                                            Callback::from(move |loading: bool| {
                                                frames_loading.set(loading);
                                            })
                                        }}
                                        on_aspect_ratio_change={Some(on_workspace_aspect_ratio_change.clone())}
                                    />
                                },
                                None => html! {
                                    <p>{"Preview area"}</p>
                                },
                            }
                        }
                    </div>

                    // Timeline axis - drag events handled by JavaScript
                    <div id="montage-timeline-container" class="timeline-container" ref={montage_timeline_ref.clone()}>
                        <div id="montage-timeline-header" class="timeline-header">
                            <span id="montage-timeline-title" class="timeline-title">{"Timeline"}</span>
                        </div>
                        if !timeline_items.is_empty() {
                            <div id="montage-timeline-progress" class="timeline-progress">
                                <input
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
                                        let type_class = match item.item_type {
                                            TimelineItemType::Source => "source",
                                            TimelineItemType::AsciiConversion => "ascii",
                                            TimelineItemType::VideoCut => "cut",
                                        };
                                        let is_active = *active_timeline_index == Some(index);
                                        let item_class = classes!("timeline-item", type_class, is_active.then_some("active"));
                                        let on_remove = on_remove_timeline_item.clone();
                                        let item_name = item.name.clone();

                                        html! {
                                            <div class={item_class} key={item.id.clone()} onmousedown={on_timeline_item_pointer_down(index, item_name)} title={item.name.clone()}>
                                                <span class="timeline-item-name">{&item.name}</span>
                                                <button
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
