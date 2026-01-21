use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use chrono::{DateTime, Utc};
use yew_icons::{Icon, IconId};
use std::rc::Rc;
use web_sys::DragEvent;
use gloo::events::EventListener;

use super::open::Project;

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

function ensureDragGhost() {
  if (window.__dragGhostEl) return window.__dragGhostEl;

  const el = document.createElement('div');
  el.className = 'pointer-drag-ghost';
  // Inline styles so it's visible even if CSS doesn't load
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
  // Place it immediately so it's visible even before first mousemove
  moveDragGhost(window.__lastPointerX || 0, window.__lastPointerY || 0);
}

function hideDragGhost() {
  if (!window.__dragGhostEl) return;
  window.__dragGhostEl.style.display = 'none';
}

function moveDragGhost(x, y) {
  const el = ensureDragGhost();
  // Offset so it doesn't sit directly under the cursor
  const offsetX = 12;
  const offsetY = 14;
  el.style.left = `${x + offsetX}px`;
  el.style.top = `${y + offsetY}px`;
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
}

export function getPendingDrop() {
  const data = window.__pendingDrop;
  window.__pendingDrop = null;
  return data;
}

export function consumeJustDropped() {
  const wasDropped = window.__justDroppedOnTimeline;
  window.__justDroppedOnTimeline = false;
  return wasDropped;
}

