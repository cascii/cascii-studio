use gloo::events::EventListener;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

use super::open::Project;
use super::project::{FrameDirectory, Preview, SourceContent};
use crate::components::explorer::{
    ExplorerLayout, ExplorerTree, ResourcesTree, SidebarState, TreeNodeId,
};
use crate::components::settings::available_cuts::VideoCut;
use crate::components::settings::ToolsSection;

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
    let project = use_state(|| None::<Project>);
    let source_files = use_state(|| Vec::<SourceContent>::new());
    let frame_directories = use_state(|| Vec::<FrameDirectory>::new());
    let video_cuts = use_state(|| Vec::<VideoCut>::new());
    let previews = use_state(|| Vec::<Preview>::new());
    let error_message = use_state(|| Option::<String>::None);

    // Explorer sidebar state
    let sidebar_state = use_state(SidebarState::default);
    let explorer_layout = use_state(ExplorerLayout::default);
    let selected_node_id = use_state(|| None::<TreeNodeId>);

    // Timeline state
    let timeline_items = use_state(|| Vec::<TimelineItem>::new());

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

        use_effect_with(project_id.clone(), move |id| {
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
                if let Ok(sources) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await)
                {
                    source_files.set(sources);
                }

                // Fetch frame directories
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                if let Ok(frames) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
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

            || ()
        });
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
        Callback::from(move |index: usize| {
            let mut items = (*timeline_items).clone();
            if index < items.len() {
                items.remove(index);
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
        Callback::from(move |source: SourceContent| {
            let name = source
                .custom_name
                .clone()
                .unwrap_or_else(|| get_file_name(&source.file_path));
            add_to_timeline("source", source.id.clone(), name, None);
        })
    };

    let on_select_frame_dir = {
        let add_to_timeline = add_to_timeline.clone();
        Callback::from(move |frame_dir: FrameDirectory| {
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
                if let Ok(s) = serde_wasm_bindgen::from_value(
                    tauri_invoke("get_project_sources", args).await,
                ) {
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
        Callback::from(move |(source, custom_name): (SourceContent, Option<String>)| {
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
        })
    };

    let on_rename_frame = {
        let frame_directories = frame_directories.clone();
        let project_id = props.project_id.clone();
        Callback::from(move |(frame_dir, custom_name): (FrameDirectory, Option<String>)| {
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
                if let Ok(f) =
                    serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await)
                {
                    frame_directories.set(f);
                }
            });
        })
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

    // Open callbacks (no-op for montage — no viewer pane)
    let on_open_source: Callback<SourceContent> = Callback::from(|_| {});
    let on_open_frame: Callback<FrameDirectory> = Callback::from(|_| {});
    let on_open_cut: Callback<VideoCut> = Callback::from(|_| {});
    let on_open_preview: Callback<Preview> = Callback::from(|_| {});

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

                    <div id="montage-workspace" class="montage-workspace">
                        <p>{"Preview area"}</p>
                    </div>

                    // Timeline axis - drag events handled by JavaScript
                    <div id="montage-timeline-container" class="timeline-container">
                        <div id="montage-timeline-header" class="timeline-header">
                            <span id="montage-timeline-title" class="timeline-title">{"Timeline"}</span>
                        </div>
                        <div id="montage-timeline-track" class="timeline-track">
                            if timeline_items.is_empty() {
                                <div id="montage-timeline-placeholder" class="timeline-placeholder">
                                    {"Click items in the sidebar to add them here"}
                                </div>
                            } else {
                                <div id="montage-timeline-items-row" class="timeline-items-row">
                                    { timeline_items.iter().enumerate().map(|(index, item)| {
                                        let item_class = match item.item_type {
                                            TimelineItemType::Source => "timeline-item source",
                                            TimelineItemType::AsciiConversion => "timeline-item ascii",
                                            TimelineItemType::VideoCut => "timeline-item cut",
                                        };
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
