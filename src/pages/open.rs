use yew::prelude::*;

#[function_component(OpenPage)]
pub fn open_page() -> Html {
    html! {
        <div class="container">
            <h1>{"Open Project"}</h1>
            <p>{"This is the Open Project page."}</p>
        </div>
    }
}
