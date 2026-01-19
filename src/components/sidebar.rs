use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq)]
pub struct SidebarProps {
    /// Callback when a nav item is clicked; argument is a simple route token
    pub on_navigate: Callback<&'static str>,
    pub current_page: String,
    pub has_active_project: bool,
}

#[function_component(Sidebar)]
pub fn sidebar(props: &SidebarProps) -> Html {
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

    html! {
        <aside class="sidebar">
            if props.has_active_project {
                if props.current_page == "montage" {
                    <button class={get_btn_class("montage", &props.current_page)} title="Montage" type="button" onclick={nav("montage")}>
                        <Icon icon_id={IconId::LucideFilm} width={"28"} height={"28"} />
                    </button>
                } else {
                    <button class={get_btn_class("project", &props.current_page)} title="Project" type="button" onclick={nav("project")}>
                        <Icon icon_id={IconId::LucideBrush} width={"28"} height={"28"} />
                    </button>
                }
            }
            <button class={get_btn_class("home", &props.current_page)} title="Home" type="button" onclick={nav("home")}>
                <Icon icon_id={IconId::LucideHome} width={"28"} height={"28"} />
            </button>
            <button class={get_btn_class("new", &props.current_page)} title="New" type="button" onclick={nav("new")}>
                <Icon icon_id={IconId::LucidePlus} width={"28"} height={"28"} />
            </button>
            <button class={get_btn_class("open", &props.current_page)} title="Open" type="button" onclick={nav("open")}>
                <Icon icon_id={IconId::LucideFolderOpen} width={"28"} height={"28"} />
            </button>
            <button class={get_btn_class("settings", &props.current_page)} title="Settings" type="button" onclick={nav("settings")}>
                <Icon icon_id={IconId::LucideSettings} width={"28"} height={"28"} />
            </button>
            <button class={get_btn_class("library", &props.current_page)} title="Library" type="button" onclick={nav("library")}>
                <Icon icon_id={IconId::LucideLibrary} width={"28"} height={"28"} />
            </button>
            <button class={get_btn_class("sponsor", &props.current_page)} title="Sponsor" type="button" onclick={nav("sponsor")}>
                <Icon icon_id={IconId::LucideHeart} width={"28"} height={"28"} />
            </button>
        </aside>
    }
}
