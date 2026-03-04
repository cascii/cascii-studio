use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

const ACTION_BUTTON_WIDTH_PX: usize = 32;
const ACTIONS_CONTAINER_WIDTH_PX: usize = 260;
const ACTIONS_HORIZONTAL_PADDING_PX: usize = 8;
const LEFT_WINDOW_CONTROLS_SAFE_WIDTH_PX: usize = 76;

#[derive(Properties, PartialEq)]
pub struct SidebarProps {
    /// Callback when a nav item is clicked; argument is a simple route token
    pub on_navigate: Callback<&'static str>,
    pub current_page: String,
    pub context_label: String,
    pub has_active_project: bool,
    pub explorer_on_left: bool,
    pub on_toggle_explorer_side: Callback<()>,
}

#[derive(Clone)]
enum SidebarActionKind {
    Navigate(&'static str),
    ToggleExplorerSide,
}

#[derive(Clone)]
enum SidebarActionIcon {
    Lucide(IconId),
    ToggleLayout,
}

#[derive(Clone)]
struct SidebarAction {
    id: &'static str,
    title: String,
    icon: SidebarActionIcon,
    kind: SidebarActionKind,
    route: Option<&'static str>,
}

fn render_toggle_layout_icon(explorer_on_left: bool) -> Html {
    if explorer_on_left {
        html! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="3.5" y="5.5" width="17" height="13" rx="1.5"></rect>
                <rect x="13.5" y="6.5" width="6.0" height="11.0" rx="0.8" fill="currentColor" opacity="0.65" stroke="none"></rect>
            </svg>
        }
    } else {
        html! {
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="3.5" y="5.5" width="17" height="13" rx="1.5"></rect>
                <rect x="4.5" y="6.5" width="6.0" height="11.0" rx="0.8" fill="currentColor" opacity="0.65" stroke="none"></rect>
            </svg>
        }
    }
}

fn render_action_icon(icon: &SidebarActionIcon, explorer_on_left: bool) -> Html {
    match icon {
        SidebarActionIcon::Lucide(icon_id) => {
            html! { <Icon icon_id={*icon_id} width={"18"} height={"18"} /> }
        }
        SidebarActionIcon::ToggleLayout => render_toggle_layout_icon(explorer_on_left),
    }
}

