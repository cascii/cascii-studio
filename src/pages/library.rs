use yew::prelude::*;

#[function_component(LibraryPage)]
pub fn library_page() -> Html {
    html! {
        <div class="container" id="library-container">
            <h1 id="library-heading">{"Library"}</h1>
            <p id="library-description">{"This is the Library page."}</p>
        </div>
    }
}
