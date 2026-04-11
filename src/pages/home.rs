use crate::components::header::Header;
use yew::prelude::*;

#[function_component(HomePage)]
pub fn home_page() -> Html {
    html! {
        <div class="container" id="home-container">
            <div id="home-header-container">
                <Header />
            </div>
            <h1 id="home-heading">{"Cascii Studio"}</h1>
        </div>
    }
}
