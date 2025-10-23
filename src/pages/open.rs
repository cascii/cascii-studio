use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

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

    {
        let projects = projects.clone();
        let error_message = error_message.clone();

        use_effect_with((), move |_| {
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
                        </tr>
                    </thead>
                    <tbody>
                        {
                            projects.iter().map(|project| {
                                let on_open_project = props.on_open_project.clone();
                                let project_id = project.id.clone();
                                let onclick = Callback::from(move |_| {
                                    on_open_project.emit(project_id.clone());
                                });

                                html! {
                                    <tr key={project.id.clone()} {onclick}>
                                        <td>{ &project.project_name }</td>
                                        <td>{ project.last_modified.format("%Y-%m-%d %H:%M").to_string() }</td>
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
