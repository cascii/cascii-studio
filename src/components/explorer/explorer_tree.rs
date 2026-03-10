use gloo::events::EventListener;
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::DragEvent;
use yew::prelude::*;
use yew_icons::IconId;

use super::drag_state::{get_active_resource_drag, set_active_resource_drag};
use super::explorer_types::*;
use super::tree_node::TreeNodeView;
use super::tree_section::TreeSection;
use crate::components::settings::available_cuts::VideoCut;
use crate::pages::project::{FrameDirectory, Preview, SourceContent};

#[wasm_bindgen(inline_js = r#"
export async function explorerTauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args);
  throw new Error('Tauri invoke is not available on this page');
}

export async function explorerTauriListen(event, handler) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, handler);
  throw new Error('Tauri listen is not available on this page');
}

export async function explorerTauriUnlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = explorerTauriInvoke)]
    async fn explorer_tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = explorerTauriListen)]
    async fn explorer_tauri_listen(
        event: &str,
        handler: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = explorerTauriUnlisten)]
    async fn explorer_tauri_unlisten(unlisten_fn: JsValue) -> Result<(), JsValue>;
}

#[derive(Properties, PartialEq)]
pub struct ExplorerTreeProps {
    pub explorer_layout: ExplorerLayout,
    pub source_files: Vec<SourceContent>,
    pub video_cuts: Vec<VideoCut>,
    pub frame_directories: Vec<FrameDirectory>,
    pub previews: Vec<Preview>,
    pub is_expanded: bool,
    pub selected_node_id: Option<TreeNodeId>,
    pub on_toggle_section: Callback<()>,
    pub on_layout_change: Callback<ExplorerLayout>,
    pub on_select_source: Callback<SourceContent>,
    pub on_select_frame_dir: Callback<FrameDirectory>,
    pub on_select_cut: Callback<VideoCut>,
    pub on_select_preview: Callback<Preview>,
    pub on_rename_source: Callback<(SourceContent, Option<String>)>,
    pub on_rename_frame: Callback<(FrameDirectory, Option<String>)>,
    pub on_rename_cut: Callback<(VideoCut, Option<String>)>,
    pub on_rename_preview: Callback<(Preview, Option<String>)>,
}

#[derive(Clone)]
struct ExplorerMenuHandlers {
    explorer_layout: ExplorerLayout,
    source_files: Vec<SourceContent>,
    video_cuts: Vec<VideoCut>,
    frame_directories: Vec<FrameDirectory>,
    previews: Vec<Preview>,
    on_layout_change: Callback<ExplorerLayout>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowExplorerContextMenuRequest {
    node_id: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeExplorerMenuActionPayload {
    node_id: String,
    action: String,
}

const RESOURCE_DRAG_MIME: &str = "application/x-cascii-resource-node";

fn log_drag(message: &str) {
    web_sys::console::log_1(&message.into());
}

fn resource_node_id_from_ref(resource: &ResourceRef) -> String {
    match resource {
        ResourceRef::SourceFile { source_id } => format!("res:source:{}", source_id),
        ResourceRef::VideoCut { cut_id } => format!("res:cut:{}", cut_id),
        ResourceRef::FrameDirectory { directory_path } => {
            format!("res:framedir:{}", directory_path)
        }
        ResourceRef::Preview { preview_id } => format!("res:preview:{}", preview_id),
    }
}

fn legacy_explorer_resource_node_id_from_ref(resource: &ResourceRef) -> String {
    match resource {
        ResourceRef::SourceFile { source_id } => format!("exp:source:{}", source_id),
        ResourceRef::VideoCut { cut_id } => format!("exp:cut:{}", cut_id),
        ResourceRef::FrameDirectory { directory_path } => {
            format!("exp:framedir:{}", directory_path)
        }
        ResourceRef::Preview { preview_id } => format!("exp:preview:{}", preview_id),
    }
}

fn resource_ref_from_resource_node_id(id: &str) -> Option<ResourceRef> {
    if let Some(source_id) = id.strip_prefix("res:source:") {
        Some(ResourceRef::SourceFile {
            source_id: source_id.to_string(),
        })
    } else if let Some(cut_id) = id.strip_prefix("res:cut:") {
        Some(ResourceRef::VideoCut {
            cut_id: cut_id.to_string(),
        })
    } else if let Some(directory_path) = id.strip_prefix("res:framedir:") {
        Some(ResourceRef::FrameDirectory {
            directory_path: directory_path.to_string(),
        })
    } else if let Some(preview_id) = id.strip_prefix("res:preview:") {
        Some(ResourceRef::Preview {
            preview_id: preview_id.to_string(),
        })
    } else {
        None
    }
}

fn resource_ref_from_explorer_node_id(id: &str) -> Option<ResourceRef> {
    if let Some(source_id) = id.strip_prefix("exp:source:") {
        Some(ResourceRef::SourceFile {
            source_id: source_id.to_string(),
        })
    } else if let Some(cut_id) = id.strip_prefix("exp:cut:") {
        Some(ResourceRef::VideoCut {
            cut_id: cut_id.to_string(),
        })
    } else if let Some(directory_path) = id.strip_prefix("exp:framedir:") {
        Some(ResourceRef::FrameDirectory {
            directory_path: directory_path.to_string(),
        })
    } else if let Some(preview_id) = id.strip_prefix("exp:preview:") {
        Some(ResourceRef::Preview {
            preview_id: preview_id.to_string(),
        })
    } else {
        None
    }
}

fn resource_ref_from_any_node_id(id: &str) -> Option<ResourceRef> {
    resource_ref_from_resource_node_id(id).or_else(|| resource_ref_from_explorer_node_id(id))
}

fn dropped_resource_from_event(event: &DragEvent) -> Option<ResourceRef> {
    if let Some(data_transfer) = event.data_transfer() {
        if let Some(node_id) = data_transfer
            .get_data(RESOURCE_DRAG_MIME)
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                data_transfer
                    .get_data("text/plain")
                    .ok()
                    .filter(|value| !value.is_empty())
            })
        {
            if let Some(resource) = resource_ref_from_any_node_id(&node_id) {
                return Some(resource);
            }
        }
    }

    get_active_resource_drag().and_then(|id| resource_ref_from_any_node_id(&id))
}

