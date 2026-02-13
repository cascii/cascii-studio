use yew::prelude::*;

#[function_component(LibraryPage)]
pub fn library_page() -> Html {
    html! {
        <div class="container" id="library-container">
            <h1>{"Library"}</h1>
            <p>{"This is the Library page."}</p>
        </div>
    }
}
