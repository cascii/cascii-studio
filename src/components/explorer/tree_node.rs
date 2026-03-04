use web_sys::DragEvent;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

use super::explorer_types::{NodeKind, ResourceRef, TreeNode, TreeNodeId};

#[derive(Properties, PartialEq)]
pub struct TreeNodeProps {
    pub node: TreeNode,
    pub on_toggle_expand: Callback<TreeNodeId>,
    pub on_select: Callback<TreeNodeId>,
    pub on_context_menu: Callback<(TreeNodeId, MouseEvent)>,
    #[prop_or_default]
    pub on_rename_submit: Option<Callback<(TreeNodeId, String)>>,
    #[prop_or_default]
    pub on_rename_cancel: Option<Callback<TreeNodeId>>,
    #[prop_or_default]
    pub on_rename_input: Option<Callback<(TreeNodeId, String)>>,
    #[prop_or_default]
    pub rename_value: Option<String>,
    #[prop_or_default]
    pub current_folder_id: Option<TreeNodeId>,
    #[prop_or_default]
    pub on_mouse_down: Option<Callback<TreeNodeId>>,
    #[prop_or_default]
    pub on_drag_start: Option<Callback<(TreeNodeId, DragEvent)>>,
    #[prop_or_default]
    pub on_drag_end: Option<Callback<()>>,
    #[prop_or_default]
    pub on_drag: Option<Callback<(TreeNodeId, DragEvent)>>,
    #[prop_or_default]
    pub on_drag_over: Option<Callback<(TreeNodeId, DragEvent)>>,
    #[prop_or_default]
    pub on_drag_leave: Option<Callback<TreeNodeId>>,
    #[prop_or_default]
    pub on_drop: Option<Callback<(TreeNodeId, DragEvent)>>,
    #[prop_or_default]
    pub drop_target_id: Option<TreeNodeId>,
}

/// Choose an icon based on the node kind.
fn node_icon(node: &TreeNode) -> IconId {
    match &node.node_kind {
        NodeKind::Folder { .. } => {
            if node.is_expanded {
                IconId::LucideFolderOpen
            } else {
                IconId::LucideFolder
            }
        }
        NodeKind::Leaf(resource_ref) => match resource_ref {
            ResourceRef::SourceFile { .. } => IconId::LucideFileVideo,
            ResourceRef::VideoCut { .. } => IconId::LucideScissors,
            ResourceRef::FrameDirectory { .. } => IconId::LucideImage,
            ResourceRef::Preview { .. } => IconId::LucideCamera,
        },
    }
}