#[function_component(Sidebar)]
pub fn sidebar(props: &SidebarProps) -> Html {
    let is_more_open = use_state(|| false);

    {
        let is_more_open = is_more_open.clone();
        use_effect_with(*is_more_open, move |open| {
            let listener = if *open {
                let is_more_open = is_more_open.clone();
                Some(EventListener::new(
                    &gloo::utils::document(),
                    "mousedown",
                    move |event| {
                        let Some(target) = event.target() else {
                            return;
                        };
                        let Ok(target_element) = target.dyn_into::<web_sys::Element>() else {
                            return;
                        };

                        let clicked_inside_menu = target_element
                            .closest("#sidebar-overflow-menu-container")
                            .ok()
                            .flatten()
                            .is_some();

                        if !clicked_inside_menu {
                            is_more_open.set(false);
                        }
                    },
                ))
            } else {
                None
            };

            move || drop(listener)
        });
    }

    let nav = |route: &'static str| {
        let cb = props.on_navigate.clone();
        Callback::from(move |_| cb.emit(route))
    };

    let get_btn_class = |route: &'static str, current_page: &str| {
        if current_page == route {
            "nav-btn active"
        } else {
            "nav-btn"
        }
    };

    let mut actions: Vec<SidebarAction> = Vec::new();
    let toggle_title = if props.explorer_on_left {
        "Move Explorer Right"
    } else {
        "Move Explorer Left"
    };

    if props.has_active_project {
        if props.current_page == "montage" {
            actions.push(SidebarAction {
                id: "sidebar-montage-btn",
                title: "Montage".to_string(),
                icon: SidebarActionIcon::Lucide(IconId::LucideFilm),
                kind: SidebarActionKind::Navigate("montage"),
                route: Some("montage"),
            });
        } else {
            actions.push(SidebarAction {
                id: "sidebar-project-btn",
                title: "Project".to_string(),
                icon: SidebarActionIcon::Lucide(IconId::LucideBrush),
                kind: SidebarActionKind::Navigate("project"),
                route: Some("project"),
            });
        }
    }

    actions.extend([
        SidebarAction {
            id: "sidebar-home-btn",
            title: "Home".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucideHome),
            kind: SidebarActionKind::Navigate("home"),
            route: Some("home"),
        },
        SidebarAction {
            id: "sidebar-new-btn",
            title: "New".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucidePlus),
            kind: SidebarActionKind::Navigate("new"),
            route: Some("new"),
        },
        SidebarAction {
            id: "sidebar-open-btn",
            title: "Open".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucideFolderOpen),
            kind: SidebarActionKind::Navigate("open"),
            route: Some("open"),
        },
        SidebarAction {
            id: "sidebar-settings-btn",
            title: "Settings".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucideSettings),
            kind: SidebarActionKind::Navigate("settings"),
            route: Some("settings"),
        },
        SidebarAction {
            id: "sidebar-library-btn",
            title: "Library".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucideLibrary),
            kind: SidebarActionKind::Navigate("library"),
            route: Some("library"),
        },
        SidebarAction {
            id: "sidebar-sponsor-btn",
            title: "Sponsor".to_string(),
            icon: SidebarActionIcon::Lucide(IconId::LucideHeart),
            kind: SidebarActionKind::Navigate("sponsor"),
            route: Some("sponsor"),
        },
        SidebarAction {
            id: "sidebar-toggle-explorer-side-btn",
            title: toggle_title.to_string(),
            icon: SidebarActionIcon::ToggleLayout,
            kind: SidebarActionKind::ToggleExplorerSide,
            route: None,
        },
    ]);

    let safe_offset = if props.explorer_on_left {
        LEFT_WINDOW_CONTROLS_SAFE_WIDTH_PX
    } else {
        0
    };
    let available_px =
        ACTIONS_CONTAINER_WIDTH_PX.saturating_sub(ACTIONS_HORIZONTAL_PADDING_PX + safe_offset);
    let max_slots = (available_px / ACTION_BUTTON_WIDTH_PX).max(1);
    let has_overflow = actions.len() > max_slots;
    let (visible_actions, overflow_actions) = if !has_overflow {
        (actions.clone(), Vec::new())
    } else {
        let visible_slots = max_slots.saturating_sub(1); // reserve one slot for "..."
        let hidden_count = actions.len().saturating_sub(visible_slots);

        // Requested priority: put Settings, Sponsor, and Home in overflow first.
        let overflow_priority = [
            "sidebar-settings-btn",
            "sidebar-sponsor-btn",
            "sidebar-home-btn",
            "sidebar-project-btn",
            "sidebar-montage-btn",
            "sidebar-library-btn",
            "sidebar-open-btn",
            "sidebar-new-btn",
            "sidebar-toggle-explorer-side-btn",
        ];

        let mut hidden_ids: Vec<&'static str> = Vec::new();
        for id in overflow_priority {
            if hidden_ids.len() >= hidden_count {
                break;
            }
            if actions.iter().any(|action| action.id == id) {
                hidden_ids.push(id);
            }
        }

        if hidden_ids.len() < hidden_count {
            for action in &actions {
                if hidden_ids.len() >= hidden_count {
                    break;
                }
                if !hidden_ids.contains(&action.id) {
                    hidden_ids.push(action.id);
                }
            }
        }

        let mut visible: Vec<SidebarAction> = Vec::new();
        for action in &actions {
            if !hidden_ids.contains(&action.id) {
                visible.push(action.clone());
            }
        }

        let mut hidden: Vec<SidebarAction> = Vec::new();
        for id in &hidden_ids {
            if let Some(action) = actions.iter().find(|action| action.id == *id) {
                hidden.push(action.clone());
            }
        }

        (visible, hidden)
    };

    let on_toggle_more = {
        let is_more_open = is_more_open.clone();
        Callback::from(move |_| is_more_open.set(!*is_more_open))
    };

    html! {
        <aside id="sidebar" class={classes!("sidebar", props.explorer_on_left.then_some("sidebar--explorer-left"))}>
            <div id="sidebar-drag-region" class="sidebar__drag-region" data-tauri-drag-region="" aria-hidden="true"></div>
            <div id="sidebar-center-title" class="sidebar__center-title" data-tauri-drag-region="" title={props.context_label.clone()}>
                <span id="sidebar-center-title-text" class="sidebar__center-title-text">{props.context_label.clone()}</span>
            </div>
            <nav id="sidebar-actions" class="sidebar__actions" aria-label="Primary Navigation">
                {
                    for visible_actions.iter().map(|action| {
                        let route = action.route;
                        let class_name = route
                            .map(|r| get_btn_class(r, &props.current_page))
                            .unwrap_or("nav-btn");

                        let onclick = match action.kind {
                            SidebarActionKind::Navigate(route) => nav(route),
                            SidebarActionKind::ToggleExplorerSide => {
                                let cb = props.on_toggle_explorer_side.clone();
                                Callback::from(move |_| cb.emit(()))
                            }
                        };

                        html! {
                            <button
                                id={action.id}
                                class={class_name}
                                title={action.title.clone()}
                                aria-label={action.title.clone()}
                                type="button"
                                onclick={onclick}
                            >
                                {render_action_icon(&action.icon, props.explorer_on_left)}
                            </button>
                        }
                    })
                }

                if !overflow_actions.is_empty() {
                    <div id="sidebar-overflow-menu-container" class="sidebar__more" onmousedown={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <button
                            id="sidebar-overflow-btn"
                            class="nav-btn"
                            title="More"
                            aria-label="More"
                            type="button"
                            onclick={on_toggle_more}
                        >
                            <span class="sidebar__more-dots" aria-hidden="true">{"..."}</span>
                        </button>
                        if *is_more_open {
                            <div id="sidebar-overflow-menu" class="sidebar__more-menu" role="menu" aria-label="More navigation actions">
                                {
                                    for overflow_actions.iter().map(|action| {
                                        let is_active = action
                                            .route
                                            .map(|route| route == props.current_page)
                                            .unwrap_or(false);

                                        let class = classes!("sidebar__more-item", is_active.then_some("sidebar__more-item--active"));
                                        let is_more_open = is_more_open.clone();
                                        let onclick = match action.kind {
                                            SidebarActionKind::Navigate(route) => {
                                                let cb = props.on_navigate.clone();
                                                Callback::from(move |_| {
                                                    cb.emit(route);
                                                    is_more_open.set(false);
                                                })
                                            }
                                            SidebarActionKind::ToggleExplorerSide => {
                                                let cb = props.on_toggle_explorer_side.clone();
                                                Callback::from(move |_| {
                                                    cb.emit(());
                                                    is_more_open.set(false);
                                                })
                                            }
                                        };

                                        html! {
                                            <button
                                                id={format!("{}-overflow-item", action.id)}
                                                type="button"
                                                role="menuitem"
                                                class={class}
                                                onclick={onclick}
                                            >
                                                <span class="sidebar__more-item-icon">
                                                    {render_action_icon(&action.icon, props.explorer_on_left)}
                                                </span>
                                                <span class="sidebar__more-item-label">{action.title.clone()}</span>
                                            </button>
                                        }
                                    })
                                }
                            </div>
                        }
                    </div>
                }
            </nav>
        </aside>
    }
}
