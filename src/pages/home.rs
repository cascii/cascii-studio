use yew::prelude::*;

#[function_component(HomePage)]
pub fn home_page() -> Html {
    html! {
        <div class="container">
            <h1>{"Cascii Studio"}</h1>
            <p>{"Click on the Tauri and Yew logos to learn more."}</p>
        </div>
    }
}
