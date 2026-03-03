use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

use crate::components::explorer::ResourceRef;

#[derive(Clone, Debug, PartialEq)]
pub struct OpenTab {
    pub id: String,
    pub resource: ResourceRef,
    pub label: String,
}

#[derive(Properties, PartialEq)]
pub struct TabBarProps {
    pub tabs: Vec<OpenTab>,
    pub active_tab_id: Option<String>,
    pub on_select_tab: Callback<String>,
    pub on_close_tab: Callback<String>,
    pub on_reorder_tabs: Callback<Vec<String>>,
}

#[derive(Clone)]
struct DragStart {
    tab_id: String,
    start_x: f64,
    start_y: f64,
}

const DRAG_THRESHOLD_PX: f64 = 4.0;

fn tab_icon(resource: &ResourceRef) -> IconId {
    match resource {
        ResourceRef::SourceFile { .. } => IconId::LucideFileVideo,
        ResourceRef::VideoCut { .. } => IconId::LucideScissors,
        ResourceRef::FrameDirectory { .. } => IconId::LucideImage,
        ResourceRef::Preview { .. } => IconId::LucideCamera,
    }
}

fn reorder_tab_ids(
    tabs: &[OpenTab],
    dragged_id: &str,
    target_id: &str,
    insert_before: bool,
) -> Option<Vec<String>> {
    if dragged_id == target_id {
        return None;
    }

    let mut ids: Vec<String> = tabs.iter().map(|tab| tab.id.clone()).collect();
    let from_index = ids.iter().position(|id| id == dragged_id)?;
    let mut target_index = ids.iter().position(|id| id == target_id)?;

    let moved_id = ids.remove(from_index);

    if from_index < target_index {
        target_index = target_index.saturating_sub(1);
    }
    if !insert_before {
        target_index += 1;
    }

    if target_index > ids.len() {
        target_index = ids.len();
    }

    ids.insert(target_index, moved_id);
    Some(ids)
}

fn drop_target_from_position(client_x: i32, dragged_id: &str) -> Option<(String, bool)> {
    let tab_bar = gloo::utils::document().get_element_by_id("tab-bar")?;
    let tabs = tab_bar.query_selector_all(".tab").ok()?;
    let mut last_id: Option<String> = None;

    for idx in 0..tabs.length() {
        let Some(node) = tabs.item(idx) else {
            continue;
        };
        let Ok(tab): Result<web_sys::Element, _> = node.dyn_into() else {
            continue;
        };
        let Some(tab_id) = tab.get_attribute("data-tab-id") else {
            continue;
        };
        if tab_id == dragged_id {
            continue;
        }

        let rect = tab.get_bounding_client_rect();
        let mid_x = rect.left() + (rect.width() / 2.0);
        if (client_x as f64) < mid_x {
            return Some((tab_id, true));
        }
        last_id = Some(tab_id);
    }

    last_id.map(|id| (id, false))
}

