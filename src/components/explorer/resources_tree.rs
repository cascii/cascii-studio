use yew::prelude::*;
use yew_icons::IconId;

use crate::pages::project::{SourceContent, FrameDirectory, Preview};
use crate::components::settings::available_cuts::VideoCut;
use super::explorer_types::{TreeNode, TreeNodeId, NodeKind, ResourceRef, SidebarState};
use super::tree_node::TreeNodeView;
use super::tree_section::TreeSection;
use super::context_menu::{ContextMenu, ContextMenuItem};

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
    pub on_open_source: Callback<SourceContent>,
    pub on_open_frame: Callback<FrameDirectory>,
    pub on_open_cut: Callback<VideoCut>,
    pub on_open_preview: Callback<Preview>,
    #[prop_or_default]
    pub on_add_files: Option<Callback<()>>,
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
    let is_selected = |id: &str| -> bool {
        selected_id.as_ref().map(|s| s.0 == id).unwrap_or(false)
    };

    // Source Files -> Original Files
    let original_files: Vec<TreeNode> = source_files.iter().map(|f| {
        let display_name = f.custom_name.as_ref()
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
            node_kind: NodeKind::Leaf(ResourceRef::SourceFile { source_id: f.id.clone() }),
            depth: 3,
            is_expanded: false,
            is_selected: is_selected(&id),
            is_rename_active: false,
            children: vec![],
        }
    }).collect();

    // Source Files -> Cuts
    let cuts: Vec<TreeNode> = video_cuts.iter().map(|c| {
        let display_name = c.custom_name.as_ref()
            .cloned()
            .unwrap_or_else(|| {
                let start_min = (c.start_time / 60.0) as u32;
                let start_sec = (c.start_time % 60.0) as u32;
                let end_min = (c.end_time / 60.0) as u32;
                let end_sec = (c.end_time % 60.0) as u32;
                format!("Cut {:02}:{:02} - {:02}:{:02}", start_min, start_sec, end_min, end_sec)
            });
        let id = format!("res:cut:{}", c.id);
        TreeNode {
            id: TreeNodeId(id.clone()),
            label: display_name,
            node_kind: NodeKind::Leaf(ResourceRef::VideoCut { cut_id: c.id.clone() }),
            depth: 3,
            is_expanded: false,
            is_selected: is_selected(&id),
            is_rename_active: false,
            children: vec![],
        }
    }).collect();

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
            node_kind: NodeKind::Leaf(ResourceRef::FrameDirectory { directory_path: fd.directory_path.clone() }),
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
    let preview_nodes: Vec<TreeNode> = previews.iter().map(|p| {
        let display_name = p.custom_name.as_ref()
            .cloned()
            .unwrap_or_else(|| p.folder_name.clone());
        let id = format!("res:preview:{}", p.id);
        TreeNode {
            id: TreeNodeId(id.clone()),
            label: display_name,
            node_kind: NodeKind::Leaf(ResourceRef::Preview { preview_id: p.id.clone() }),
            depth: 3,
            is_expanded: false,
            is_selected: is_selected(&id),
            is_rename_active: false,
            children: vec![],
        }
    }).collect();

    // Build the tree structure
    let source_files_folder = TreeNode {
        id: TreeNodeId("res:source_files".to_string()),
        label: format!("Source Files ({})", source_files.len()),
        node_kind: NodeKind::Folder { is_user_created: false },
        depth: 1,
        is_expanded: sidebar_state.source_files_expanded,
        is_selected: false,
        is_rename_active: false,
        children: vec![
            TreeNode {
                id: TreeNodeId("res:original_files".to_string()),
                label: format!("Original Files ({})", original_files.len()),
                node_kind: NodeKind::Folder { is_user_created: false },
                depth: 2,
                is_expanded: sidebar_state.original_files_expanded,
                is_selected: false,
                is_rename_active: false,
                children: original_files,
            },
            TreeNode {
                id: TreeNodeId("res:cuts".to_string()),
                label: format!("Cuts ({})", cuts.len()),
                node_kind: NodeKind::Folder { is_user_created: false },
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
        node_kind: NodeKind::Folder { is_user_created: false },
        depth: 1,
        is_expanded: sidebar_state.frames_expanded,
        is_selected: false,
        is_rename_active: false,
        children: vec![
            TreeNode {
                id: TreeNodeId("res:source_frames".to_string()),
                label: format!("Source Frames ({})", source_frames_nodes.len()),
                node_kind: NodeKind::Folder { is_user_created: false },
                depth: 2,
                is_expanded: sidebar_state.source_frames_expanded,
                is_selected: false,
                is_rename_active: false,
                children: source_frames_nodes,
            },
            TreeNode {
                id: TreeNodeId("res:frame_cuts".to_string()),
                label: format!("Frame Cuts ({})", frame_cuts_nodes.len()),
                node_kind: NodeKind::Folder { is_user_created: false },
                depth: 2,
                is_expanded: sidebar_state.frame_cuts_expanded,
                is_selected: false,
                is_rename_active: false,
                children: frame_cuts_nodes,
            },
            TreeNode {
                id: TreeNodeId("res:previews".to_string()),
                label: format!("Previews ({})", preview_nodes.len()),
                node_kind: NodeKind::Folder { is_user_created: false },
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
    let context_menu_state = use_state(|| None::<(TreeNodeId, i32, i32)>);

    let tree = build_resources_tree(
        &props.source_files,
        &props.video_cuts,
        &props.frame_directories,
        &props.previews,
        &props.sidebar_state,
        &props.selected_node_id,
    );

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
                if let Some(fd) = frame_directories.iter().find(|d| d.directory_path == dir_path) {
                    on_select_frame_dir.emit(fd.clone());
                }
            } else if let Some(preview_id) = id_str.strip_prefix("res:preview:") {
                if let Some(p) = previews.iter().find(|pr| pr.id == preview_id) {
                    on_select_preview.emit(p.clone());
                }
            }
        })
    };

    // Context menu
    let on_context_menu = {
        let ctx_state = context_menu_state.clone();
        Callback::from(move |(id, e): (TreeNodeId, MouseEvent)| {
            // Only show context menu for leaf nodes (not folders)
            if id.0.starts_with("res:source:") || id.0.starts_with("res:cut:")
                || id.0.starts_with("res:framedir:") || id.0.starts_with("res:preview:") {
                ctx_state.set(Some((id, e.client_x(), e.client_y())));
            }
        })
    };

    let on_close_menu = {
        let ctx_state = context_menu_state.clone();
        Callback::from(move |_| {
            ctx_state.set(None);
        })
    };

    // Build context menu items based on selected node
    let context_menu_html = if let Some((ref ctx_id, x, y)) = *context_menu_state {
        let id_str = ctx_id.0.clone();
        let mut items = Vec::new();

        if let Some(source_id) = id_str.strip_prefix("res:source:") {
            let file = props.source_files.iter().find(|f| f.id == source_id).cloned();
            if let Some(file) = file {
                let f1 = file.clone();
                let f2 = file.clone();
                let f3 = file.clone();
                let on_rename = props.on_rename_source.clone();
                let on_open = props.on_open_source.clone();
                let on_delete = props.on_delete_source.clone();
                let close1 = on_close_menu.clone();
                let close2 = on_close_menu.clone();
                let close3 = on_close_menu.clone();
                items.push(ContextMenuItem {
                    label: "Rename".to_string(),
                    icon: IconId::LucidePencil,
                    on_click: Callback::from(move |_| { on_rename.emit(f1.clone()); close1.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Open Folder".to_string(),
                    icon: IconId::LucideFolderOpen,
                    on_click: Callback::from(move |_| { on_open.emit(f2.clone()); close2.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Delete".to_string(),
                    icon: IconId::LucideTrash2,
                    on_click: Callback::from(move |_| { on_delete.emit(f3.clone()); close3.emit(()); }),
                    is_destructive: true,
                });
            }
        } else if let Some(cut_id) = id_str.strip_prefix("res:cut:") {
            let cut = props.video_cuts.iter().find(|c| c.id == cut_id).cloned();
            if let Some(cut) = cut {
                let c1 = cut.clone();
                let c2 = cut.clone();
                let c3 = cut.clone();
                let on_rename = props.on_rename_cut.clone();
                let on_open = props.on_open_cut.clone();
                let on_delete = props.on_delete_cut.clone();
                let close1 = on_close_menu.clone();
                let close2 = on_close_menu.clone();
                let close3 = on_close_menu.clone();
                items.push(ContextMenuItem {
                    label: "Rename".to_string(),
                    icon: IconId::LucidePencil,
                    on_click: Callback::from(move |_| { on_rename.emit(c1.clone()); close1.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Open Folder".to_string(),
                    icon: IconId::LucideFolderOpen,
                    on_click: Callback::from(move |_| { on_open.emit(c2.clone()); close2.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Delete".to_string(),
                    icon: IconId::LucideTrash2,
                    on_click: Callback::from(move |_| { on_delete.emit(c3.clone()); close3.emit(()); }),
                    is_destructive: true,
                });
            }
        } else if let Some(dir_path) = id_str.strip_prefix("res:framedir:") {
            let fd = props.frame_directories.iter().find(|d| d.directory_path == dir_path).cloned();
            if let Some(fd) = fd {
                let f1 = fd.clone();
                let f2 = fd.clone();
                let f3 = fd.clone();
                let on_rename = props.on_rename_frame.clone();
                let on_open = props.on_open_frame.clone();
                let on_delete = props.on_delete_frame.clone();
                let close1 = on_close_menu.clone();
                let close2 = on_close_menu.clone();
                let close3 = on_close_menu.clone();
                items.push(ContextMenuItem {
                    label: "Rename".to_string(),
                    icon: IconId::LucidePencil,
                    on_click: Callback::from(move |_| { on_rename.emit(f1.clone()); close1.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Open Folder".to_string(),
                    icon: IconId::LucideFolderOpen,
                    on_click: Callback::from(move |_| { on_open.emit(f2.clone()); close2.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Delete".to_string(),
                    icon: IconId::LucideTrash2,
                    on_click: Callback::from(move |_| { on_delete.emit(f3.clone()); close3.emit(()); }),
                    is_destructive: true,
                });
            }
        } else if let Some(preview_id) = id_str.strip_prefix("res:preview:") {
            let preview = props.previews.iter().find(|p| p.id == preview_id).cloned();
            if let Some(preview) = preview {
                let p1 = preview.clone();
                let p2 = preview.clone();
                let on_open = props.on_open_preview.clone();
                let on_delete = props.on_delete_preview.clone();
                let close1 = on_close_menu.clone();
                let close2 = on_close_menu.clone();
                items.push(ContextMenuItem {
                    label: "Open Folder".to_string(),
                    icon: IconId::LucideFolderOpen,
                    on_click: Callback::from(move |_| { on_open.emit(p1.clone()); close1.emit(()); }),
                    is_destructive: false,
                });
                items.push(ContextMenuItem {
                    label: "Delete".to_string(),
                    icon: IconId::LucideTrash2,
                    on_click: Callback::from(move |_| { on_delete.emit(p2.clone()); close2.emit(()); }),
                    is_destructive: true,
                });
            }
        }

        if items.is_empty() {
            html! {}
        } else {
            html! { <ContextMenu x={x} y={y} items={items} on_close={on_close_menu.clone()} /> }
        }
    } else {
        html! {}
    };

    // Add files button for the section header
    let add_btn = if let Some(on_add) = &props.on_add_files {
        let on_add = on_add.clone();
        html! {
            <button type="button" class="explorer-add-folder-btn" onclick={Callback::from(move |e: MouseEvent| { e.stop_propagation(); on_add.emit(()); })} title="Add files">
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
            {context_menu_html}
        </>
    }
}
