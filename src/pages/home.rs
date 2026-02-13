use yew::prelude::*;

#[function_component(HomePage)]
pub fn home_page() -> Html {
    html! {
        <div class="container" id="home-container">
            <h1>{"Cascii Studio"}</h1>
        </div>
    }
}