fn dragged_node_id_from_event(event: &DragEvent) -> Option<String> {
    let data_transfer = event.data_transfer()?;
    data_transfer
        .get_data(RESOURCE_DRAG_MIME)
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            data_transfer
                .get_data("text/plain")
                .ok()
                .filter(|value| !value.is_empty())
        })
}

fn active_or_event_node_id(event: &DragEvent) -> Option<String> {
    get_active_resource_drag().or_else(|| dragged_node_id_from_event(event))
}

fn event_has_resource_payload(event: &DragEvent) -> bool {
    if active_or_event_node_id(event)
        .as_deref()
        .and_then(resource_ref_from_any_node_id)
        .is_some()
    {
        return true;
    }

    let Some(data_transfer) = event.data_transfer() else {
        return false;
    };
    let types = data_transfer.types();
    if types.length() == 0 {
        // Tauri/WebKit can report empty type lists during active drags.
        return true;
    }
    for idx in 0..types.length() {
        if let Some(ty) = types.get(idx).as_string() {
            if ty == RESOURCE_DRAG_MIME || ty == "text/plain" {
                return true;
            }
        }
    }
    // Be permissive in WebView drag sessions where MIME types are inconsistent.
    true
}

fn folder_node_id_from_point(client_x: i32, client_y: i32) -> Option<TreeNodeId> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let element = document.element_from_point(client_x as f32, client_y as f32)?;
    let tree_node = element.closest(".tree-node").ok().flatten()?;
    let folder_id = tree_node.get_attribute("data-drop-folder-id")?;
    if folder_id.is_empty() || !is_explorer_folder_node(&folder_id) {
        return None;
    }
    Some(TreeNodeId(folder_id))
}

fn point_in_project_drop_zone(client_x: i32, client_y: i32) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(drop_zone) = document.get_element_by_id("project-drop-zone") else {
        return false;
    };

    let rect = drop_zone.get_bounding_client_rect();
    let x = client_x as f64;
    let y = client_y as f64;

    x >= rect.left() && x <= rect.right() && y >= rect.top() && y <= rect.bottom()
}

/// Resolve a ResourceRef to a display label.
fn resolve_label(
    resource: &ResourceRef,
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
) -> String {
    match resource {
        ResourceRef::SourceFile { source_id } => source_files
            .iter()
            .find(|f| f.id == *source_id)
            .map(|f| {
                f.custom_name.as_ref().cloned().unwrap_or_else(|| {
                    std::path::Path::new(&f.file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.file_path)
                        .to_string()
                })
            })
            .unwrap_or_else(|| "(missing)".to_string()),
        ResourceRef::VideoCut { cut_id } => video_cuts
            .iter()
            .find(|c| c.id == *cut_id)
            .map(|c| {
                c.custom_name.as_ref().cloned().unwrap_or_else(|| {
                    let sm = (c.start_time / 60.0) as u32;
                    let ss = (c.start_time % 60.0) as u32;
                    let em = (c.end_time / 60.0) as u32;
                    let es = (c.end_time % 60.0) as u32;
                    format!("Cut {:02}:{:02} - {:02}:{:02}", sm, ss, em, es)
                })
            })
            .unwrap_or_else(|| "(missing)".to_string()),
        ResourceRef::FrameDirectory { directory_path } => frame_directories
            .iter()
            .find(|d| d.directory_path == *directory_path)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "(missing)".to_string()),
        ResourceRef::Preview { preview_id } => previews
            .iter()
            .find(|p| p.id == *preview_id)
            .map(|p| {
                p.custom_name
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| p.folder_name.clone())
            })
            .unwrap_or_else(|| "(missing)".to_string()),
    }
}

/// Convert ExplorerItem list to TreeNode list recursively.
fn items_to_nodes(
    items: &[ExplorerItem],
    depth: u32,
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
    selected_id: &Option<TreeNodeId>,
    rename_id: &Option<TreeNodeId>,
) -> Vec<TreeNode> {
    items
        .iter()
        .map(|item| match item {
            ExplorerItem::Folder {
                id,
                name,
                children,
                is_expanded,
            } => {
                let node_id = format!("exp:folder:{}", id);
                TreeNode {
                    id: TreeNodeId(node_id.clone()),
                    label: name.clone(),
                    node_kind: NodeKind::Folder {
                        is_user_created: true,
                    },
                    depth,
                    is_expanded: *is_expanded,
                    is_selected: selected_id
                        .as_ref()
                        .map(|s| s.0 == node_id)
                        .unwrap_or(false),
                    is_rename_active: rename_id
                        .as_ref()
                        .map(|rid| rid.0 == node_id)
                        .unwrap_or(false),
                    children: items_to_nodes(
                        children,
                        depth + 1,
                        source_files,
                        video_cuts,
                        frame_directories,
                        previews,
                        selected_id,
                        rename_id,
                    ),
                }
            }
            ExplorerItem::ResourceRef(resource) => {
                let label = resolve_label(
                    resource,
                    source_files,
                    video_cuts,
                    frame_directories,
                    previews,
                );
                let node_id = resource_node_id_from_ref(resource);
                TreeNode {
                    id: TreeNodeId(node_id.clone()),
                    label,
                    node_kind: NodeKind::Leaf(resource.clone()),
                    depth,
                    is_expanded: false,
                    is_selected: selected_id
                        .as_ref()
                        .map(|s| s.0 == node_id)
                        .unwrap_or(false),
                    is_rename_active: rename_id
                        .as_ref()
                        .map(|rid| rid.0 == node_id)
                        .unwrap_or(false),
                    children: vec![],
                }
            }
        })
        .collect()
}

/// Add a new folder to the explorer layout.
fn add_folder(layout: &ExplorerLayout) -> ExplorerLayout {
    let mut new_layout = layout.clone();
    let id = format!("{}", js_sys::Math::random().to_bits());
    new_layout.root_items.push(ExplorerItem::Folder {
        id,
        name: "New Folder".to_string(),
        children: vec![],
        is_expanded: true,
    });
    new_layout
}

fn is_explorer_folder_node(id: &str) -> bool {
    id.starts_with("exp:folder:")
}

