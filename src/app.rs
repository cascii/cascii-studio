use yew::prelude::*;
use crate::components::sidebar::Sidebar;
use crate::pages;

#[function_component(App)]
pub fn app() -> Html {
    let current_page = use_state(|| "home".to_string());
    let active_project_id = use_state(|| Option::<String>::None);
    let active_project_name = use_state(|| Option::<String>::None);

    let on_nav = {
        let current_page = current_page.clone();
        Callback::from(move |route: &'static str| {
            current_page.set(route.to_string());
        })
    };

    let on_open_project = {
        let current_page = current_page.clone();
        let active_project_id = active_project_id.clone();
        let active_project_name = active_project_name.clone();
        Callback::from(move |project_id: String| {
            active_project_id.set(Some(project_id));
            active_project_name.set(None);
            current_page.set("project".to_string());
        })
    };

    let on_open_montage = {
        let current_page = current_page.clone();
        let active_project_id = active_project_id.clone();
        let active_project_name = active_project_name.clone();
        Callback::from(move |project_id: String| {
            active_project_id.set(Some(project_id));
            active_project_name.set(None);
            current_page.set("montage".to_string());
        })
    };

    let on_project_name_change = {
        let active_project_name = active_project_name.clone();
        Callback::from(move |project_name: String| {
            active_project_name.set(Some(project_name));
        })
    };

    let context_label = match current_page.as_str() {
        "home" => "cascii studio".to_string(),
        "project" | "montage" => {
            if let Some(project_name) = &*active_project_name {
                format!("cascii studio - {}", project_name)
            } else {
                format!("cascii studio - {}", current_page.as_str())
            }
        }
        _ => format!("cascii studio - {}", current_page.as_str()),
    };

    html! {
        <>
            <Sidebar
                on_navigate={on_nav}
                current_page={(*current_page).clone()}
                context_label={context_label}
                has_active_project={active_project_id.is_some()}
            />
            <main class="container" id="app-container">
                {
                    match current_page.as_str() {
                        "home"      => html! { <pages::home::HomePage /> },
                        "new"       => html! { <pages::new::NewPage on_open_project={on_open_project.clone()} /> },
                        "open"      => html! { <pages::open::OpenPage on_open_project={on_open_project.clone()} on_open_montage={Some(on_open_montage.clone())} /> },
                        "settings"  => html! { <pages::settings::SettingsPage /> },
                        "library"   => html! { <pages::library::LibraryPage /> },
                        "sponsor"   => html! { <pages::sponsor::SponsorPage /> },
                        "project" => {
                            if let Some(id) = &*active_project_id {
                                html! { <pages::project::ProjectPage project_id={id.clone()} on_project_name_change={on_project_name_change.clone()} /> }
                            } else {
                                html! { <pages::open::OpenPage on_open_project={on_open_project.clone()} on_open_montage={Some(on_open_montage.clone())} /> }
                            }
                        },
                        "montage" => {
                            if let Some(id) = &*active_project_id {
                                html! { <pages::montage::MontagePage project_id={id.clone()} on_project_name_change={on_project_name_change.clone()} /> }
                            } else {
                                html! { <pages::open::OpenPage on_open_project={on_open_project.clone()} on_open_montage={Some(on_open_montage.clone())} /> }
                            }
                        },
                        _ => html! { <pages::home::HomePage /> },
                    }
                }
            </main>
        </>
    }
}
