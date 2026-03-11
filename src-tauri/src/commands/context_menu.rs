use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuEvent};
use tauri::{Emitter, EventTarget, LogicalPosition, Manager};

#[derive(Default)]
pub(crate) struct ResourcesContextMenuState {
    pending: Mutex<Option<PendingResourcesContextMenu>>,
}

#[derive(Clone, Debug)]
struct PendingResourcesContextMenu {
    window_label: String,
    node_id: String,
}

#[derive(Default)]
pub(crate) struct ExplorerContextMenuState {
    pending: Mutex<Option<PendingExplorerContextMenu>>,
}

#[derive(Clone, Debug)]
struct PendingExplorerContextMenu {
    window_label: String,
    node_id: String,
}

#[derive(Default)]
pub(crate) struct OpenProjectsContextMenuState {
    pending: Mutex<Option<PendingOpenProjectsContextMenu>>,
}

#[derive(Clone, Debug)]
struct PendingOpenProjectsContextMenu {
    window_label: String,
    project_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShowResourcesContextMenuRequest {
    node_id: String,
    x: f64,
    y: f64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourcesContextMenuActionPayload {
    node_id: String,
    action: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShowExplorerContextMenuRequest {
    node_id: String,
    x: f64,
    y: f64,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShowOpenProjectsContextMenuRequest {
    project_id: String,
    x: f64,
    y: f64,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExplorerContextMenuActionPayload {
    node_id: String,
    action: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenProjectsContextMenuActionPayload {
    project_id: String,
    action: String,
}

const RESOURCES_MENU_ITEM_RENAME: &str = "resources-context-menu:rename";
const RESOURCES_MENU_ITEM_DUPLICATE: &str = "resources-context-menu:duplicate";
const RESOURCES_MENU_ITEM_OPEN: &str = "resources-context-menu:open";
const RESOURCES_MENU_ITEM_DELETE: &str = "resources-context-menu:delete";
const EXPLORER_MENU_ITEM_RENAME: &str = "explorer-context-menu:rename";
const EXPLORER_MENU_ITEM_DUPLICATE: &str = "explorer-context-menu:duplicate";
const EXPLORER_MENU_ITEM_DELETE: &str = "explorer-context-menu:delete";
const EXPLORER_MENU_ITEM_REMOVE: &str = "explorer-context-menu:remove";
const OPEN_PROJECTS_MENU_ITEM_RENAME: &str = "open-projects-context-menu:rename";
const OPEN_PROJECTS_MENU_ITEM_DUPLICATE: &str = "open-projects-context-menu:duplicate";
const OPEN_PROJECTS_MENU_ITEM_OPEN_FOLDER: &str = "open-projects-context-menu:open-folder";
const OPEN_PROJECTS_MENU_ITEM_DELETE: &str = "open-projects-context-menu:delete";

fn resources_menu_action(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        RESOURCES_MENU_ITEM_RENAME => Some("rename"),
        RESOURCES_MENU_ITEM_DUPLICATE => Some("duplicate"),
        RESOURCES_MENU_ITEM_OPEN => Some("open"),
        RESOURCES_MENU_ITEM_DELETE => Some("delete"),
        _ => None,
    }
}

fn explorer_menu_action(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        EXPLORER_MENU_ITEM_RENAME => Some("rename"),
        EXPLORER_MENU_ITEM_DUPLICATE => Some("duplicate"),
        EXPLORER_MENU_ITEM_DELETE => Some("delete"),
        EXPLORER_MENU_ITEM_REMOVE => Some("remove"),
        _ => None,
    }
}

fn open_projects_menu_action(menu_id: &str) -> Option<&'static str> {
    match menu_id {
        OPEN_PROJECTS_MENU_ITEM_RENAME => Some("rename"),
        OPEN_PROJECTS_MENU_ITEM_DUPLICATE => Some("duplicate"),
        OPEN_PROJECTS_MENU_ITEM_OPEN_FOLDER => Some("open-folder"),
        OPEN_PROJECTS_MENU_ITEM_DELETE => Some("delete"),
        _ => None,
    }
}

#[tauri::command]
pub fn show_resources_context_menu(
    window: tauri::Window,
    state: tauri::State<ResourcesContextMenuState>,
    request: ShowResourcesContextMenuRequest,
) -> Result<(), String> {
    let node_id = request.node_id;
    let is_source = node_id.starts_with("res:source:");
    let is_cut = node_id.starts_with("res:cut:");
    let is_frame_dir = node_id.starts_with("res:framedir:");
    let is_preview = node_id.starts_with("res:preview:");

    if !is_source && !is_cut && !is_frame_dir && !is_preview {
        return Ok(());
    }

    let menu_builder = MenuBuilder::new(&window)
        .text(RESOURCES_MENU_ITEM_RENAME, "Rename")
        .text(RESOURCES_MENU_ITEM_DUPLICATE, "Duplicate")
        .separator()
        .text(RESOURCES_MENU_ITEM_OPEN, "Open Folder")
        .separator()
        .text(RESOURCES_MENU_ITEM_DELETE, "Delete");

    let menu = menu_builder
        .build()
        .map_err(|e| format!("Failed to build resources context menu: {}", e))?;

    if let Ok(mut pending) = state.pending.lock() {
        *pending = Some(PendingResourcesContextMenu {
            window_label: window.label().to_string(),
            node_id,
        });
    }

    window
        .popup_menu_at(&menu, LogicalPosition::new(request.x, request.y))
        .map_err(|e| format!("Failed to show resources context menu: {}", e))
}

#[tauri::command]
pub fn show_explorer_context_menu(
    window: tauri::Window,
    state: tauri::State<ExplorerContextMenuState>,
    request: ShowExplorerContextMenuRequest,
) -> Result<(), String> {
    let is_folder = request.node_id.starts_with("exp:folder:");
    let is_resource = request.node_id.starts_with("exp:source:")
        || request.node_id.starts_with("exp:cut:")
        || request.node_id.starts_with("exp:framedir:")
        || request.node_id.starts_with("exp:preview:")
        || request.node_id.starts_with("res:source:")
        || request.node_id.starts_with("res:cut:")
        || request.node_id.starts_with("res:framedir:")
        || request.node_id.starts_with("res:preview:");

    if !is_folder && !is_resource {
        return Ok(());
    }

    let menu = if is_folder {
        MenuBuilder::new(&window)
            .text(EXPLORER_MENU_ITEM_RENAME, "Rename")
            .text(EXPLORER_MENU_ITEM_DUPLICATE, "Duplicate")
            .separator()
            .text(EXPLORER_MENU_ITEM_DELETE, "Delete Folder")
            .build()
            .map_err(|e| format!("Failed to build explorer context menu: {}", e))?
    } else {
        MenuBuilder::new(&window)
            .text(EXPLORER_MENU_ITEM_RENAME, "Rename")
            .text(EXPLORER_MENU_ITEM_DUPLICATE, "Duplicate")
            .separator()
            .text(EXPLORER_MENU_ITEM_REMOVE, "Remove from Project")
            .build()
            .map_err(|e| format!("Failed to build explorer context menu: {}", e))?
    };

    if let Ok(mut pending) = state.pending.lock() {
        *pending = Some(PendingExplorerContextMenu {
            window_label: window.label().to_string(),
            node_id: request.node_id,
        });
    }

    window
        .popup_menu_at(&menu, LogicalPosition::new(request.x, request.y))
        .map_err(|e| format!("Failed to show explorer context menu: {}", e))
}

#[tauri::command]
pub fn show_open_projects_context_menu(
    window: tauri::Window,
    state: tauri::State<OpenProjectsContextMenuState>,
    request: ShowOpenProjectsContextMenuRequest,
) -> Result<(), String> {
    if request.project_id.trim().is_empty() {
        return Ok(());
    }

    let menu = MenuBuilder::new(&window)
        .text(OPEN_PROJECTS_MENU_ITEM_RENAME, "Rename")
        .text(OPEN_PROJECTS_MENU_ITEM_DUPLICATE, "Duplicate")
        .separator()
        .text(OPEN_PROJECTS_MENU_ITEM_OPEN_FOLDER, "Open Folder")
        .separator()
        .text(OPEN_PROJECTS_MENU_ITEM_DELETE, "Delete")
        .build()
        .map_err(|e| format!("Failed to build open projects context menu: {}", e))?;

    if let Ok(mut pending) = state.pending.lock() {
        *pending = Some(PendingOpenProjectsContextMenu {
            window_label: window.label().to_string(),
            project_id: request.project_id,
        });
    }

    window
        .popup_menu_at(&menu, LogicalPosition::new(request.x, request.y))
        .map_err(|e| format!("Failed to show open projects context menu: {}", e))
}

pub(crate) fn handle_resources_menu_event(app: &tauri::AppHandle, event: &MenuEvent) {
    let Some(action) = resources_menu_action(event.id().as_ref()) else {
        return;
    };

    let Some(state) = app.try_state::<ResourcesContextMenuState>() else {
        return;
    };

    let pending = match state.pending.lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => None,
    };

    let Some(pending) = pending else {
        return;
    };

    let payload = ResourcesContextMenuActionPayload {
        node_id: pending.node_id,
        action: action.to_string(),
    };

    if let Err(err) = app.emit_to(
        EventTarget::window(pending.window_label),
        "resources-context-menu-action",
        payload,
    ) {
        eprintln!("Failed to emit resources context menu action: {}", err);
    }
}

pub(crate) fn handle_explorer_menu_event(app: &tauri::AppHandle, event: &MenuEvent) {
    let Some(action) = explorer_menu_action(event.id().as_ref()) else {
        return;
    };

    let Some(state) = app.try_state::<ExplorerContextMenuState>() else {
        return;
    };

    let pending = match state.pending.lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => None,
    };

    let Some(pending) = pending else {
        return;
    };

    let payload = ExplorerContextMenuActionPayload {
        node_id: pending.node_id,
        action: action.to_string(),
    };

    if let Err(err) = app.emit_to(
        EventTarget::window(pending.window_label),
        "explorer-context-menu-action",
        payload,
    ) {
        eprintln!("Failed to emit explorer context menu action: {}", err);
    }
}

pub(crate) fn handle_open_projects_menu_event(app: &tauri::AppHandle, event: &MenuEvent) {
    let Some(action) = open_projects_menu_action(event.id().as_ref()) else {
        return;
    };

    let Some(state) = app.try_state::<OpenProjectsContextMenuState>() else {
        return;
    };

    let pending = match state.pending.lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => None,
    };

    let Some(pending) = pending else {
        return;
    };

    let payload = OpenProjectsContextMenuActionPayload {
        project_id: pending.project_id,
        action: action.to_string(),
    };

    if let Err(err) = app.emit_to(
        EventTarget::window(pending.window_label),
        "open-projects-context-menu-action",
        payload,
    ) {
        eprintln!("Failed to emit open projects context menu action: {}", err);
    }
}
