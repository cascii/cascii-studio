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
            <button class="nav-btn" title="Home" type="button" onclick={nav("home")}>      <Icon icon_id={IconId::LucideHome}         width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="New" type="button" onclick={nav("new")}>        <Icon icon_id={IconId::LucidePlus}         width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Open" type="button" onclick={nav("open")}>      <Icon icon_id={IconId::LucideFolderOpen}  width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Settings" type="button" onclick={nav("settings")}><Icon icon_id={IconId::LucideSettings}    width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Library" type="button" onclick={nav("library")}> <Icon icon_id={IconId::LucideLibrary}     width={"28"} height={"28"} /></button>
            <button class="nav-btn" title="Sponsor" type="button" onclick={nav("sponsor")}> <Icon icon_id={IconId::LucideHeart}       width={"28"} height={"28"} /></button>
        </aside>
    }
}
