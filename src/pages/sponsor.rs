use yew::prelude::*;

#[function_component(SponsorPage)]
pub fn sponsor_page() -> Html {
    html! {
        <div class="container" id="sponsor-container">
            <h1>{"Sponsor"}</h1>
            <p>{"This is the Sponsor page."}</p>
        </div>
    }
}
