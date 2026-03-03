use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq)]
pub struct TreeSectionProps {
    pub title: String,
    pub is_expanded: bool,
    pub on_toggle: Callback<()>,
    #[prop_or_default]
    pub action_buttons: Option<Html>,
    #[prop_or_default]
    pub children: Children,
}

#[function_component(TreeSection)]
pub fn tree_section(props: &TreeSectionProps) -> Html {
    let on_toggle = {
        let cb = props.on_toggle.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let chevron_class = classes!(
        "tree-section-header__chevron",
        (!props.is_expanded).then_some("tree-section-header__chevron--collapsed"),
    );

    let section_slug = props.title.to_lowercase().replace(' ', "-");
    let section_id = format!("tree-section-{}", section_slug);
    let header_id = format!("tree-section-{}-header", section_slug);
    let content_id = format!("tree-section-{}-content", section_slug);
    let actions_id = format!("tree-section-{}-actions", section_slug);

    html! {
        <div id={section_id} class="tree-section">
            <div id={header_id} class="tree-section-header" onclick={on_toggle}>
                <span class={chevron_class}>
                    <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
                </span>
                <span class="tree-section-header__title">{&props.title}</span>
                {if let Some(actions) = &props.action_buttons {
                    html! {
                        <div id={actions_id} class="tree-section-header__actions" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            {actions.clone()}
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>
            {if props.is_expanded {
                html! {
                    <div id={content_id} class="tree-section__content">
                        { for props.children.iter() }
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
