use yew::prelude::*;
use crate::components::ascii_animation::AsciiAnimation;

#[function_component(HomePage)]
pub fn home_page() -> Html {
    html! {
        <div class="container">
            <h1>{"Cascii Studio"}</h1>
            <AsciiAnimation frame_folder="loop_project" fps={30} />
        </div>
    }
}
