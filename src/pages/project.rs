use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// Assuming Project struct is defined in open.rs or a shared module
use super::open::Project;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceContent {
    pub id: String,
    pub content_type: String, // "Image" or "Video"
    pub project_id: String,
    pub date_added: DateTime<Utc>,
    pub size: i64,
    pub file_path: String,
}

#[derive(Properties, PartialEq)]
pub struct ProjectPageProps {
    pub project_id: String,
}

#[function_component(ProjectPage)]
pub fn project_page(props: &ProjectPageProps) -> Html {
    let project = use_state(|| None::<Project>);
    let source_files = use_state(|| Vec::<SourceContent>::new());
    let error_message = use_state(|| Option::<String>::None);

    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let error_message = error_message.clone();

        use_effect_with(project_id.clone(), move |id| {
            let id = id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch project details
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "projectId": id })).unwrap();
                match invoke("get_project", args).await {
                    result => {
                        if let Ok(p) = serde_wasm_bindgen::from_value(result) {
                            project.set(Some(p));
                        } else {
                            error_message.set(Some("Failed to fetch project details.".to_string()));
                        }
                    }
                }

                // Fetch source files
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "projectId": id })).unwrap();
                match invoke("get_project_sources", args).await {
                    result => {
                        if let Ok(s) = serde_wasm_bindgen::from_value(result) {
                            source_files.set(s);
                        } else {
                            error_message.set(Some("Failed to fetch source files.".to_string()));
                        }
                    }
                }
            });

            || ()
        });
    }
    
    html! {
        <div class="container project-page">
            if let Some(p) = &*project {
                <h1>{ &p.project_name }</h1>
            } else {
                <h1>{"Loading Project..."}</h1>
            }

            if let Some(error) = &*error_message {
                <div class="alert alert-error">{error}</div>
            }

            <div class="project-layout">
                <div class="source-files-column">
                    <h2>{"Source Files"}</h2>
                    <div class="source-list">
                        {
                            source_files.iter().map(|file| {
                                let file_name = std::path::Path::new(&file.file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(&file.file_path);
                                html! {
                                    <div class="source-item" key={file.id.clone()}>
                                        { file_name }
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    </div>
                </div>
                <div class="main-content">
                    <div class="square-container">
                        <div class="square"><span>{"Source"}</span></div>
                        <div class="square"><span>{"Preview"}</span></div>
                    </div>
                </div>
            </div>
        </div>
    }
}
