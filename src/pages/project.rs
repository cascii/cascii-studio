use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use super::open::Project;
use crate::components::video_player::VideoPlayer;

#[wasm_bindgen]
extern "C" {
    // Tauri v2: window.__TAURI__.core.convertFileSrc(path)
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = convertFileSrc)]
    fn convert_file_src(path: &str) -> String;

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
    let selected_source = use_state(|| None::<SourceContent>);
    let asset_url = use_state(|| None::<String>);
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

    let on_select_source = {
        let selected_source = selected_source.clone();
        let asset_url = asset_url.clone();
        Callback::from(move |source: SourceContent| {
            let selected_source = selected_source.clone();
            let asset_url = asset_url.clone();

            let url = convert_file_src(&source.file_path);
            selected_source.set(Some(source));
            asset_url.set(Some(url));
        })
    };

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

                                let on_select = on_select_source.clone();
                                let file_clone = file.clone();
                                let onclick = Callback::from(move |_| on_select.emit(file_clone.clone()));

                                html! {
                                    <div class="source-item" key={file.id.clone()} {onclick}>
                                        { file_name }
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    </div>
                </div>

                <div class="main-content">
                    <div class="square-container">
                        <div class="square">
                            {
                                if let (Some(source), Some(url)) = (&*selected_source, &*asset_url) {
                                    if source.content_type == "Image" {
                                        html! { <img src={url.clone()} alt="Source Image" style="max-width:100%;max-height:100%;object-fit:contain;border-radius:8px;" /> }
                                    } else if source.content_type == "Video" {
                                        html! { <VideoPlayer src={url.clone()} class={classes!("source-video")} /> }
                                    } else {
                                        html! { <span>{"Unsupported file type"}</span> }
                                    }
                                } else {
                                    html! { <span>{"Source"}</span> }
                                }
                            }
                        </div>
                        <div class="square"><span>{"Preview"}</span></div>
                    </div>
                </div>
            </div>
        </div>
    }
}
