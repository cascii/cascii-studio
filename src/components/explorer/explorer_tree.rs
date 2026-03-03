use yew::prelude::*;
use yew_icons::IconId;

use crate::pages::project::{SourceContent, FrameDirectory, Preview};
use crate::components::settings::available_cuts::VideoCut;
use super::explorer_types::*;
use super::tree_node::TreeNodeView;
use super::tree_section::TreeSection;
use super::context_menu::{ContextMenu, ContextMenuItem};

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
        ResourceRef::SourceFile { source_id } => {
            source_files.iter()
                .find(|f| f.id == *source_id)
                .map(|f| {
                    f.custom_name.as_ref()
                        .cloned()
                        .unwrap_or_else(|| {
                            std::path::Path::new(&f.file_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(&f.file_path)
                                .to_string()
                        })
                })
                .unwrap_or_else(|| "(missing)".to_string())
        }
        ResourceRef::VideoCut { cut_id } => {
            video_cuts.iter()
                .find(|c| c.id == *cut_id)
                .map(|c| {
                    c.custom_name.as_ref()
                        .cloned()
                        .unwrap_or_else(|| {
                            let sm = (c.start_time / 60.0) as u32;
                            let ss = (c.start_time % 60.0) as u32;
                            let em = (c.end_time / 60.0) as u32;
                            let es = (c.end_time % 60.0) as u32;
                            format!("Cut {:02}:{:02} - {:02}:{:02}", sm, ss, em, es)
                        })
                })
                .unwrap_or_else(|| "(missing)".to_string())
        }
        ResourceRef::FrameDirectory { directory_path } => {
            frame_directories.iter()
                .find(|d| d.directory_path == *directory_path)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "(missing)".to_string())
        }
        ResourceRef::Preview { preview_id } => {
            previews.iter()
                .find(|p| p.id == *preview_id)
                .map(|p| {
                    p.custom_name.as_ref()
                        .cloned()
                        .unwrap_or_else(|| p.folder_name.clone())
                })
                .unwrap_or_else(|| "(missing)".to_string())
        }
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
) -> Vec<TreeNode> {
    items.iter().map(|item| {
        match item {
            ExplorerItem::Folder { id, name, children, is_expanded } => {
                let node_id = format!("exp:folder:{}", id);
                TreeNode {
                    id: TreeNodeId(node_id.clone()),
                    label: name.clone(),
                    node_kind: NodeKind::Folder { is_user_created: true },
                    depth,
                    is_expanded: *is_expanded,
                    is_selected: selected_id.as_ref().map(|s| s.0 == node_id).unwrap_or(false),
                    is_rename_active: false,
                    children: items_to_nodes(children, depth + 1, source_files, video_cuts, frame_directories, previews, selected_id),
                }
            }
            ExplorerItem::ResourceRef(resource) => {
                let label = resolve_label(resource, source_files, video_cuts, frame_directories, previews);
                let node_id = match resource {
                    ResourceRef::SourceFile { source_id } => format!("exp:source:{}", source_id),
                    ResourceRef::VideoCut { cut_id } => format!("exp:cut:{}", cut_id),
                    ResourceRef::FrameDirectory { directory_path } => format!("exp:framedir:{}", directory_path),
                    ResourceRef::Preview { preview_id } => format!("exp:preview:{}", preview_id),
                };
                TreeNode {
                    id: TreeNodeId(node_id.clone()),
                    label,
                    node_kind: NodeKind::Leaf(resource.clone()),
                    depth,
                    is_expanded: false,
                    is_selected: selected_id.as_ref().map(|s| s.0 == node_id).unwrap_or(false),
                    is_rename_active: false,
                    children: vec![],
                }
            }
        }
    }).collect()
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

/// Remove an item from the explorer layout by matching on node id.
fn remove_item(items: &mut Vec<ExplorerItem>, target_id: &str) -> bool {
    let initial_len = items.len();
    items.retain(|item| {
        match item {
            ExplorerItem::Folder { id, .. } => format!("exp:folder:{}", id) != target_id,
            ExplorerItem::ResourceRef(r) => {
                let item_id = match r {
                    ResourceRef::SourceFile { source_id } => format!("exp:source:{}", source_id),
                    ResourceRef::VideoCut { cut_id } => format!("exp:cut:{}", cut_id),
                    ResourceRef::FrameDirectory { directory_path } => format!("exp:framedir:{}", directory_path),
                    ResourceRef::Preview { preview_id } => format!("exp:preview:{}", preview_id),
                };
                item_id != target_id
            }
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

/// Toggle the expanded state of a folder by id.
fn toggle_folder(items: &mut Vec<ExplorerItem>, folder_id: &str) -> bool {
    for item in items.iter_mut() {
        if let ExplorerItem::Folder { id, is_expanded, children, .. } = item {
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
    let context_menu_state = use_state(|| None::<(TreeNodeId, i32, i32)>);

    let tree = items_to_nodes(
        &props.explorer_layout.root_items,
        1,
        &props.source_files,
        &props.video_cuts,
        &props.frame_directories,
        &props.previews,
        &props.selected_node_id,
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
            let id_str = &id.0;
            if let Some(source_id) = id_str.strip_prefix("exp:source:") {
                if let Some(src) = source_files.iter().find(|f| f.id == source_id) {
                    on_select_source.emit(src.clone());
                }
            } else if let Some(cut_id) = id_str.strip_prefix("exp:cut:") {
                if let Some(cut) = video_cuts.iter().find(|c| c.id == cut_id) {
                    on_select_cut.emit(cut.clone());
                }
            } else if let Some(dir_path) = id_str.strip_prefix("exp:framedir:") {
                if let Some(fd) = frame_directories.iter().find(|d| d.directory_path == dir_path) {
                    on_select_frame_dir.emit(fd.clone());
                }
            } else if let Some(preview_id) = id_str.strip_prefix("exp:preview:") {
                if let Some(p) = previews.iter().find(|pr| pr.id == preview_id) {
                    on_select_preview.emit(p.clone());
                }
            }
        })
    };

    let on_context_menu = {
        let ctx_state = context_menu_state.clone();
        Callback::from(move |(id, e): (TreeNodeId, MouseEvent)| {
            ctx_state.set(Some((id, e.client_x(), e.client_y())));
        })
    };

    let on_close_menu = {
        let ctx_state = context_menu_state.clone();
        Callback::from(move |_| {
            ctx_state.set(None);
        })
    };

    // Context menu
    let context_menu_html = if let Some((ref ctx_id, x, y)) = *context_menu_state {
        let mut items = Vec::new();

        if ctx_id.0.starts_with("exp:folder:") {
            // Folder context menu: Delete folder
            let layout = props.explorer_layout.clone();
            let on_layout_change = props.on_layout_change.clone();
            let target_id = ctx_id.0.clone();
            let close = on_close_menu.clone();
            items.push(ContextMenuItem {
                label: "Delete Folder".to_string(),
                icon: IconId::LucideTrash2,
                on_click: Callback::from(move |_| {
                    let mut new_layout = layout.clone();
                    remove_item(&mut new_layout.root_items, &target_id);
                    on_layout_change.emit(new_layout);
                    close.emit(());
                }),
                is_destructive: true,
            });
        } else {
            // Resource ref: remove from explorer
            let layout = props.explorer_layout.clone();
            let on_layout_change = props.on_layout_change.clone();
            let target_id = ctx_id.0.clone();
            let close = on_close_menu.clone();
            items.push(ContextMenuItem {
                label: "Remove from Explorer".to_string(),
                icon: IconId::LucideMinus,
                on_click: Callback::from(move |_| {
                    let mut new_layout = layout.clone();
                    remove_item(&mut new_layout.root_items, &target_id);
                    on_layout_change.emit(new_layout);
                    close.emit(());
                }),
                is_destructive: false,
            });
        }

        html! { <ContextMenu x={x} y={y} items={items} on_close={on_close_menu.clone()} /> }
    } else {
        html! {}
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

    html! {
        <>
            <TreeSection title="EXPLORER" is_expanded={props.is_expanded} on_toggle={on_toggle} action_buttons={Some(add_folder_btn)}>
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
                                />
                            }
                        })}
                    }
                }}
            </TreeSection>
            {context_menu_html}
        </>
    }
}
