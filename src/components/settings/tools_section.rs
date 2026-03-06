use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq)]
pub struct ToolsSectionProps {
    pub on_navigate: Callback<&'static str>,
    pub current_page: AttrValue,
}

#[function_component(ToolsSection)]
pub fn tools_section(props: &ToolsSectionProps) -> Html {
    let tools_collapsed = use_state(|| false);

    let is_project = props.current_page == "project";
    let is_montage = props.current_page == "montage";

    let on_toggle = {
        let tools_collapsed = tools_collapsed.clone();
        Callback::from(move |_| {
            tools_collapsed.set(!*tools_collapsed);
        })
    };

    let nav_project = {
        let on_navigate = props.on_navigate.clone();
        let is_project = is_project;
        Callback::from(move |_: MouseEvent| {
            if !is_project {
                on_navigate.emit("project");
            }
        })
    };

    let nav_montage = {
        let on_navigate = props.on_navigate.clone();
        let is_montage = is_montage;
        Callback::from(move |_: MouseEvent| {
            if !is_montage {
                on_navigate.emit("montage");
            }
        })
    };

    html! {
        <div id="tools-section" class="tools-section">
            <div id="tools-header" class="tree-section-header" onclick={on_toggle}>
                <span class={classes!("tree-section-header__chevron", (*tools_collapsed).then_some("tree-section-header__chevron--collapsed"))}>
                    <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
                </span>
                <span class="tree-section-header__title">{"TOOLS"}</span>
            </div>
            if !*tools_collapsed {
                <div id="tools-content" class="tools-section__content">
                    <button
                        id="tools-project-btn"
                        class={classes!("ctrl-btn", is_project.then_some("active"))}
                        type="button"
                        onclick={nav_project}
                        title="Project"
                    >
                        <Icon icon_id={IconId::LucideBrush} width={"16"} height={"16"} />
                    </button>
                    <button
                        id="tools-montage-btn"
                        class={classes!("ctrl-btn", is_montage.then_some("active"))}
                        type="button"
                        onclick={nav_montage}
                        title="Montage"
                    >
                        <Icon icon_id={IconId::LucideFilm} width={"16"} height={"16"} />
                    </button>
                </div>
            }
        </div>
    }
}
