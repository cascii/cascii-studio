use yew::prelude::*;
use crate::components::sidebar::Sidebar;
use crate::pages;

#[function_component(App)]
pub fn app() -> Html {
    let current_page = use_state(|| "home".to_string());
    let on_nav = {
        let current_page = current_page.clone();
        Callback::from(move |route: &'static str| {
            current_page.set(route.to_string());
        })
    };

    html! {
        <>
            <Sidebar on_navigate={on_nav} />
            <main class="container">
                {
                    match current_page.as_str() {
                        "home" => html! { <pages::home::HomePage /> },
                        "new" => html! { <pages::new::NewPage /> },
                        "open" => html! { <pages::open::OpenPage /> },
                        "settings" => html! { <pages::settings::SettingsPage /> },
                        "library" => html! { <pages::library::LibraryPage /> },
                        "sponsor" => html! { <pages::sponsor::SponsorPage /> },
                        _ => html! { <pages::home::HomePage /> },
                    }
                }
            </main>
        </>
    }
}
