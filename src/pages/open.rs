use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use yew_icons::{Icon, IconId};
use serde_json::json;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProjectType {
    Image,
    Animation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub project_name: String,
    pub project_type: ProjectType,
    pub project_path: String,
    pub size: i64,
    pub frames: i32,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Properties, PartialEq)]
pub struct OpenPageProps {
    pub on_open_project: Callback<String>,
}

#[function_component(OpenPage)]
pub fn open_page(props: &OpenPageProps) -> Html {
    let projects = use_state(|| Vec::<Project>::new());
    let error_message = use_state(|| Option::<String>::None);
    let deleting_project_id = use_state(|| Option::<String>::None);
    let refresh_trigger = use_state(|| 0);

    // Fetch projects effect
    {
        let projects = projects.clone();
        let error_message = error_message.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match invoke("get_all_projects", JsValue::NULL).await {
                    result => {
                        if let Ok(fetched_projects) = serde_wasm_bindgen::from_value(result) {
                            projects.set(fetched_projects);
                        } else {
                            error_message.set(Some("Failed to fetch projects.".to_string()));
                        }
                    }
                }
            });

            || ()
        });
    }

    // Delete handler
    let on_delete_project = {
        let deleting_project_id = deleting_project_id.clone();
        let error_message = error_message.clone();
        let refresh_trigger = refresh_trigger.clone();

        Callback::from(move |project_id: String| {
            let deleting_project_id = deleting_project_id.clone();
            let error_message = error_message.clone();
            let refresh_trigger = refresh_trigger.clone();
            let project_id_clone = project_id.clone();

            deleting_project_id.set(Some(project_id.clone()));

            wasm_bindgen_futures::spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id_clone })).unwrap();
                match invoke("delete_project", args).await {
                    result => {
                        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
                            // Success - refresh the list
                            refresh_trigger.set(*refresh_trigger + 1);
                        } else {
                            error_message.set(Some("Failed to delete project.".to_string()));
                        }
                        deleting_project_id.set(None);
                    }
                }
            });
        })
    };

    html! {
        <div class="container open-page">
            <h1>{"Open Project"}</h1>

            if let Some(error) = &*error_message {
                <div class="alert alert-error">{error}</div>
            }

            if projects.is_empty() {
                <p>{"No projects found."}</p>
            } else {
                <table class="project-table">
                    <thead>
                        <tr>
                            <th>{"Project Name"}</th>
                            <th>{"Last Modified"}</th>
                            <th class="actions-column"></th>
                        </tr>
                    </thead>
                    <tbody>
                        {
                            projects.iter().map(|project| {
                                let on_open_project = props.on_open_project.clone();
                                let on_delete_project = on_delete_project.clone();
                                let project_id = project.id.clone();
                                let project_id_for_delete = project.id.clone();
                                let is_deleting = deleting_project_id.as_ref() == Some(&project.id);

                                let onclick = {
                                    let project_id = project_id.clone();
                                    Callback::from(move |_: MouseEvent| {
                                        on_open_project.emit(project_id.clone());
                                    })
                                };

                                let on_delete_click = Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    on_delete_project.emit(project_id_for_delete.clone());
                                });

                                html! {
                                    <tr key={project.id.clone()} {onclick} class={if is_deleting { "deleting" } else { "" }}>
                                        <td>{ &project.project_name }</td>
                                        <td>{ project.last_modified.format("%Y-%m-%d %H:%M").to_string() }</td>
                                        <td class="actions-cell">
                                            <button 
                                                class="delete-btn" 
                                                onclick={on_delete_click}
                                                disabled={is_deleting}
                                                title="Delete project"
                                            >
                                                <Icon icon_id={IconId::LucideTrash2} width={"18"} height={"18"} />
                                            </button>
                                        </td>
                                    </tr>
                                }
                            }).collect::<Html>()
                        }
                    </tbody>
                </table>
            }
        </div>
    }
}
