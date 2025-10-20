use yew::prelude::*;

#[function_component(NewPage)]
pub fn new_page() -> Html {
    html! {
        <div class="container">
            <h1>{"New Project"}</h1>
            <p>{"This is the New Project page."}</p>
        </div>
    }
}
