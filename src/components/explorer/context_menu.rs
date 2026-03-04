use gloo::events::EventListener;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Clone, Debug, PartialEq)]
pub struct ContextMenuItem {
    pub label: String,
    pub icon: IconId,
    pub on_click: Callback<()>,
    pub is_destructive: bool,
}

#[derive(Properties, PartialEq)]
pub struct ContextMenuProps {
    pub x: i32,
    pub y: i32,
    pub items: Vec<ContextMenuItem>,
    pub on_close: Callback<()>,
}

#[function_component(ContextMenu)]
pub fn context_menu(props: &ContextMenuProps) -> Html {
    let on_close = props.on_close.clone();

    // Close on click outside
    {
        let on_close = on_close.clone();
        use_effect_with((), move |_| {
            let listener = EventListener::new(&gloo::utils::document(), "mousedown", move |_| {
                on_close.emit(());
            });
            move || drop(listener)
        });
    }

    let style = format!("left: {}px; top: {}px;", props.x, props.y);

    html! {
        <div id="explorer-context-menu" class="explorer-context-menu" {style}
             onmousedown={Callback::from(|e: MouseEvent| e.stop_propagation())}>
            { for props.items.iter().enumerate().map(|(i, item)| {
                let on_click = {
                    let item_cb = item.on_click.clone();
                    let on_close = on_close.clone();
                    Callback::from(move |e: MouseEvent| {
                        e.stop_propagation();
                        item_cb.emit(());
                        on_close.emit(());
                    })
                };
                let class = classes!(
                    "explorer-context-menu__item",
                    item.is_destructive.then_some("explorer-context-menu__item--destructive"),
                );
                let item_id = format!("context-menu-item-{}", i);
                html! {
                    <button id={item_id} type="button" {class} onclick={on_click}>
                        <Icon icon_id={item.icon} width={"14"} height={"14"} />
                        <span>{&item.label}</span>
                    </button>
                }
            })}
        </div>
    }
}