export function startPointerDrag() {
  window.__isPointerDragging = true;
  window.__isPointerOverTimeline = false;
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

    const container = document.querySelector('.timeline-container');
    if (container && window.__dragData) {
      const rect = container.getBoundingClientRect();
      const isOver = e.clientX >= rect.left && e.clientX <= rect.right &&
                     e.clientY >= rect.top && e.clientY <= rect.bottom;

      if (isOver) {
        if (!container.classList.contains('drag-over')) {
          console.log('Drag over timeline-container');
        }
        container.classList.add('drag-over');
      } else {
        container.classList.remove('drag-over');
      }
    }
  }, true);

  document.addEventListener('drop', function(e) {
    e.preventDefault();
    console.log('Document drop at:', e.clientX, e.clientY);

    const container = document.querySelector('.timeline-container');
    if (container) {
      const rect = container.getBoundingClientRect();
      const isOver = e.clientX >= rect.left && e.clientX <= rect.right &&
                     e.clientY >= rect.top && e.clientY <= rect.bottom;

      container.classList.remove('drag-over');

      if (isOver && window.__dragData) {
        console.log('Drop on timeline-container, storing pending drop');
        window.__pendingDrop = window.__dragData;
      }
      window.__dragData = null;
    }
  }, true);

  document.addEventListener('dragend', function(e) {
    console.log('Drag ended');
    const container = document.querySelector('.timeline-container');
    if (container) {
      container.classList.remove('drag-over');
    }
    hideDragGhost();
  }, true);

  // Pointer-based fallback for webviews that don't fire dragover/drop reliably
  document.addEventListener('mousemove', function(e) {
    if (!window.__isPointerDragging || !window.__dragData) return;

    window.__lastPointerX = e.clientX;
    window.__lastPointerY = e.clientY;
    // Keep a visual "ghost" under the cursor while dragging
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
    } else {
      if (window.__isPointerOverTimeline) {
        console.log('Pointer left timeline-container');
        window.__isPointerOverTimeline = false;
      }
      container.classList.remove('drag-over');
    }
  }, true);

  document.addEventListener('mouseup', function(e) {
    if (!window.__isPointerDragging) return;
    console.log('Pointer released');

    const container = document.querySelector('.timeline-container');
    if (container) container.classList.remove('drag-over');
    hideDragGhost();

    if (window.__isPointerOverTimeline && window.__dragData) {
      console.log('Pointer drop on timeline-container, storing pending drop');
      window.__pendingDrop = window.__dragData;
      window.__justDroppedOnTimeline = true;
      window.dispatchEvent(new CustomEvent('cascii:timeline-drop'));
    }

    window.__dragData = null;
    window.__isPointerDragging = false;
    window.__isPointerOverTimeline = false;
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

    #[wasm_bindgen(js_name = startPointerDrag)]
    fn start_pointer_drag();

    #[wasm_bindgen(js_name = startPointerDragAt)]
    fn start_pointer_drag_at(x: i32, y: i32);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceContent {
    pub id: String,
    pub content_type: String,
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
    // Drag state
    let dragging_index = use_state(|| None::<usize>);
    let is_timeline_drag_over = use_state(|| false);

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
                web_sys::console::log_1(&"Rust received cascii:timeline-drop".into());
                if let Some(data_str) = get_pending_drop() {
                    web_sys::console::log_1(&format!("Processing pending drop: {}", data_str).into());
                    if let Ok(drag_data) = serde_json::from_str::<DragData>(&data_str) {
                        if drag_data.origin == "sidebar" {
                            let type_enum = match drag_data.item_type.as_str() {
                                "source" => TimelineItemType::Source,
                                "frame" => TimelineItemType::AsciiConversion,
                                "cut" => TimelineItemType::VideoCut,
                                _ => return,
                            };

                            // Read the current items from the ref (always up-to-date)
                            let mut items = timeline_items_ref.borrow().clone();
                            let new_item = TimelineItem {
                                id: make_unique_timeline_item_id(&drag_data.id),
                                name: drag_data.name,
                                item_type: type_enum,
                                original_id: drag_data.id,
                            };
                            items.push(new_item);
                            timeline_items.set(items);
                        }
                    }
                }
            });
            || drop(listener)
        });
    }

    // Helper: extract DragData from either our JS global or DataTransfer
    let read_drag_data = Rc::new(move |e: &DragEvent| -> Option<DragData> {
        let data_str = get_drag_data().or_else(|| {
            e.data_transfer()
                .and_then(|dt| dt.get_data("text/plain").ok())
                .filter(|s| !s.is_empty())
        })?;
        serde_json::from_str::<DragData>(&data_str).ok()
    });

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

    // Move item in timeline
    let move_timeline_item = {
        let timeline_items = timeline_items.clone();
        Rc::new(move |from_index: usize, to_index: usize| {
            let mut items = (*timeline_items).clone();
            if from_index < items.len() {
                let item = items.remove(from_index);
                // Adjust to_index if we removed an item before it
                let insert_idx = if to_index > from_index {
                    to_index.min(items.len())
                } else {
                    to_index
                };

                // Clamp
                let final_idx = if insert_idx > items.len() { items.len() } else { insert_idx };
                items.insert(final_idx, item);
                timeline_items.set(items);
            }
        })
    };

    // Drag handlers for Sidebar Items
    let _on_sidebar_drag_start = |item_type: String, id: String, name: String| {
        Callback::from(move |e: DragEvent| {
            web_sys::console::log_1(&format!("Sidebar drag start: {}", name).into());
            let data = DragData {
                origin: "sidebar".to_string(),
                item_type: item_type.clone(),
                id: id.clone(),
                name: name.clone(),
                index: None,
            };
            if let Ok(json_str) = serde_json::to_string(&data) {
                set_drag_data(&json_str);
                // Also set on DataTransfer for compatibility
                if let Some(data_transfer) = e.data_transfer() {
                    let _ = data_transfer.set_data("text/plain", &json_str);
                    data_transfer.set_effect_allowed("copyMove");
                }
            }
        })
    };

    // Pointer-based "grab" start for sidebar items (fallback path)
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

    let _on_sidebar_drag_end = {
        let timeline_items = timeline_items.clone();
        Callback::from(move |_: DragEvent| {
            web_sys::console::log_1(&"Sidebar drag end".into());

            // Check if there's a pending drop from JavaScript
            if let Some(data_str) = get_pending_drop() {
                web_sys::console::log_1(&format!("Processing pending drop: {}", data_str).into());
                if let Ok(drag_data) = serde_json::from_str::<DragData>(&data_str) {
                    if drag_data.origin == "sidebar" {
                        let type_enum = match drag_data.item_type.as_str() {
                            "source" => TimelineItemType::Source,
                            "frame" => TimelineItemType::AsciiConversion,
                            "cut" => TimelineItemType::VideoCut,
                            _ => return,
                        };

                        let mut items = (*timeline_items).clone();
                        let new_item = TimelineItem {
                            id: make_unique_timeline_item_id(&drag_data.id),
                            name: drag_data.name,
                            item_type: type_enum,
                            original_id: drag_data.id,
                        };
                        items.push(new_item);
                        timeline_items.set(items);
                    }
                }
            }

            clear_drag_data();
        })
    };

    // Drag/Drop handlers for the timeline container/track (sidebar -> timeline)
    let on_timeline_drag_enter = {
        let is_timeline_drag_over = is_timeline_drag_over.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            e.stop_propagation();
            if !*is_timeline_drag_over {
                web_sys::console::log_1(&"Timeline drag enter".into());
            }
            is_timeline_drag_over.set(true);
        })
    };

    let on_timeline_drag_over = {
        let is_timeline_drag_over = is_timeline_drag_over.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default(); // Required to allow drop
            e.stop_propagation();

            // This fires a lot; only log the first time we consider ourselves "over"
            if !*is_timeline_drag_over {
                web_sys::console::log_1(&"Timeline drag over".into());
                is_timeline_drag_over.set(true);
            }

            if let Some(dt) = e.data_transfer() {
                dt.set_drop_effect("copy");
            }
        })
    };

    let on_timeline_drag_leave = {
        let is_timeline_drag_over = is_timeline_drag_over.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            e.stop_propagation();
            if *is_timeline_drag_over {
                web_sys::console::log_1(&"Timeline drag leave".into());
            }
            is_timeline_drag_over.set(false);
        })
    };

    let on_timeline_drop = {
        let add_to_timeline = add_to_timeline.clone();
        let is_timeline_drag_over = is_timeline_drag_over.clone();
        let read_drag_data = read_drag_data.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            e.stop_propagation();
            web_sys::console::log_1(&"Timeline drop".into());

            is_timeline_drag_over.set(false);

            if let Some(data) = read_drag_data.as_ref()(&e) {
                web_sys::console::log_1(&format!("Timeline drop data: {:?}", data).into());
                if data.origin == "sidebar" {
                    add_to_timeline(&data.item_type, data.id, data.name, None);
                }
            } else {
                web_sys::console::log_1(&"Timeline drop: no drag data found".into());
            }

            clear_drag_data();
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

    // Drag/Drop handlers for Timeline Items
    let on_item_drag_start = {
        let dragging_index = dragging_index.clone();
        Callback::from(move |(index, e): (usize, DragEvent)| {
            web_sys::console::log_1(&format!("Item drag start: {}", index).into());
            dragging_index.set(Some(index));
            let data = DragData {
                origin: "timeline".to_string(),
                item_type: "".to_string(),
                id: "".to_string(),
                name: "".to_string(),
                index: Some(index),
            };
            if let Ok(json_str) = serde_json::to_string(&data) {
                set_drag_data(&json_str);
                if let Some(data_transfer) = e.data_transfer() {
                    let _ = data_transfer.set_data("text/plain", &json_str);
                    data_transfer.set_effect_allowed("copyMove");
                }
            }
        })
    };

    let on_item_drag_end = {
        let dragging_index = dragging_index.clone();
        Callback::from(move |_| {
            dragging_index.set(None);
            clear_drag_data();
        })
    };

    let on_item_drop = {
        let add_to_timeline = add_to_timeline.clone();
        let move_timeline_item = move_timeline_item.clone();
        Callback::from(move |(target_index, e): (usize, DragEvent)| {
            e.prevent_default();
            e.stop_propagation();

            let data_str = get_drag_data().or_else(|| {
                e.data_transfer()
                    .and_then(|dt| dt.get_data("text/plain").ok())
                    .filter(|s| !s.is_empty())
            });

            if let Some(data_str) = data_str {
                if let Ok(data) = serde_json::from_str::<DragData>(&data_str) {
                    if data.origin == "sidebar" {
                        add_to_timeline(&data.item_type, data.id, data.name, Some(target_index));
                    } else if data.origin == "timeline" {
                        if let Some(from_index) = data.index {
                            if from_index != target_index {
                                move_timeline_item(from_index, target_index);
                            }
                        }
                    }
                }
            }
            clear_drag_data();
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
                    <div class={classes!("timeline-container", (*is_timeline_drag_over).then_some("drag-over"))} ondragenter={on_timeline_drag_enter.clone()} ondragover={on_timeline_drag_over.clone()} ondragleave={on_timeline_drag_leave.clone()} ondrop={on_timeline_drop.clone()}>
                        <div class="timeline-header">
                            <span class="timeline-title">{"Timeline"}</span>
                        </div>
                        <div class="timeline-track" ondragenter={on_timeline_drag_enter} ondragover={on_timeline_drag_over} ondragleave={on_timeline_drag_leave} ondrop={on_timeline_drop}>
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
                                        let on_drag_start = on_item_drag_start.clone();
                                        let on_drop = on_item_drop.clone();
                                        let on_remove = on_remove_timeline_item.clone();
                                        let on_drag_end = on_item_drag_end.clone();

                                        html! {
                                            <div class={item_class} key={item.id.clone()} draggable="true" ondragstart={move |e| on_drag_start.emit((index, e))} ondragend={on_drag_end} ondragover={Callback::from(|e: DragEvent| {
                                                    e.prevent_default(); // Allow drop
                                                    if let Some(dt) = e.data_transfer() {
                                                        dt.set_drop_effect("move");
                                                    }
                                                })}
                                                ondrop={move |e| on_drop.emit((index, e))} title={item.name.clone()}>
                                                <div class="timeline-item-header">
                                                    <span class="timeline-item-index">{index + 1}</span>
                                                    <span class="timeline-item-name">{&item.name}</span>
                                                </div>
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
