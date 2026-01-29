use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{DateTime, Utc};
use yew_icons::{Icon, IconId};
use std::rc::Rc;
use gloo::events::EventListener;

use super::open::Project;
use super::project::SourceContent;

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrameDirectory {
    pub name: String,
    pub directory_path: String,
    pub source_file_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VideoCut {
    pub id: String,
    pub project_id: String,
    pub source_file_id: String,
    pub file_path: String,
    pub date_added: String,
    pub size: i64,
    pub custom_name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    pub duration: f64,
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
    origin: String, // "sidebar" or "timeline"
    item_type: String, // "source", "frame", "cut" (for sidebar)
    id: String,
    name: String,
    index: Option<usize>, // for timeline
}

#[derive(Properties, PartialEq)]
pub struct MontagePageProps {
    pub project_id: String,
}

#[function_component(MontagePage)]
pub fn montage_page(props: &MontagePageProps) -> Html {
    let project = use_state(|| None::<Project>);
    let source_files = use_state(|| Vec::<SourceContent>::new());
    let frame_directories = use_state(|| Vec::<FrameDirectory>::new());
    let video_cuts = use_state(|| Vec::<VideoCut>::new());
    let error_message = use_state(|| Option::<String>::None);

    // Collapsible section states
    let sources_collapsed = use_state(|| false);
    let frames_collapsed = use_state(|| false);
    let cuts_collapsed = use_state(|| false);

    // Timeline state
    let timeline_items = use_state(|| Vec::<TimelineItem>::new());

    // Load project details and data
    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
        let video_cuts = video_cuts.clone();
        let error_message = error_message.clone();

        use_effect_with(project_id.clone(), move |id| {
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
                if let Ok(sources) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_sources", args).await) {
                    source_files.set(sources);
                }

                // Fetch frame directories
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                if let Ok(frames) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_frames", args).await) {
                    frame_directories.set(frames);
                }

                // Fetch video cuts
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                if let Ok(cuts) = serde_wasm_bindgen::from_value(tauri_invoke("get_project_cuts", args).await) {
                    video_cuts.set(cuts);
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
        Rc::new(move |item_type: &str, id: String, name: String, insert_at: Option<usize>| {
            web_sys::console::log_1(&format!("Adding to timeline: type={}, name={}", item_type, name).into());
            let type_enum = match item_type {
                "source" => TimelineItemType::Source,
                "frame" => TimelineItemType::AsciiConversion,
                "cut" => TimelineItemType::VideoCut,
                _ => {
                    web_sys::console::log_1(&format!("Unknown item type: {}", item_type).into());
                    return;
                },
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
        })
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
                            web_sys::console::log_1(&format!("Parsed drag data - origin: {}, index: {:?}", drag_data.origin, drag_data.index).into());
                            let mut items = timeline_items_ref.borrow().clone();
                            web_sys::console::log_1(&format!("Current items count: {}", items.len()).into());

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
                                    web_sys::console::log_1(&format!("Moving item from {} to {}", from_index, to_index).into());
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
                                    web_sys::console::log_1(&"No target index for timeline reorder".into());
                                }
                            }
                        }
                        },
                        Err(e) => {
                            web_sys::console::log_1(&format!("Failed to parse drag data: {:?}", e).into());
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

    // Pointer-based drag start for sidebar items
    let on_sidebar_pointer_down = |item_type: String, id: String, name: String| {
        Callback::from(move |e: MouseEvent| {
            // Only start drag on left mouse button
            if e.button() != 0 {
                return;
            }
            web_sys::console::log_1(&format!("Sidebar pointer down: {}", name).into());
            let data = DragData {
                origin: "sidebar".to_string(),
                item_type: item_type.clone(),
                id: id.clone(),
                name: name.clone(),
                index: None,
            };
            if let Ok(json_str) = serde_json::to_string(&data) {
                set_drag_data(&json_str);
                start_pointer_drag_at(e.client_x(), e.client_y());
            }
        })
    };

    // Click to add (alternative to drag)
    let on_sidebar_click = {
        let add_to_timeline = add_to_timeline.clone();
        move |item_type: String, id: String, name: String| {
            let add_to_timeline = add_to_timeline.clone();
            Callback::from(move |_: MouseEvent| {
                // Skip if we just did a pointer drop (to avoid double-adding)
                if consume_just_dropped() {
                    web_sys::console::log_1(&"Click skipped - just dropped".into());
                    return;
                }
                add_to_timeline(&item_type, id.clone(), name.clone(), None);
            })
        }
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
                    if element.closest(".timeline-item-remove").ok().flatten().is_some() {
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
        <div class="container montage-page">
            <div class="montage-layout">
                <div class="left-sidebar">
                    // Source Files Section
                    <div class="sidebar-section">
                        <div class="section-header" onclick={{
                            let sources_collapsed = sources_collapsed.clone();
                            Callback::from(move |_| sources_collapsed.set(!*sources_collapsed))
                        }}>
                            <Icon icon_id={IconId::LucideFolder} width={"16"} height={"16"} />
                            <span class="section-title">{"Source Files"}</span>
                            <span class="section-count">{format!("({})", source_files.len())}</span>
                            <Icon icon_id={if *sources_collapsed { IconId::LucidePlus } else { IconId::LucideMinus }} width={"14"} height={"14"} />
                        </div>
                        if !*sources_collapsed {
                            <div class="section-content">
                                if source_files.is_empty() {
                                    <div class="empty-message">{"No source files"}</div>
                                } else {
                                    { source_files.iter().map(|source| {
                                        let display_name = source.custom_name.clone()
                                            .unwrap_or_else(|| get_file_name(&source.file_path));
                                        let id = source.id.clone();
                                        let name = display_name.clone();

                                        let click_id = source.id.clone();
                                        let click_name = display_name.clone();
                                        html! {
                                            <div class="list-item clickable" key={source.id.clone()} draggable="false" onmousedown={on_sidebar_pointer_down("source".to_string(), id, name.clone())} onclick={on_sidebar_click("source".to_string(), click_id, click_name)}>
                                                <span class="item-name">{display_name}</span>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                }
                            </div>
                        }
                    </div>

                    // ASCII Conversions Section
                    <div class="sidebar-section">
                        <div class="section-header" onclick={{
                            let frames_collapsed = frames_collapsed.clone();
                            Callback::from(move |_| frames_collapsed.set(!*frames_collapsed))
                        }}>
                            <Icon icon_id={IconId::LucideWand} width={"16"} height={"16"} />
                            <span class="section-title">{"ASCII Conversions"}</span>
                            <span class="section-count">{format!("({})", frame_directories.len())}</span>
                            <Icon icon_id={if *frames_collapsed { IconId::LucidePlus } else { IconId::LucideMinus }} width={"14"} height={"14"} />
                        </div>
                        if !*frames_collapsed {
                            <div class="section-content">
                                if frame_directories.is_empty() {
                                    <div class="empty-message">{"No ASCII conversions"}</div>
                                } else {
                                    { frame_directories.iter().map(|frame_dir| {
                                        let id = frame_dir.directory_path.clone();
                                        let name = frame_dir.name.clone();
                                        let click_id = frame_dir.directory_path.clone();
                                        let click_name = frame_dir.name.clone();
                                        html! {
                                            <div class="list-item clickable" key={frame_dir.directory_path.clone()} draggable="false" onmousedown={on_sidebar_pointer_down("frame".to_string(), id, name.clone())} onclick={on_sidebar_click("frame".to_string(), click_id, click_name)}>
                                                <span class="item-name">{&frame_dir.name}</span>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                }
                            </div>
                        }
                    </div>

                    // Video Cuts Section
                    <div class="sidebar-section">
                        <div class="section-header" onclick={{
                            let cuts_collapsed = cuts_collapsed.clone();
                            Callback::from(move |_| cuts_collapsed.set(!*cuts_collapsed))
                        }}>
                            <Icon icon_id={IconId::LucideScissors} width={"16"} height={"16"} />
                            <span class="section-title">{"Video Cuts"}</span>
                            <span class="section-count">{format!("({})", video_cuts.len())}</span>
                            <Icon icon_id={if *cuts_collapsed { IconId::LucidePlus } else { IconId::LucideMinus }} width={"14"} height={"14"} />
                        </div>
                        if !*cuts_collapsed {
                            <div class="section-content">
                                if video_cuts.is_empty() {
                                    <div class="empty-message">{"No video cuts"}</div>
                                } else {
                                    { video_cuts.iter().map(|cut| {
                                        let display_name = cut.custom_name.clone()
                                            .unwrap_or_else(|| get_file_name(&cut.file_path));
                                        let id = cut.id.clone();
                                        let name = display_name.clone();
                                        let click_id = cut.id.clone();
                                        let click_name = display_name.clone();
                                        html! {
                                            <div class="list-item clickable" key={cut.id.clone()} draggable="false" onmousedown={on_sidebar_pointer_down("cut".to_string(), id, name.clone())} onclick={on_sidebar_click("cut".to_string(), click_id, click_name)}>
                                                <span class="item-name">{display_name}</span>
                                            </div>
                                        }
                                    }).collect::<Html>() }
                                }
                            </div>
                        }
                    </div>
                </div>

                <div class="main-content">
                    <h1>{ project.as_ref().map(|p| format!("Montage: {}", p.project_name)).unwrap_or_else(|| "Loading Montage...".into()) }</h1>

                    if let Some(error) = &*error_message {
                        <div class="alert alert-error">{error}</div>
                    }

                    <div class="montage-workspace">
                        <p>{"Preview area"}</p>
                    </div>

                    // Timeline axis - drag events handled by JavaScript
                    <div class="timeline-container">
                        <div class="timeline-header">
                            <span class="timeline-title">{"Timeline"}</span>
                        </div>
                        <div class="timeline-track">
                            if timeline_items.is_empty() {
                                <div class="timeline-placeholder">
                                    {"Click items in the sidebar to add them here"}
                                </div>
                            } else {
                                <div class="timeline-items-row">
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
