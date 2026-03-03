use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_icons::IconId;

use super::explorer_types::{NodeKind, ResourceRef, SidebarState, TreeNode, TreeNodeId};
use super::tree_node::TreeNodeView;
use super::tree_section::TreeSection;
use crate::components::settings::available_cuts::VideoCut;
use crate::pages::project::{FrameDirectory, Preview, SourceContent};

#[wasm_bindgen(inline_js = r#"
export async function resourcesTauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args);
  throw new Error('Tauri invoke is not available on this page');
}

export async function resourcesTauriListen(event, handler) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, handler);
  throw new Error('Tauri listen is not available on this page');
}

export async function resourcesTauriUnlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = resourcesTauriInvoke)]
    async fn resources_tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = resourcesTauriListen)]
    async fn resources_tauri_listen(
        event: &str,
        handler: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = resourcesTauriUnlisten)]
    async fn resources_tauri_unlisten(unlisten_fn: JsValue) -> Result<(), JsValue>;
}

#[derive(Properties, PartialEq)]
pub struct ResourcesTreeProps {
    pub source_files: Vec<SourceContent>,
    pub video_cuts: Vec<VideoCut>,
    pub frame_directories: Vec<FrameDirectory>,
    pub previews: Vec<Preview>,
    pub sidebar_state: SidebarState,
    pub selected_node_id: Option<TreeNodeId>,
    pub on_toggle_section: Callback<String>,
    pub on_select_source: Callback<SourceContent>,
    pub on_select_frame_dir: Callback<FrameDirectory>,
    pub on_select_cut: Callback<VideoCut>,
    pub on_select_preview: Callback<Preview>,
    pub on_delete_source: Callback<SourceContent>,
    pub on_delete_frame: Callback<FrameDirectory>,
    pub on_delete_cut: Callback<VideoCut>,
    pub on_delete_preview: Callback<Preview>,
    pub on_rename_source: Callback<SourceContent>,
    pub on_rename_frame: Callback<FrameDirectory>,
    pub on_rename_cut: Callback<VideoCut>,
    pub on_rename_preview: Callback<Preview>,
    pub on_open_source: Callback<SourceContent>,
    pub on_open_frame: Callback<FrameDirectory>,
    pub on_open_cut: Callback<VideoCut>,
    pub on_open_preview: Callback<Preview>,
    #[prop_or_default]
    pub on_add_files: Option<Callback<()>>,
}

