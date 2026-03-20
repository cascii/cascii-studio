use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq)]
pub struct ExportSectionProps {
    #[prop_or_default]
    pub on_export_mp4: Callback<()>,
    #[prop_or_default]
    pub on_export_project: Callback<()>,
}

#[function_component(ExportSection)]
pub fn export_section(props: &ExportSectionProps) -> Html {
    let collapsed = use_state(|| false);

    let on_toggle = {
        let collapsed = collapsed.clone();
        Callback::from(move |_| {
            collapsed.set(!*collapsed);
        })
    };

    let on_mp4 = {
        let cb = props.on_export_mp4.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let on_project = {
        let cb = props.on_export_project.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    html! {
        <div id="export-section" class="export-section">
            <div id="export-header" class="tree-section-header" onclick={on_toggle}>
                <span class={classes!("tree-section-header__chevron", (*collapsed).then_some("tree-section-header__chevron--collapsed"))}>
                    <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
                </span>
                <span class="tree-section-header__title">{"EXPORT"}</span>
            </div>
            if !*collapsed {
                <div id="export-content" class="export-section__content">
                    <button
                        id="export-mp4-btn"
                        class="ctrl-btn"
                        type="button"
                        onclick={on_mp4}
                        title="Export to MP4"
                    >
                        <Icon icon_id={IconId::LucideFilm} width={"16"} height={"16"} />
                    </button>
                    <button
                        id="export-project-btn"
                        class="ctrl-btn"
                        type="button"
                        onclick={on_project}
                        title="Export project files"
                    >
                        <Icon icon_id={IconId::LucideFolderOpen} width={"16"} height={"16"} />
                    </button>
                </div>
            }
        </div>
    }
}