#[function_component(TabBar)]
pub fn tab_bar(props: &TabBarProps) -> Html {
    let drag_start_ref = use_mut_ref(|| None::<DragStart>);
    let dragging_ref = use_mut_ref(|| false);
    let suppress_click_ref = use_mut_ref(|| false);
    let tabs_ref = use_mut_ref(|| props.tabs.clone());
    let on_reorder_tabs_ref = use_mut_ref(|| props.on_reorder_tabs.clone());
    let dragged_tab_id = use_state(|| None::<String>);
    let is_dragging = use_state(|| false);
    let drop_target = use_state(|| None::<(String, bool)>);

    {
        let mut tabs = tabs_ref.borrow_mut();
        *tabs = props.tabs.clone();
    }
    {
        let mut on_reorder_tabs = on_reorder_tabs_ref.borrow_mut();
        *on_reorder_tabs = props.on_reorder_tabs.clone();
    }

    let on_tab_bar_mouse_move = {
        let drag_start_ref = drag_start_ref.clone();
        let dragging_ref = dragging_ref.clone();
        let dragged_tab_id = dragged_tab_id.clone();
        let is_dragging = is_dragging.clone();
        let drop_target = drop_target.clone();
        Callback::from(move |e: MouseEvent| {
            let Some(drag_start) = drag_start_ref.borrow().clone() else {
                return;
            };

            // Mouse released outside the tab bar: reset drag state once the pointer re-enters.
            if e.buttons() & 1 == 0 {
                drag_start_ref.borrow_mut().take();
                *dragging_ref.borrow_mut() = false;
                dragged_tab_id.set(None);
                is_dragging.set(false);
                drop_target.set(None);
                return;
            }

            let dx = e.client_x() as f64 - drag_start.start_x;
            let dy = e.client_y() as f64 - drag_start.start_y;
            let distance = (dx * dx + dy * dy).sqrt();

            let was_dragging = *dragging_ref.borrow();
            if !was_dragging && distance < DRAG_THRESHOLD_PX {
                return;
            }
            if !was_dragging {
                *dragging_ref.borrow_mut() = true;
                is_dragging.set(true);
            }

            let next_target = drop_target_from_position(e.client_x(), &drag_start.tab_id);
            if *drop_target != next_target {
                drop_target.set(next_target);
            }
        })
    };

    let on_tab_bar_mouse_up = {
        let drag_start_ref = drag_start_ref.clone();
        let dragging_ref = dragging_ref.clone();
        let suppress_click_ref = suppress_click_ref.clone();
        let tabs_ref = tabs_ref.clone();
        let on_reorder_tabs_ref = on_reorder_tabs_ref.clone();
        let dragged_tab_id = dragged_tab_id.clone();
        let is_dragging = is_dragging.clone();
        let drop_target = drop_target.clone();
        Callback::from(move |e: MouseEvent| {
            if e.button() != 0 {
                return;
            }

            let Some(drag_start) = drag_start_ref.borrow_mut().take() else {
                return;
            };
            let was_dragging = *dragging_ref.borrow();
            *dragging_ref.borrow_mut() = false;

            if was_dragging {
                *suppress_click_ref.borrow_mut() = true;
            }

            let active_drop_target = (*drop_target).clone();
            drop_target.set(None);
            dragged_tab_id.set(None);
            is_dragging.set(false);

            if !was_dragging {
                return;
            }

            let Some((target_id, insert_before)) = active_drop_target else {
                return;
            };

            let tabs_snapshot = tabs_ref.borrow().clone();
            if let Some(next_order) = reorder_tab_ids(
                &tabs_snapshot,
                &drag_start.tab_id,
                &target_id,
                insert_before,
            ) {
                on_reorder_tabs_ref.borrow().emit(next_order);
            }
        })
    };

    let on_tab_bar_mouse_leave = {
        let drop_target = drop_target.clone();
        Callback::from(move |_| {
            if drop_target.is_some() {
                drop_target.set(None);
            }
        })
    };

    html! {
        <div
            id="tab-bar"
            class="tab-bar"
            role="tablist"
            aria-label="Open resources"
            onmousemove={on_tab_bar_mouse_move}
            onmouseup={on_tab_bar_mouse_up}
            onmouseleave={on_tab_bar_mouse_leave}
        >
            {for props.tabs.iter().map(|tab| {
                let tab_id = tab.id.clone();
                let close_id = tab.id.clone();
                let close_btn_id = format!("{}-close", tab.id);
                let on_select_tab = props.on_select_tab.clone();
                let on_close_tab = props.on_close_tab.clone();
                let is_active = props.active_tab_id.as_ref().map(|id| id == &tab.id).unwrap_or(false);
                let icon_id = tab_icon(&tab.resource);
                let is_dragged = *is_dragging && dragged_tab_id.as_deref() == Some(tab.id.as_str());

                let mut class_name = classes!("tab", is_active.then_some("tab--active"), is_dragged.then_some("tab--dragging"));
                if let Some((drop_id, insert_before)) = &*drop_target {
                    if drop_id == &tab.id {
                        if *insert_before {
                            class_name.push("tab--drop-before");
                        } else {
                            class_name.push("tab--drop-after");
                        }
                    }
                }

                let suppress_click_ref_for_click = suppress_click_ref.clone();
                let on_tab_click = Callback::from(move |_| {
                    let should_suppress_click = *suppress_click_ref_for_click.borrow();
                    if should_suppress_click {
                        *suppress_click_ref_for_click.borrow_mut() = false;
                        return;
                    }
                    on_select_tab.emit(tab_id.clone());
                });

                let on_close_click = Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    on_close_tab.emit(close_id.clone());
                });

                let drag_tab_id = tab.id.clone();
                let drag_start_ref = drag_start_ref.clone();
                let dragging_ref = dragging_ref.clone();
                let suppress_click_ref = suppress_click_ref.clone();
                let dragged_tab_id_state = dragged_tab_id.clone();
                let is_dragging_state = is_dragging.clone();
                let drop_target_state = drop_target.clone();
                let on_mouse_down = Callback::from(move |e: MouseEvent| {
                    if e.button() != 0 {
                        return;
                    }

                    e.prevent_default();
                    *suppress_click_ref.borrow_mut() = false;
                    *drag_start_ref.borrow_mut() = Some(DragStart {
                        tab_id: drag_tab_id.clone(),
                        start_x: e.client_x() as f64,
                        start_y: e.client_y() as f64,
                    });
                    *dragging_ref.borrow_mut() = false;
                    dragged_tab_id_state.set(Some(drag_tab_id.clone()));
                    is_dragging_state.set(false);
                    drop_target_state.set(None);
                });

                let on_close_mouse_down = Callback::from(|e: MouseEvent| {
                    e.stop_propagation();
                });

                html! {
                    <div
                        class={class_name}
                        data-tab-id={tab.id.clone()}
                        role="tab"
                        aria-selected={is_active.to_string()}
                        title={tab.label.clone()}
                        onclick={on_tab_click}
                        onmousedown={on_mouse_down}
                    >
                        <span class="tab__icon">
                            <Icon icon_id={icon_id} width={"14"} height={"14"} />
                        </span>
                        <span class="tab__label">{tab.label.clone()}</span>
                        <button id={close_btn_id} type="button" class="tab__close" title="Close tab" aria-label="Close tab" onclick={on_close_click} onmousedown={on_close_mouse_down}>
                            <Icon icon_id={IconId::LucideX} width={"12"} height={"12"} />
                        </button>
                    </div>
                }
            })}
        </div>
    }
}