#[derive(Clone)]
struct ResourcesMenuHandlers {
    source_files: Vec<SourceContent>,
    video_cuts: Vec<VideoCut>,
    frame_directories: Vec<FrameDirectory>,
    previews: Vec<Preview>,
    on_delete_source: Callback<SourceContent>,
    on_delete_frame: Callback<FrameDirectory>,
    on_delete_cut: Callback<VideoCut>,
    on_delete_preview: Callback<Preview>,
    on_rename_source: Callback<SourceContent>,
    on_rename_frame: Callback<FrameDirectory>,
    on_rename_cut: Callback<VideoCut>,
    on_rename_preview: Callback<Preview>,
    on_open_source: Callback<SourceContent>,
    on_open_frame: Callback<FrameDirectory>,
    on_open_cut: Callback<VideoCut>,
    on_open_preview: Callback<Preview>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowResourcesContextMenuRequest {
    node_id: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeResourcesMenuActionPayload {
    node_id: String,
    action: String,
}

fn is_resource_leaf_node(id: &str) -> bool {
    id.starts_with("res:source:")
        || id.starts_with("res:cut:")
        || id.starts_with("res:framedir:")
        || id.starts_with("res:preview:")
}

/// Build the RESOURCES tree from live data.
fn build_resources_tree(
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
    sidebar_state: &SidebarState,
    selected_id: &Option<TreeNodeId>,
) -> Vec<TreeNode> {
    let is_selected =
        |id: &str| -> bool { selected_id.as_ref().map(|s| s.0 == id).unwrap_or(false) };

    // Source Files -> Original Files
    let original_files: Vec<TreeNode> = source_files
        .iter()
        .map(|f| {
            let display_name = f
                .custom_name
                .as_ref()
                .map(|n| n.as_str())
                .unwrap_or_else(|| {
                    std::path::Path::new(&f.file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&f.file_path)
                })
                .to_string();
            let id = format!("res:source:{}", f.id);
            TreeNode {
                id: TreeNodeId(id.clone()),
                label: display_name,
                node_kind: NodeKind::Leaf(ResourceRef::SourceFile {
                    source_id: f.id.clone(),
                }),
                depth: 3,
                is_expanded: false,
                is_selected: is_selected(&id),
                is_rename_active: false,
                children: vec![],
            }
        })
        .collect();

    // Source Files -> Cuts
    let cuts: Vec<TreeNode> = video_cuts
        .iter()
        .map(|c| {
            let display_name = c.custom_name.as_ref().cloned().unwrap_or_else(|| {
                let start_min = (c.start_time / 60.0) as u32;
                let start_sec = (c.start_time % 60.0) as u32;
                let end_min = (c.end_time / 60.0) as u32;
                let end_sec = (c.end_time % 60.0) as u32;
                format!(
                    "Cut {:02}:{:02} - {:02}:{:02}",
                    start_min, start_sec, end_min, end_sec
                )
            });
            let id = format!("res:cut:{}", c.id);
            TreeNode {
                id: TreeNodeId(id.clone()),
                label: display_name,
                node_kind: NodeKind::Leaf(ResourceRef::VideoCut {
                    cut_id: c.id.clone(),
                }),
                depth: 3,
                is_expanded: false,
                is_selected: is_selected(&id),
                is_rename_active: false,
                children: vec![],
            }
        })
        .collect();

    // Determine which frame dirs come from cuts vs original sources
    let cut_file_paths: Vec<&str> = video_cuts.iter().map(|c| c.file_path.as_str()).collect();

    let mut source_frames_nodes = Vec::new();
    let mut frame_cuts_nodes = Vec::new();

    for fd in frame_directories {
        let display_name = fd.name.clone();
        let id = format!("res:framedir:{}", fd.directory_path);
        let is_from_cut = cut_file_paths.iter().any(|cp| {
            std::path::Path::new(cp)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|name| fd.source_file_name == name)
                .unwrap_or(false)
        });

        let node = TreeNode {
            id: TreeNodeId(id.clone()),
            label: display_name,
            node_kind: NodeKind::Leaf(ResourceRef::FrameDirectory {
                directory_path: fd.directory_path.clone(),
            }),
            depth: 3,
            is_expanded: false,
            is_selected: is_selected(&id),
            is_rename_active: false,
            children: vec![],
        };

        if is_from_cut {
            frame_cuts_nodes.push(node);
        } else {
            source_frames_nodes.push(node);
        }
    }

    // Previews
    let preview_nodes: Vec<TreeNode> = previews
        .iter()
        .map(|p| {
            let display_name = p
                .custom_name
                .as_ref()
                .cloned()
                .unwrap_or_else(|| p.folder_name.clone());
            let id = format!("res:preview:{}", p.id);
            TreeNode {
                id: TreeNodeId(id.clone()),
                label: display_name,
                node_kind: NodeKind::Leaf(ResourceRef::Preview {
                    preview_id: p.id.clone(),
                }),
                depth: 3,
                is_expanded: false,
                is_selected: is_selected(&id),
                is_rename_active: false,
                children: vec![],
            }
        })
        .collect();

    // Build the tree structure
    let source_files_folder = TreeNode {
        id: TreeNodeId("res:source_files".to_string()),
        label: format!("Source Files ({})", source_files.len()),
        node_kind: NodeKind::Folder {
            is_user_created: false,
        },
        depth: 1,
        is_expanded: sidebar_state.source_files_expanded,
        is_selected: false,
        is_rename_active: false,
        children: vec![
            TreeNode {
                id: TreeNodeId("res:original_files".to_string()),
                label: format!("Original Files ({})", original_files.len()),
                node_kind: NodeKind::Folder {
                    is_user_created: false,
                },
                depth: 2,
                is_expanded: sidebar_state.original_files_expanded,
                is_selected: false,
                is_rename_active: false,
                children: original_files,
            },
            TreeNode {
                id: TreeNodeId("res:cuts".to_string()),
                label: format!("Cuts ({})", cuts.len()),
                node_kind: NodeKind::Folder {
                    is_user_created: false,
                },
                depth: 2,
                is_expanded: sidebar_state.cuts_expanded,
                is_selected: false,
                is_rename_active: false,
                children: cuts,
            },
        ],
    };

    let frames_folder = TreeNode {
        id: TreeNodeId("res:frames".to_string()),
        label: format!("Frames ({})", frame_directories.len() + previews.len()),
        node_kind: NodeKind::Folder {
            is_user_created: false,
        },
        depth: 1,
        is_expanded: sidebar_state.frames_expanded,
        is_selected: false,
        is_rename_active: false,
        children: vec![
            TreeNode {
                id: TreeNodeId("res:source_frames".to_string()),
                label: format!("Source Frames ({})", source_frames_nodes.len()),
                node_kind: NodeKind::Folder {
                    is_user_created: false,
                },
                depth: 2,
                is_expanded: sidebar_state.source_frames_expanded,
                is_selected: false,
                is_rename_active: false,
                children: source_frames_nodes,
            },
            TreeNode {
                id: TreeNodeId("res:frame_cuts".to_string()),
                label: format!("Frame Cuts ({})", frame_cuts_nodes.len()),
                node_kind: NodeKind::Folder {
                    is_user_created: false,
                },
                depth: 2,
                is_expanded: sidebar_state.frame_cuts_expanded,
                is_selected: false,
                is_rename_active: false,
                children: frame_cuts_nodes,
            },
            TreeNode {
                id: TreeNodeId("res:previews".to_string()),
                label: format!("Previews ({})", preview_nodes.len()),
                node_kind: NodeKind::Folder {
                    is_user_created: false,
                },
                depth: 2,
                is_expanded: sidebar_state.previews_expanded,
                is_selected: false,
                is_rename_active: false,
                children: preview_nodes,
            },
        ],
    };

    vec![source_files_folder, frames_folder]
}

#[function_component(ResourcesTree)]
pub fn resources_tree(props: &ResourcesTreeProps) -> Html {
    let tree = build_resources_tree(
        &props.source_files,
        &props.video_cuts,
        &props.frame_directories,
        &props.previews,
        &props.sidebar_state,
        &props.selected_node_id,
    );

    let menu_handlers_ref = use_mut_ref(|| ResourcesMenuHandlers {
        source_files: props.source_files.clone(),
        video_cuts: props.video_cuts.clone(),
        frame_directories: props.frame_directories.clone(),
        previews: props.previews.clone(),
        on_delete_source: props.on_delete_source.clone(),
        on_delete_frame: props.on_delete_frame.clone(),
        on_delete_cut: props.on_delete_cut.clone(),
        on_delete_preview: props.on_delete_preview.clone(),
        on_rename_source: props.on_rename_source.clone(),
        on_rename_frame: props.on_rename_frame.clone(),
        on_rename_cut: props.on_rename_cut.clone(),
        on_rename_preview: props.on_rename_preview.clone(),
        on_open_source: props.on_open_source.clone(),
        on_open_frame: props.on_open_frame.clone(),
        on_open_cut: props.on_open_cut.clone(),
        on_open_preview: props.on_open_preview.clone(),
    });
    {
        let mut handlers = menu_handlers_ref.borrow_mut();
        *handlers = ResourcesMenuHandlers {
            source_files: props.source_files.clone(),
            video_cuts: props.video_cuts.clone(),
            frame_directories: props.frame_directories.clone(),
            previews: props.previews.clone(),
            on_delete_source: props.on_delete_source.clone(),
            on_delete_frame: props.on_delete_frame.clone(),
            on_delete_cut: props.on_delete_cut.clone(),
            on_delete_preview: props.on_delete_preview.clone(),
            on_rename_source: props.on_rename_source.clone(),
            on_rename_frame: props.on_rename_frame.clone(),
            on_rename_cut: props.on_rename_cut.clone(),
            on_rename_preview: props.on_rename_preview.clone(),
            on_open_source: props.on_open_source.clone(),
            on_open_frame: props.on_open_frame.clone(),
            on_open_cut: props.on_open_cut.clone(),
            on_open_preview: props.on_open_preview.clone(),
        };
    }

    let menu_listener_handle = use_mut_ref(|| None::<JsValue>);
    let menu_listener_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);

