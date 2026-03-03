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
}

fn tab_icon(resource: &ResourceRef) -> IconId {
    match resource {
        ResourceRef::SourceFile { .. } => IconId::LucideFileVideo,
        ResourceRef::VideoCut { .. } => IconId::LucideScissors,
        ResourceRef::FrameDirectory { .. } => IconId::LucideImage,
        ResourceRef::Preview { .. } => IconId::LucideCamera,
    }
}

#[function_component(TabBar)]
pub fn tab_bar(props: &TabBarProps) -> Html {
    html! {
        <div class="tab-bar" role="tablist" aria-label="Open resources">
            {for props.tabs.iter().map(|tab| {
                let tab_id = tab.id.clone();
                let close_id = tab.id.clone();
                let on_select_tab = props.on_select_tab.clone();
                let on_close_tab = props.on_close_tab.clone();
                let is_active = props.active_tab_id.as_ref().map(|id| id == &tab.id).unwrap_or(false);
                let class_name = classes!("tab", is_active.then_some("tab--active"));
                let icon_id = tab_icon(&tab.resource);

                let on_tab_click = Callback::from(move |_| {
                    on_select_tab.emit(tab_id.clone());
                });

                let on_close_click = Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    on_close_tab.emit(close_id.clone());
                });

                html! {
                    <div
                        class={class_name}
                        role="tab"
                        aria-selected={is_active.to_string()}
                        title={tab.label.clone()}
                        onclick={on_tab_click}
                    >
                        <span class="tab__icon">
                            <Icon icon_id={icon_id} width={"14"} height={"14"} />
                        </span>
                        <span class="tab__label">{tab.label.clone()}</span>
                        <button
                            type="button"
                            class="tab__close"
                            title="Close tab"
                            aria-label="Close tab"
                            onclick={on_close_click}
                        >
                            <Icon icon_id={IconId::LucideX} width={"12"} height={"12"} />
                        </button>
                    </div>
                }
            })}
        </div>
    }
}