#[function_component(TreeNodeView)]
pub fn tree_node_view(props: &TreeNodeProps) -> Html {
    let node = &props.node;
    let indent_px = node.depth * 20;

    // Click on chevron -> toggle expand
    let on_chevron_click = {
        let id = node.id.clone();
        let on_toggle = props.on_toggle_expand.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_toggle.emit(id.clone());
        })
    };

    // Click on row -> select
    let on_row_click = {
        let id = node.id.clone();
        let on_select = props.on_select.clone();
        let on_toggle = props.on_toggle_expand.clone();
        let is_folder = node.is_folder();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_select.emit(id.clone());
            if is_folder {
                on_toggle.emit(id.clone());
            }
        })
    };

    // Right-click -> context menu
    let on_ctx = {
        let id = node.id.clone();
        let on_context_menu = props.on_context_menu.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            e.stop_propagation();
            on_context_menu.emit((id.clone(), e));
        })
    };

    let row_class = classes!(
        "tree-node",
        node.is_selected.then_some("tree-node--selected"),
        props
            .drop_target_id
            .as_ref()
            .map(|id| id.0 == node.id.0)
            .unwrap_or(false)
            .then_some("tree-node--drop-target"),
    );

    let icon_id = node_icon(node);

    // Chevron for folders, spacer for leaves
    let chevron_html = if node.is_folder() {
        let chevron_class = classes!(
            "tree-node__chevron",
            node.is_expanded.then_some("tree-node__chevron--expanded"),
        );
        html! {
            <span class={chevron_class} onclick={on_chevron_click}>
                <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
            </span>
        }
    } else {
        html! { <span class="tree-node__chevron-spacer"></span> }
    };

    // Label or rename input
    let label_html = if node.is_rename_active {
        let rename_val = props
            .rename_value
            .clone()
            .unwrap_or_else(|| node.label.clone());
        let node_id = node.id.clone();
        let node_id2 = node.id.clone();
        let on_rename_submit = props.on_rename_submit.clone();
        let on_rename_cancel = props.on_rename_cancel.clone();
        let on_rename_input = props.on_rename_input.clone();

        let on_input = {
            let node_id = node_id.clone();
            Callback::from(move |e: InputEvent| {
                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                if let Some(ref cb) = on_rename_input {
                    cb.emit((node_id.clone(), input.value()));
                }
            })
        };

        let on_keydown = {
            let node_id = node_id.clone();
            let on_submit = on_rename_submit.clone();
            let on_cancel = on_rename_cancel.clone();
            Callback::from(move |e: KeyboardEvent| {
                if e.key() == "Enter" {
                    e.prevent_default();
                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                    if let Some(ref cb) = on_submit {
                        cb.emit((node_id.clone(), input.value()));
                    }
                } else if e.key() == "Escape" {
                    e.prevent_default();
                    if let Some(ref cb) = on_cancel {
                        cb.emit(node_id.clone());
                    }
                }
            })
        };

        let on_blur = {
            Callback::from(move |_| {
                if let Some(ref cb) = on_rename_cancel {
                    cb.emit(node_id2.clone());
                }
            })
        };

        html! {
            <input
                class="tree-node__rename-input"
                type="text"
                value={rename_val}
                oninput={on_input}
                onkeydown={on_keydown}
                onblur={on_blur}
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                autofocus=true
            />
        }
    } else {
        html! { <span class="tree-node__label">{&node.label}</span> }
    };

    // Render children recursively if expanded
    let children_html = if node.is_folder() && node.is_expanded {
        let next_folder_id = Some(node.id.clone());
        html! {
            { for node.children.iter().map(|child| {
                html! {
                    <TreeNodeView
                        node={child.clone()}
                        on_toggle_expand={props.on_toggle_expand.clone()}
                        on_select={props.on_select.clone()}
                        on_context_menu={props.on_context_menu.clone()}
                        on_rename_submit={props.on_rename_submit.clone()}
                        on_rename_cancel={props.on_rename_cancel.clone()}
                        on_rename_input={props.on_rename_input.clone()}
                        rename_value={props.rename_value.clone()}
                        current_folder_id={next_folder_id.clone()}
                        on_mouse_down={props.on_mouse_down.clone()}
                        on_drag_start={props.on_drag_start.clone()}
                        on_drag_end={props.on_drag_end.clone()}
                        on_drag={props.on_drag.clone()}
                        on_drag_over={props.on_drag_over.clone()}
                        on_drag_leave={props.on_drag_leave.clone()}
                        on_drop={props.on_drop.clone()}
                        drop_target_id={props.drop_target_id.clone()}
                    />
                }
            })}
        }
    } else {
        html! {}
    };

    let row_id = format!("tree-node-{}", node.id.0);
    let is_draggable = props.on_drag_start.is_some() && matches!(node.node_kind, NodeKind::Leaf(_));

    let on_drag_start = {
        let id = node.id.clone();
        let on_drag_start = props.on_drag_start.clone();
        Callback::from(move |e: DragEvent| {
            if let Some(ref cb) = on_drag_start {
                cb.emit((id.clone(), e));
            }
        })
    };

    let on_drag_end = {
        let on_drag_end = props.on_drag_end.clone();
        Callback::from(move |_| {
            if let Some(ref cb) = on_drag_end {
                cb.emit(());
            }
        })
    };

    let on_drag = {
        let id = node.id.clone();
        let on_drag = props.on_drag.clone();
        Callback::from(move |e: DragEvent| {
            if let Some(ref cb) = on_drag {
                cb.emit((id.clone(), e));
            }
        })
    };

    let on_drag_over = {
        let id = node.id.clone();
        let on_drag_over = props.on_drag_over.clone();
        Callback::from(move |e: DragEvent| {
            if let Some(ref cb) = on_drag_over {
                cb.emit((id.clone(), e));
            }
        })
    };

    let on_drag_leave = {
        let id = node.id.clone();
        let on_drag_leave = props.on_drag_leave.clone();
        Callback::from(move |_| {
            if let Some(ref cb) = on_drag_leave {
                cb.emit(id.clone());
            }
        })
    };

    let on_drop = {
        let id = node.id.clone();
        let on_drop = props.on_drop.clone();
        Callback::from(move |e: DragEvent| {
            if let Some(ref cb) = on_drop {
                cb.emit((id.clone(), e));
            }
        })
    };

    let on_mouse_down = {
        let id = node.id.clone();
        let on_mouse_down = props.on_mouse_down.clone();
        Callback::from(move |_| {
            if let Some(ref cb) = on_mouse_down {
                cb.emit(id.clone());
            }
        })
    };

    let drop_folder_id = if node.is_folder() {
        node.id.0.clone()
    } else {
        props
            .current_folder_id
            .as_ref()
            .map(|id| id.0.clone())
            .unwrap_or_default()
    };

    html! {
        <>
            <div id={row_id} class={row_class} onclick={on_row_click} oncontextmenu={on_ctx}
                 data-node-id={node.id.0.clone()}
                 data-drop-folder-id={drop_folder_id}
                 draggable={if is_draggable { "true" } else { "false" }}
                 onmousedown={on_mouse_down}
                 ondragstart={on_drag_start}
                 ondragend={on_drag_end}
                 ondrag={on_drag}
                 ondragover={on_drag_over}
                 ondragleave={on_drag_leave}
                 ondrop={on_drop}
                 style={format!("padding-left: {}px;", indent_px)}>
                {chevron_html}
                <span class="tree-node__icon">
                    <Icon icon_id={icon_id} width={"16"} height={"16"} />
                </span>
                {label_html}
            </div>
            {children_html}
        </>
    }
}