    {
        let menu_handlers_ref = menu_handlers_ref.clone();
        let menu_listener_handle = menu_listener_handle.clone();
        let menu_listener_closure = menu_listener_closure.clone();

        use_effect_with((), move |_| {
            let menu_handlers_ref = menu_handlers_ref.clone();
            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure_storage = menu_listener_closure.clone();

            let on_menu_action = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let payload_key = JsValue::from_str("payload");
                if let Ok(payload_js) = js_sys::Reflect::get(&event, &payload_key) {
                    if let Ok(payload) = serde_wasm_bindgen::from_value::<
                        NativeResourcesMenuActionPayload,
                    >(payload_js)
                    {
                        let handlers = menu_handlers_ref.borrow().clone();
                        let action = payload.action.as_str();

                        if let Some(source_id) = payload.node_id.strip_prefix("res:source:") {
                            if let Some(file) = handlers
                                .source_files
                                .iter()
                                .find(|f| f.id == source_id)
                                .cloned()
                            {
                                match action {
                                    "rename" => handlers.on_rename_source.emit(file),
                                    "open" => handlers.on_open_source.emit(file),
                                    "delete" => handlers.on_delete_source.emit(file),
                                    _ => {}
                                }
                            }
                        } else if let Some(cut_id) = payload.node_id.strip_prefix("res:cut:") {
                            if let Some(cut) =
                                handlers.video_cuts.iter().find(|c| c.id == cut_id).cloned()
                            {
                                match action {
                                    "rename" => handlers.on_rename_cut.emit(cut),
                                    "open" => handlers.on_open_cut.emit(cut),
                                    "delete" => handlers.on_delete_cut.emit(cut),
                                    _ => {}
                                }
                            }
                        } else if let Some(dir_path) = payload.node_id.strip_prefix("res:framedir:")
                        {
                            if let Some(frame_dir) = handlers
                                .frame_directories
                                .iter()
                                .find(|d| d.directory_path == dir_path)
                                .cloned()
                            {
                                match action {
                                    "rename" => handlers.on_rename_frame.emit(frame_dir),
                                    "open" => handlers.on_open_frame.emit(frame_dir),
                                    "delete" => handlers.on_delete_frame.emit(frame_dir),
                                    _ => {}
                                }
                            }
                        } else if let Some(preview_id) =
                            payload.node_id.strip_prefix("res:preview:")
                        {
                            if let Some(preview) = handlers
                                .previews
                                .iter()
                                .find(|p| p.id == preview_id)
                                .cloned()
                            {
                                match action {
                                    "rename" => handlers.on_rename_preview.emit(preview),
                                    "open" => handlers.on_open_preview.emit(preview),
                                    "delete" => handlers.on_delete_preview.emit(preview),
                                    _ => {}
                                }
                            }
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
                    resources_tauri_listen("resources-context-menu-action", &js_callback).await
                {
                    *handle_storage.borrow_mut() = Some(unlisten);
                }
            });

            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure = menu_listener_closure.clone();
            move || {
                if let Some(unlisten) = menu_listener_handle.borrow_mut().take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = resources_tauri_unlisten(unlisten).await;
                    });
                }
                menu_listener_closure.borrow_mut().take();
            }
        });
    }

    // Toggle expand for sub-sections
    let on_toggle_expand = {
        let on_toggle_section = props.on_toggle_section.clone();
        Callback::from(move |id: TreeNodeId| {
            on_toggle_section.emit(id.0);
        })
    };

    // Select a resource node
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
            let id_str = &id.0;
            if let Some(source_id) = id_str.strip_prefix("res:source:") {
                if let Some(src) = source_files.iter().find(|f| f.id == source_id) {
                    on_select_source.emit(src.clone());
                }
            } else if let Some(cut_id) = id_str.strip_prefix("res:cut:") {
                if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id) {
                    on_select_cut.emit(cut.clone());
                }
            } else if let Some(dir_path) = id_str.strip_prefix("res:framedir:") {
                if let Some(fd) = frame_directories
                    .iter()
                    .find(|d| d.directory_path == dir_path)
                {
                    on_select_frame_dir.emit(fd.clone());
                }
            } else if let Some(preview_id) = id_str.strip_prefix("res:preview:") {
                if let Some(p) = previews.iter().find(|pr| pr.id == preview_id) {
                    on_select_preview.emit(p.clone());
                }
            }
        })
    };

    // Native context menu
    let on_context_menu = {
        let on_select = on_select.clone();
        Callback::from(move |(id, e): (TreeNodeId, MouseEvent)| {
            if !is_resource_leaf_node(&id.0) {
                return;
            }

            on_select.emit(id.clone());

            let request = ShowResourcesContextMenuRequest {
                node_id: id.0.clone(),
                x: e.client_x() as f64,
                y: e.client_y() as f64,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(args) = serde_wasm_bindgen::to_value(&serde_json::json!({
                    "request": request
                })) {
                    let _ = resources_tauri_invoke("show_resources_context_menu", args).await;
                }
            });
        })
    };

    // Add files button for the section header
    let add_btn = if let Some(on_add) = &props.on_add_files {
        let on_add = on_add.clone();
        html! {
            <button id="resources-add-files-btn" type="button" class="explorer-add-folder-btn" onclick={Callback::from(move |e: MouseEvent| { e.stop_propagation(); on_add.emit(()); })} title="Add files">
                <yew_icons::Icon icon_id={IconId::LucidePlus} width={"14"} height={"14"} />
            </button>
        }
    } else {
        html! {}
    };

    let on_toggle_resources = {
        let cb = props.on_toggle_section.clone();
        Callback::from(move |_| cb.emit("resources".to_string()))
    };

    html! {
        <>
            <TreeSection title="RESOURCES" is_expanded={props.sidebar_state.resources_expanded} on_toggle={on_toggle_resources} action_buttons={Some(add_btn)}>
                { for tree.into_iter().map(|node| {
                    html! {
                        <TreeNodeView
                            node={node}
                            on_toggle_expand={on_toggle_expand.clone()}
                            on_select={on_select.clone()}
                            on_context_menu={on_context_menu.clone()}
                        />
                    }
                })}
            </TreeSection>
        </>
    }
}
