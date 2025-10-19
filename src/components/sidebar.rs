use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq)]
pub struct SidebarProps {
    /// Callback when a nav item is clicked; argument is a simple route token
    pub on_navigate: Callback<&'static str>,
}

#[function_component(Sidebar)]
pub fn sidebar(props: &SidebarProps) -> Html {
    let nav = |route: &'static str| {
        let cb = props.on_navigate.clone();
        Callback::from(move |_| cb.emit(route))
    };

    html! {
        <aside class="sidebar">
            <button class="nav-btn" title="New" onclick={nav("new")}>        <Icon icon_id={IconId::LucidePlus}        width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Open" onclick={nav("open")}>      <Icon icon_id={IconId::LucideFolderOpen} width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Settings" onclick={nav("settings")}><Icon icon_id={IconId::LucideSettings}   width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Library" onclick={nav("library")}> <Icon icon_id={IconId::LucideLibrary}    width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Heart" onclick={nav("heart")}><Icon icon_id={IconId::LucideHeart}      width={"28"} height={"28"} /></button>
        </aside>
    }
}