fn contains_resource_in_container(items: &[ExplorerItem], resource: &ResourceRef) -> bool {
    items.iter().any(|item| match item {
        ExplorerItem::ResourceRef(existing) => existing == resource,
        _ => false,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddResourceResult {
    Added,
    Duplicate,
    FolderNotFound,
}

fn destination_label(layout: &ExplorerLayout, target_folder: Option<&str>) -> String {
    target_folder
        .and_then(|folder_id| find_folder_name(&layout.root_items, folder_id))
        .unwrap_or_else(|| "PROJECT root".to_string())
}

fn log_drop_outcome(resource: &ResourceRef, destination: &str, outcome: AddResourceResult) {
    let resource_id = resource_node_id_from_ref(resource);
    match outcome {
        AddResourceResult::Added => {
            log_drag(&format!(
                "[Explorer DnD] Drop added: {} -> {}",
                resource_id, destination
            ));
        }
        AddResourceResult::Duplicate => {
            log_drag(&format!(
                "[Explorer DnD] Drop ignored (duplicate): {} -> {}",
                resource_id, destination
            ));
        }
        AddResourceResult::FolderNotFound => {
            log_drag(&format!(
                "[Explorer DnD] Drop ignored (folder not found): {} -> {}",
                resource_id, destination
            ));
        }
    }
}

fn add_resource_to_folder(
    items: &mut [ExplorerItem],
    folder_id: &str,
    resource: &ResourceRef,
) -> AddResourceResult {
    for item in items.iter_mut() {
        if let ExplorerItem::Folder {
            id,
            children,
            is_expanded,
            ..
        } = item
        {
            if format!("exp:folder:{}", id) == folder_id {
                *is_expanded = true;
                if contains_resource_in_container(children, resource) {
                    return AddResourceResult::Duplicate;
                }
                children.push(ExplorerItem::ResourceRef(resource.clone()));
                return AddResourceResult::Added;
            }
            let nested_result = add_resource_to_folder(children, folder_id, resource);
            if nested_result != AddResourceResult::FolderNotFound {
                return nested_result;
            }
        }
    }

    AddResourceResult::FolderNotFound
}

fn add_resource_to_layout(
    layout: &mut ExplorerLayout,
    folder_id: Option<&str>,
    resource: &ResourceRef,
) -> AddResourceResult {
    if let Some(folder_id) = folder_id {
        add_resource_to_folder(&mut layout.root_items, folder_id, resource)
    } else if contains_resource_in_container(&layout.root_items, resource) {
        AddResourceResult::Duplicate
    } else {
        layout
            .root_items
            .push(ExplorerItem::ResourceRef(resource.clone()));
        AddResourceResult::Added
    }
}

/// Remove an item from the explorer layout by matching on node id.
fn remove_item(items: &mut Vec<ExplorerItem>, target_id: &str) -> bool {
    let initial_len = items.len();
    items.retain(|item| match item {
        ExplorerItem::Folder { id, .. } => format!("exp:folder:{}", id) != target_id,
        ExplorerItem::ResourceRef(r) => {
            let resource_node_id = resource_node_id_from_ref(r);
            let legacy_node_id = legacy_explorer_resource_node_id_from_ref(r);
            resource_node_id != target_id && legacy_node_id != target_id
        }
    });
    if items.len() < initial_len {
        return true;
    }
    // Recurse into sub-folders
    for item in items.iter_mut() {
        if let ExplorerItem::Folder { children, .. } = item {
            if remove_item(children, target_id) {
                return true;
            }
        }
    }
    false
}

fn rename_folder(items: &mut [ExplorerItem], target_id: &str, new_name: &str) -> bool {
    for item in items.iter_mut() {
        if let ExplorerItem::Folder {
            id, name, children, ..
        } = item
        {
            if format!("exp:folder:{}", id) == target_id {
                *name = new_name.to_string();
                return true;
            }
            if rename_folder(children, target_id, new_name) {
                return true;
            }
        }
    }
    false
}

fn find_folder_name(items: &[ExplorerItem], target_id: &str) -> Option<String> {
    for item in items {
        if let ExplorerItem::Folder {
            id, name, children, ..
        } = item
        {
            if format!("exp:folder:{}", id) == target_id {
                return Some(name.clone());
            }
            if let Some(found) = find_folder_name(children, target_id) {
                return Some(found);
            }
        }
    }
    None
}

/// Toggle the expanded state of a folder by id.
fn toggle_folder(items: &mut Vec<ExplorerItem>, folder_id: &str) -> bool {
    for item in items.iter_mut() {
        if let ExplorerItem::Folder {
            id,
            is_expanded,
            children,
            ..
        } = item
        {
            if format!("exp:folder:{}", id) == folder_id {
                *is_expanded = !*is_expanded;
                return true;
            }
            if toggle_folder(children, folder_id) {
                return true;
            }
        }
    }
    false
}

#[function_component(ExplorerTree)]
pub fn explorer_tree(props: &ExplorerTreeProps) -> Html {
    let rename_target_id = use_state(|| None::<TreeNodeId>);
    let rename_value = use_state(String::new);
    let drop_target_id = use_state(|| None::<TreeNodeId>);
    let root_drop_active = use_state(|| false);
    let menu_handlers_ref = use_mut_ref(|| ExplorerMenuHandlers {
        explorer_layout: props.explorer_layout.clone(),
        source_files: props.source_files.clone(),
        video_cuts: props.video_cuts.clone(),
        frame_directories: props.frame_directories.clone(),
        previews: props.previews.clone(),
        on_layout_change: props.on_layout_change.clone(),
    });
    let last_hover_log_target = use_mut_ref(|| None::<String>);

    {
        let mut handlers = menu_handlers_ref.borrow_mut();
        *handlers = ExplorerMenuHandlers {
            explorer_layout: props.explorer_layout.clone(),
            source_files: props.source_files.clone(),
            video_cuts: props.video_cuts.clone(),
            frame_directories: props.frame_directories.clone(),
            previews: props.previews.clone(),
            on_layout_change: props.on_layout_change.clone(),
        };
    }

    {
        let menu_handlers_ref = menu_handlers_ref.clone();
        let drop_target_id = drop_target_id.clone();
        let root_drop_active = root_drop_active.clone();
        let last_hover_log_target = last_hover_log_target.clone();

        use_effect_with((), move |_| {
            let mousemove_listener = web_sys::window().map(|window| {
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let menu_handlers_ref = menu_handlers_ref.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "mousemove", move |event| {
                    let Some(active_resource_node_id) = get_active_resource_drag() else {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    };

                    let Some(mouse_event) = event.dyn_ref::<web_sys::MouseEvent>() else {
                        return;
                    };

                    let client_x = mouse_event.client_x();
                    let client_y = mouse_event.client_y();

                    if !point_in_project_drop_zone(client_x, client_y) {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    }

                    if let Some(folder_id) = folder_node_id_from_point(client_x, client_y) {
                        if *drop_target_id != Some(folder_id.clone()) {
                            drop_target_id.set(Some(folder_id.clone()));
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        let hover_key = format!("folder:{}", folder_id.0);
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some(hover_key.as_str()) {
                            let handlers = menu_handlers_ref.borrow();
                            let folder_name = find_folder_name(
                                &handlers.explorer_layout.root_items,
                                &folder_id.0,
                            )
                            .unwrap_or_else(|| folder_id.0.clone());
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT folder: {} ({}) while dragging {}",
                                folder_name, folder_id.0, active_resource_node_id
                            ));
                            *last_hover = Some(hover_key);
                        }
                    } else {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if !*root_drop_active {
                            root_drop_active.set(true);
                        }
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some("project-root") {
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT root while dragging {}",
                                active_resource_node_id
                            ));
                            *last_hover = Some("project-root".to_string());
                        }
                    }
                })
            });

            let mouseup_listener = web_sys::window().map(|window| {
                let menu_handlers_ref = menu_handlers_ref.clone();
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "mouseup", move |event| {
                    let Some(node_id) = get_active_resource_drag() else {
                        return;
                    };

                    let Some(mouse_event) = event.dyn_ref::<web_sys::MouseEvent>() else {
                        set_active_resource_drag(None);
                        drop_target_id.set(None);
                        root_drop_active.set(false);
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    };

                    let client_x = mouse_event.client_x();
                    let client_y = mouse_event.client_y();

                    if point_in_project_drop_zone(client_x, client_y) {
                        if let Some(resource) = resource_ref_from_any_node_id(&node_id) {
                            let target_folder =
                                folder_node_id_from_point(client_x, client_y).map(|id| id.0);
                            let handlers = menu_handlers_ref.borrow().clone();
                            let mut new_layout = handlers.explorer_layout.clone();
                            let destination = destination_label(
                                &handlers.explorer_layout,
                                target_folder.as_deref(),
                            );
                            let add_result = add_resource_to_layout(
                                &mut new_layout,
                                target_folder.as_deref(),
                                &resource,
                            );
                            log_drop_outcome(&resource, &destination, add_result);
                            if add_result == AddResourceResult::Added {
                                handlers.on_layout_change.emit(new_layout);
                            }
                        } else {
                            log_drag(&format!(
                                "[Explorer DnD] Mouseup drop ignored: unsupported node id {}",
                                node_id
                            ));
                        }
                    }

                    set_active_resource_drag(None);
                    drop_target_id.set(None);
                    root_drop_active.set(false);
                    if last_hover_log_target.borrow().is_some() {
                        *last_hover_log_target.borrow_mut() = None;
                    }
                })
            });

            // Native drag visual feedback — "drag" fires on the source during
            // HTML5 drag (mousemove does NOT fire during native drag).
            let drag_listener = web_sys::window().map(|window| {
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let menu_handlers_ref = menu_handlers_ref.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "drag", move |event| {
                    let Some(drag_event) = event.dyn_ref::<web_sys::DragEvent>() else {
                        return;
                    };
                    let Some(active_resource_node_id) = active_or_event_node_id(drag_event)
                        .and_then(|node_id| {
                            resource_ref_from_any_node_id(&node_id).map(|_| node_id)
                        })
                    else {
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    };
                    let client_x = drag_event.client_x();
                    let client_y = drag_event.client_y();

                    if client_x == 0 && client_y == 0 {
                        return; // WebKit sends (0,0) at end of drag
                    }

                    if !point_in_project_drop_zone(client_x, client_y) {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    }

                    if let Some(folder_id) = folder_node_id_from_point(client_x, client_y) {
                        if *drop_target_id != Some(folder_id.clone()) {
                            drop_target_id.set(Some(folder_id.clone()));
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        let hover_key = format!("folder:{}", folder_id.0);
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some(hover_key.as_str()) {
                            let handlers = menu_handlers_ref.borrow();
                            let folder_name = find_folder_name(
                                &handlers.explorer_layout.root_items,
                                &folder_id.0,
                            )
                            .unwrap_or_else(|| folder_id.0.clone());
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT folder: {} ({}) while dragging {}",
                                folder_name, folder_id.0, active_resource_node_id
                            ));
                            *last_hover = Some(hover_key);
                        }
                    } else {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if !*root_drop_active {
                            root_drop_active.set(true);
                        }
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some("project-root") {
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT root while dragging {}",
                                active_resource_node_id
                            ));
                            *last_hover = Some("project-root".to_string());
                        }
                    }
                })
            });

            // Fallback hover detection for webviews that don't reliably dispatch
            // `drag` on window or on individual drop targets.
            let dragover_listener = web_sys::window().map(|window| {
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let menu_handlers_ref = menu_handlers_ref.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "dragover", move |event| {
                    let Some(drag_event) = event.dyn_ref::<web_sys::DragEvent>() else {
                        return;
                    };
                    let Some(active_resource_node_id) = active_or_event_node_id(drag_event)
                        .and_then(|node_id| {
                            resource_ref_from_any_node_id(&node_id).map(|_| node_id)
                        })
                    else {
                        return;
                    };

                    if !point_in_project_drop_zone(drag_event.client_x(), drag_event.client_y()) {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    }

                    drag_event.prevent_default();
                    if let Some(data_transfer) = drag_event.data_transfer() {
                        data_transfer.set_drop_effect("copy");
                    }

                    if let Some(folder_id) =
                        folder_node_id_from_point(drag_event.client_x(), drag_event.client_y())
                    {
                        if *drop_target_id != Some(folder_id.clone()) {
                            drop_target_id.set(Some(folder_id.clone()));
                        }
                        if *root_drop_active {
                            root_drop_active.set(false);
                        }
                        let hover_key = format!("folder:{}", folder_id.0);
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some(hover_key.as_str()) {
                            let handlers = menu_handlers_ref.borrow();
                            let folder_name = find_folder_name(
                                &handlers.explorer_layout.root_items,
                                &folder_id.0,
                            )
                            .unwrap_or_else(|| folder_id.0.clone());
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT folder: {} ({}) while dragging {}",
                                folder_name, folder_id.0, active_resource_node_id
                            ));
                            *last_hover = Some(hover_key);
                        }
                    } else {
                        if drop_target_id.is_some() {
                            drop_target_id.set(None);
                        }
                        if !*root_drop_active {
                            root_drop_active.set(true);
                        }
                        let mut last_hover = last_hover_log_target.borrow_mut();
                        if last_hover.as_deref() != Some("project-root") {
                            log_drag(&format!(
                                "[Explorer DnD] Hover PROJECT root while dragging {}",
                                active_resource_node_id
                            ));
                            *last_hover = Some("project-root".to_string());
                        }
                    }
                })
            });

            // Fallback drop handler via dragend — fires on the source element
            // when the native drag ends, even if ondrop on the target did not fire.
            let dragend_listener = web_sys::window().map(|window| {
                let menu_handlers_ref = menu_handlers_ref.clone();
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "dragend", move |event| {
                    let Some(drag_event) = event.dyn_ref::<web_sys::DragEvent>() else {
                        set_active_resource_drag(None);
                        drop_target_id.set(None);
                        root_drop_active.set(false);
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    };
                    let Some(node_id) = active_or_event_node_id(drag_event) else {
                        drop_target_id.set(None);
                        root_drop_active.set(false);
                        if last_hover_log_target.borrow().is_some() {
                            *last_hover_log_target.borrow_mut() = None;
                        }
                        return;
                    };

                    let client_x = drag_event.client_x();
                    let client_y = drag_event.client_y();

                    if point_in_project_drop_zone(client_x, client_y) {
                        if let Some(resource) = resource_ref_from_any_node_id(&node_id) {
                            let target_folder =
                                folder_node_id_from_point(client_x, client_y).map(|id| id.0);
                            let handlers = menu_handlers_ref.borrow().clone();
                            let mut new_layout = handlers.explorer_layout.clone();
                            let destination = destination_label(
                                &handlers.explorer_layout,
                                target_folder.as_deref(),
                            );
                            let add_result = add_resource_to_layout(
                                &mut new_layout,
                                target_folder.as_deref(),
                                &resource,
                            );
                            log_drop_outcome(&resource, &destination, add_result);
                            if add_result == AddResourceResult::Added {
                                handlers.on_layout_change.emit(new_layout);
                            }
                        } else {
                            log_drag(&format!(
                                "[Explorer DnD] Dragend drop ignored: unsupported node id {}",
                                node_id
                            ));
                        }
                    }

                    set_active_resource_drag(None);
                    drop_target_id.set(None);
                    root_drop_active.set(false);
                    if last_hover_log_target.borrow().is_some() {
                        *last_hover_log_target.borrow_mut() = None;
                    }
                })
            });

            // Last-resort drop fallback in case neither element `ondrop` nor
            // source `dragend` performs the insert in this webview.
            let window_drop_listener = web_sys::window().map(|window| {
                let menu_handlers_ref = menu_handlers_ref.clone();
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "drop", move |event| {
                    let Some(drag_event) = event.dyn_ref::<web_sys::DragEvent>() else {
                        return;
                    };
                    if drag_event.default_prevented() {
                        return;
                    }

                    let Some(node_id) = active_or_event_node_id(drag_event) else {
                        return;
                    };
                    if !point_in_project_drop_zone(drag_event.client_x(), drag_event.client_y()) {
                        return;
                    }

                    drag_event.prevent_default();
                    drag_event.stop_propagation();

                    if let Some(resource) = resource_ref_from_any_node_id(&node_id) {
                        let target_folder =
                            folder_node_id_from_point(drag_event.client_x(), drag_event.client_y())
                                .map(|id| id.0);
                        let handlers = menu_handlers_ref.borrow().clone();
                        let mut new_layout = handlers.explorer_layout.clone();
                        let destination =
                            destination_label(&handlers.explorer_layout, target_folder.as_deref());
                        let add_result = add_resource_to_layout(
                            &mut new_layout,
                            target_folder.as_deref(),
                            &resource,
                        );
                        log_drop_outcome(&resource, &destination, add_result);
                        if add_result == AddResourceResult::Added {
                            handlers.on_layout_change.emit(new_layout);
                        }
                    } else {
                        log_drag(&format!(
                            "[Explorer DnD] Window drop ignored: unsupported node id {}",
                            node_id
                        ));
                    }

                    set_active_resource_drag(None);
                    drop_target_id.set(None);
                    root_drop_active.set(false);
                    if last_hover_log_target.borrow().is_some() {
                        *last_hover_log_target.borrow_mut() = None;
                    }
                })
            });

            // Explicit cross-component drop signal emitted by ResourcesTree.
            let resource_drop_signal_listener = web_sys::window().map(|window| {
                let menu_handlers_ref = menu_handlers_ref.clone();
                let drop_target_id = drop_target_id.clone();
                let root_drop_active = root_drop_active.clone();
                let last_hover_log_target = last_hover_log_target.clone();
                EventListener::new(&window, "cascii-resource-drop", move |event| {
                    let detail_key = JsValue::from_str("detail");
                    let node_id_key = JsValue::from_str("nodeId");
                    let folder_id_key = JsValue::from_str("folderId");

                    let Ok(detail) = js_sys::Reflect::get(event.as_ref(), &detail_key) else {
                        return;
                    };

                    let node_id = js_sys::Reflect::get(&detail, &node_id_key)
                        .ok()
                        .and_then(|value| value.as_string())
                        .unwrap_or_default();
                    if node_id.is_empty() {
                        return;
                    }
                    let folder_id =
                        js_sys::Reflect::get(&detail, &folder_id_key)
                            .ok()
                            .and_then(|value| {
                                if value.is_null() || value.is_undefined() {
                                    None
                                } else {
                                    value.as_string()
                                }
                            });

                    if let Some(resource) = resource_ref_from_any_node_id(&node_id) {
                        let handlers = menu_handlers_ref.borrow().clone();
                        let mut new_layout = handlers.explorer_layout.clone();
                        let destination =
                            destination_label(&handlers.explorer_layout, folder_id.as_deref());
                        let add_result = add_resource_to_layout(
                            &mut new_layout,
                            folder_id.as_deref(),
                            &resource,
                        );
                        log_drop_outcome(&resource, &destination, add_result);
                        if add_result == AddResourceResult::Added {
                            handlers.on_layout_change.emit(new_layout);
                        }
                    } else {
                        log_drag(&format!(
                            "[Explorer DnD] Signal drop ignored: unsupported node id {}",
                            node_id
                        ));
                    }

                    set_active_resource_drag(None);
                    drop_target_id.set(None);
                    root_drop_active.set(false);
                    if last_hover_log_target.borrow().is_some() {
                        *last_hover_log_target.borrow_mut() = None;
                    }
                })
            });

            move || {
                drop(mousemove_listener);
                drop(mouseup_listener);
                drop(drag_listener);
                drop(dragover_listener);
                drop(dragend_listener);
                drop(window_drop_listener);
                drop(resource_drop_signal_listener);
            }
        });
    }

    let menu_listener_handle = use_mut_ref(|| None::<JsValue>);
    let menu_listener_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);

    {
        let menu_handlers_ref = menu_handlers_ref.clone();
        let menu_listener_handle = menu_listener_handle.clone();
        let menu_listener_closure = menu_listener_closure.clone();
        let rename_target_id = rename_target_id.clone();
        let rename_value = rename_value.clone();

        use_effect_with((), move |_| {
            let menu_handlers_ref = menu_handlers_ref.clone();
            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure_storage = menu_listener_closure.clone();
            let rename_target_id = rename_target_id.clone();
            let rename_value = rename_value.clone();

            let on_menu_action = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let payload_key = JsValue::from_str("payload");
                if let Ok(payload_js) = js_sys::Reflect::get(&event, &payload_key) {
                    if let Ok(payload) = serde_wasm_bindgen::from_value::<
                        NativeExplorerMenuActionPayload,
                    >(payload_js)
                    {
                        let handlers = menu_handlers_ref.borrow().clone();
                        match payload.action.as_str() {
                            "rename" => {
                                let current_name = if is_explorer_folder_node(&payload.node_id) {
                                    find_folder_name(
                                        &handlers.explorer_layout.root_items,
                                        &payload.node_id,
                                    )
                                    .unwrap_or_else(|| "Folder".to_string())
                                } else if let Some(resource) =
                                    resource_ref_from_any_node_id(&payload.node_id)
                                {
                                    resolve_label(
                                        &resource,
                                        &handlers.source_files,
                                        &handlers.video_cuts,
                                        &handlers.frame_directories,
                                        &handlers.previews,
                                    )
                                } else {
                                    return;
                                };
                                rename_value.set(current_name);
                                rename_target_id.set(Some(TreeNodeId(payload.node_id.clone())));
                            }
                            "delete" => {
                                if !is_explorer_folder_node(&payload.node_id) {
                                    return;
                                }
                                let mut new_layout = handlers.explorer_layout.clone();
                                if remove_item(&mut new_layout.root_items, &payload.node_id) {
                                    handlers.on_layout_change.emit(new_layout);
                                }
                                if rename_target_id
                                    .as_ref()
                                    .map(|id| id.0 == payload.node_id)
                                    .unwrap_or(false)
                                {
                                    rename_target_id.set(None);
                                    rename_value.set(String::new());
                                }
                            }
                            "remove" => {
                                if is_explorer_folder_node(&payload.node_id) {
                                    return;
                                }
                                let mut new_layout = handlers.explorer_layout.clone();
                                if remove_item(&mut new_layout.root_items, &payload.node_id) {
                                    handlers.on_layout_change.emit(new_layout);
                                }
                                if rename_target_id
                                    .as_ref()
                                    .map(|id| id.0 == payload.node_id)
                                    .unwrap_or(false)
                                {
                                    rename_target_id.set(None);
                                    rename_value.set(String::new());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            });

            let js_callback = on_menu_action
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone();
            *menu_listener_closure_storage.borrow_mut() = Some(on_menu_action);

            let handle_storage = menu_listener_handle.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(unlisten) =
                    explorer_tauri_listen("explorer-context-menu-action", &js_callback).await
                {
                    *handle_storage.borrow_mut() = Some(unlisten);
                }
            });

            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure = menu_listener_closure.clone();
            move || {
                if let Some(unlisten) = menu_listener_handle.borrow_mut().take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = explorer_tauri_unlisten(unlisten).await;
                    });
                }
                menu_listener_closure.borrow_mut().take();
            }
        });
    }

    let tree = items_to_nodes(
        &props.explorer_layout.root_items,
        1,
        &props.source_files,
        &props.video_cuts,
        &props.frame_directories,
        &props.previews,
        &props.selected_node_id,
        &rename_target_id,
    );

    let on_toggle_expand = {
        let layout = props.explorer_layout.clone();
        let on_layout_change = props.on_layout_change.clone();
        Callback::from(move |id: TreeNodeId| {
            let mut new_layout = layout.clone();
            toggle_folder(&mut new_layout.root_items, &id.0);
            on_layout_change.emit(new_layout);
        })
    };

    let on_select = {
        let source_files = props.source_files.clone();
        let video_cuts = props.video_cuts.clone();
        let frame_directories = props.frame_directories.clone();
        let previews = props.previews.clone();
        let on_select_source = props.on_select_source.clone();
        let on_select_frame_dir = props.on_select_frame_dir.clone();
        let on_select_cut = props.on_select_cut.clone();
        let on_select_preview = props.on_select_preview.clone();
        Callback::from(move |id: TreeNodeId| {
            if let Some(resource) = resource_ref_from_any_node_id(&id.0) {
                match resource {
                    ResourceRef::SourceFile { source_id } => {
                        if let Some(src) = source_files.iter().find(|f| f.id == source_id) {
                            on_select_source.emit(src.clone());
                        }
                    }
                    ResourceRef::VideoCut { cut_id } => {
                        if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id) {
                            on_select_cut.emit(cut.clone());
                        }
                    }
                    ResourceRef::FrameDirectory { directory_path } => {
                        if let Some(fd) = frame_directories
                            .iter()
                            .find(|d| d.directory_path == directory_path)
                        {
                            on_select_frame_dir.emit(fd.clone());
                        }
                    }
                    ResourceRef::Preview { preview_id } => {
                        if let Some(p) = previews.iter().find(|pr| pr.id == preview_id) {
                            on_select_preview.emit(p.clone());
                        }
                    }
                }
            }
        })
    };

    let on_rename_submit = {
        let layout = props.explorer_layout.clone();
        let on_layout_change = props.on_layout_change.clone();
        let rename_target_id = rename_target_id.clone();
        let rename_value = rename_value.clone();
        let source_files = props.source_files.clone();
        let video_cuts = props.video_cuts.clone();
        let frame_directories = props.frame_directories.clone();
        let previews = props.previews.clone();
        let on_rename_source = props.on_rename_source.clone();
        let on_rename_frame = props.on_rename_frame.clone();
        let on_rename_cut = props.on_rename_cut.clone();
        let on_rename_preview = props.on_rename_preview.clone();
        Callback::from(move |(id, new_name): (TreeNodeId, String)| {
            let trimmed = new_name.trim().to_string();
            if trimmed.is_empty() && is_explorer_folder_node(&id.0) {
                rename_target_id.set(None);
                rename_value.set(String::new());
                return;
            }

            if is_explorer_folder_node(&id.0) {
                let mut new_layout = layout.clone();
                if rename_folder(&mut new_layout.root_items, &id.0, &trimmed) {
                    on_layout_change.emit(new_layout);
                }
            } else {
                let custom_name = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                };
                if let Some(resource) = resource_ref_from_any_node_id(&id.0) {
                    match resource {
                        ResourceRef::SourceFile { source_id } => {
                            if let Some(file) =
                                source_files.iter().find(|f| f.id == source_id).cloned()
                            {
                                on_rename_source.emit((file, custom_name));
                            }
                        }
                        ResourceRef::VideoCut { cut_id } => {
                            if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id).cloned() {
                                on_rename_cut.emit((cut, custom_name));
                            }
                        }
                        ResourceRef::FrameDirectory { directory_path } => {
                            if let Some(frame_dir) = frame_directories
                                .iter()
                                .find(|d| d.directory_path == directory_path)
                                .cloned()
                            {
                                on_rename_frame.emit((frame_dir, custom_name));
                            }
                        }
                        ResourceRef::Preview { preview_id } => {
                            if let Some(preview) =
                                previews.iter().find(|p| p.id == preview_id).cloned()
                            {
                                on_rename_preview.emit((preview, custom_name));
                            }
                        }
                    }
                }
            }
            rename_target_id.set(None);
            rename_value.set(String::new());
        })
    };

    let on_rename_cancel = {
        let rename_target_id = rename_target_id.clone();
        let rename_value = rename_value.clone();
        Callback::from(move |_id: TreeNodeId| {
            rename_target_id.set(None);
            rename_value.set(String::new());
        })
    };

    let on_rename_input = {
        let rename_target_id = rename_target_id.clone();
        let rename_value = rename_value.clone();
        Callback::from(move |(id, value): (TreeNodeId, String)| {
            if rename_target_id
                .as_ref()
                .map(|current| current.0 == id.0)
                .unwrap_or(false)
            {
                rename_value.set(value);
            }
        })
    };

    let on_context_menu = {
        Callback::from(move |(id, e): (TreeNodeId, MouseEvent)| {
            let request = ShowExplorerContextMenuRequest {
                node_id: id.0.clone(),
                x: e.client_x() as f64,
                y: e.client_y() as f64,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(args) = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "request": request
                })) {
                    let _ = explorer_tauri_invoke("show_explorer_context_menu", args).await;
                }
            });
        })
    };

    let on_folder_drag_over = {
        let drop_target_id = drop_target_id.clone();
        let root_drop_active = root_drop_active.clone();
        let menu_handlers_ref = menu_handlers_ref.clone();
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |(id, e): (TreeNodeId, DragEvent)| {
            if !is_explorer_folder_node(&id.0) {
                return;
            }

            if !event_has_resource_payload(&e) {
                return;
            }

            e.prevent_default();
            e.stop_propagation();
            if let Some(data_transfer) = e.data_transfer() {
                data_transfer.set_drop_effect("copy");
            }

            root_drop_active.set(false);
            if *drop_target_id != Some(id.clone()) {
                drop_target_id.set(Some(id.clone()));
            }

            let hover_key = format!("folder:{}", id.0);
            let mut last_hover = last_hover_log_target.borrow_mut();
            if last_hover.as_deref() != Some(hover_key.as_str()) {
                let handlers = menu_handlers_ref.borrow();
                let folder_name = find_folder_name(&handlers.explorer_layout.root_items, &id.0)
                    .unwrap_or_else(|| id.0.clone());
                let active_resource_node_id = get_active_resource_drag().unwrap_or_default();
                log_drag(&format!(
                    "[Explorer DnD] Hover PROJECT folder: {} ({}) while dragging {}",
                    folder_name, id.0, active_resource_node_id
                ));
                *last_hover = Some(hover_key);
            }
        })
    };

    let on_folder_drag_leave = {
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |_id: TreeNodeId| {
            if get_active_resource_drag().is_none() && last_hover_log_target.borrow().is_some() {
                *last_hover_log_target.borrow_mut() = None;
            }
        })
    };

    let on_folder_drop = {
        let layout = props.explorer_layout.clone();
        let on_layout_change = props.on_layout_change.clone();
        let drop_target_id = drop_target_id.clone();
        let root_drop_active = root_drop_active.clone();
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |(id, e): (TreeNodeId, DragEvent)| {
            if !is_explorer_folder_node(&id.0) {
                return;
            }

            let Some(resource) = dropped_resource_from_event(&e) else {
                log_drag("[Explorer DnD] Folder drop ignored: missing resource payload");
                return;
            };

            e.prevent_default();
            e.stop_propagation();

            let mut new_layout = layout.clone();
            let destination =
                find_folder_name(&layout.root_items, &id.0).unwrap_or_else(|| id.0.clone());
            let add_result = add_resource_to_layout(&mut new_layout, Some(&id.0), &resource);
            log_drop_outcome(&resource, &destination, add_result);
            if add_result == AddResourceResult::Added {
                on_layout_change.emit(new_layout);
            }

            set_active_resource_drag(None);
            drop_target_id.set(None);
            root_drop_active.set(false);
            if last_hover_log_target.borrow().is_some() {
                *last_hover_log_target.borrow_mut() = None;
            }
        })
    };

    let on_root_drag_over = {
        let drop_target_id = drop_target_id.clone();
        let root_drop_active = root_drop_active.clone();
        let menu_handlers_ref = menu_handlers_ref.clone();
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |e: DragEvent| {
            if !event_has_resource_payload(&e) {
                return;
            }

            e.prevent_default();
            if let Some(data_transfer) = e.data_transfer() {
                data_transfer.set_drop_effect("copy");
            }
            if let Some(folder_id) = folder_node_id_from_point(e.client_x(), e.client_y()) {
                root_drop_active.set(false);
                if *drop_target_id != Some(folder_id.clone()) {
                    drop_target_id.set(Some(folder_id.clone()));
                }

                let hover_key = format!("folder:{}", folder_id.0);
                let mut last_hover = last_hover_log_target.borrow_mut();
                if last_hover.as_deref() != Some(hover_key.as_str()) {
                    let handlers = menu_handlers_ref.borrow();
                    let folder_name =
                        find_folder_name(&handlers.explorer_layout.root_items, &folder_id.0)
                            .unwrap_or_else(|| folder_id.0.clone());
                    let active_resource_node_id = get_active_resource_drag().unwrap_or_default();
                    log_drag(&format!(
                        "[Explorer DnD] Hover PROJECT folder: {} ({}) while dragging {}",
                        folder_name, folder_id.0, active_resource_node_id
                    ));
                    *last_hover = Some(hover_key);
                }
            } else {
                root_drop_active.set(true);
                if drop_target_id.is_some() {
                    drop_target_id.set(None);
                }
                let mut last_hover = last_hover_log_target.borrow_mut();
                if last_hover.as_deref() != Some("project-root") {
                    let active_resource_node_id = get_active_resource_drag().unwrap_or_default();
                    log_drag(&format!(
                        "[Explorer DnD] Hover PROJECT root while dragging {}",
                        active_resource_node_id
                    ));
                    *last_hover = Some("project-root".to_string());
                }
            }
        })
    };

    let on_root_drag_leave = {
        let root_drop_active = root_drop_active.clone();
        let drop_target_id = drop_target_id.clone();
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |e: DragEvent| {
            if point_in_project_drop_zone(e.client_x(), e.client_y()) {
                return;
            }
            root_drop_active.set(false);
            if drop_target_id.is_some() {
                drop_target_id.set(None);
            }
            if last_hover_log_target.borrow().is_some() {
                *last_hover_log_target.borrow_mut() = None;
            }
        })
    };

    let on_root_drop = {
        let layout = props.explorer_layout.clone();
        let on_layout_change = props.on_layout_change.clone();
        let drop_target_id = drop_target_id.clone();
        let root_drop_active = root_drop_active.clone();
        let last_hover_log_target = last_hover_log_target.clone();
        Callback::from(move |e: DragEvent| {
            let Some(resource) = dropped_resource_from_event(&e) else {
                log_drag("[Explorer DnD] Root drop ignored: missing resource payload");
                return;
            };

            e.prevent_default();
            e.stop_propagation();

            let target_folder = (*drop_target_id).clone().map(|id| id.0);
            let mut new_layout = layout.clone();
            let destination = destination_label(&layout, target_folder.as_deref());
            let add_result =
                add_resource_to_layout(&mut new_layout, target_folder.as_deref(), &resource);
            log_drop_outcome(&resource, &destination, add_result);
            if add_result == AddResourceResult::Added {
                on_layout_change.emit(new_layout);
            }

            set_active_resource_drag(None);
            drop_target_id.set(None);
            root_drop_active.set(false);
            if last_hover_log_target.borrow().is_some() {
                *last_hover_log_target.borrow_mut() = None;
            }
        })
    };

    // Add folder button
    let on_add_folder = {
        let layout = props.explorer_layout.clone();
        let on_layout_change = props.on_layout_change.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let new_layout = add_folder(&layout);
            on_layout_change.emit(new_layout);
        })
    };

    let add_folder_btn = html! {
        <button id="explorer-add-folder-btn" type="button" class="explorer-add-folder-btn" onclick={on_add_folder} title="New Folder">
            <yew_icons::Icon icon_id={IconId::LucideFolderPlus} width={"14"} height={"14"} />
        </button>
    };

    let on_toggle = {
        let cb = props.on_toggle_section.clone();
        Callback::from(move |_| cb.emit(()))
    };

    let project_drop_zone_class = classes!(
        "project-drop-zone",
        (*root_drop_active).then_some("project-drop-zone--active")
    );

    html! {
        <TreeSection title="PROJECT" is_expanded={props.is_expanded} on_toggle={on_toggle} action_buttons={Some(add_folder_btn)}>
            <div
                id="project-drop-zone"
                class={project_drop_zone_class}
                ondragenter={on_root_drag_over.clone()}
                ondragover={on_root_drag_over}
                ondragleave={on_root_drag_leave}
                ondrop={on_root_drop}
            >
                {if tree.is_empty() {
                    html! { <div id="explorer-empty-state" class="tree-section__empty">{"Drag items here to organize"}</div> }
                } else {
                    html! {
                        { for tree.into_iter().map(|node| {
                            html! {
                                <TreeNodeView
                                    node={node}
                                    on_toggle_expand={on_toggle_expand.clone()}
                                    on_select={on_select.clone()}
                                    on_context_menu={on_context_menu.clone()}
                                    on_rename_submit={Some(on_rename_submit.clone())}
                                    on_rename_cancel={Some(on_rename_cancel.clone())}
                                    on_rename_input={Some(on_rename_input.clone())}
                                    rename_value={if rename_target_id.is_some() { Some((*rename_value).clone()) } else { None }}
                                    on_drag_over={Some(on_folder_drag_over.clone())}
                                    on_drag_leave={Some(on_folder_drag_leave.clone())}
                                    on_drop={Some(on_folder_drop.clone())}
                                    drop_target_id={(*drop_target_id).clone()}
                                />
                            }
                        })}
                    }
                }}
            </div>
        </TreeSection>
    }
}
