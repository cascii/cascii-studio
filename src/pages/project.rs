use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;

use super::open::Project;
use crate::components::video_player::VideoPlayer;

// Wasm bindings to Tauri API
#[wasm_bindgen(inline_js = r#"
export async function tauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);   // v2
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args); // v1
  throw new Error('Tauri invoke is not available on this page');
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;
}

// Wasm binding to our custom JS shim for convertFileSrc
#[wasm_bindgen(inline_js = r#"
export function appConvertFileSrc(path) {
  if (window.__APP__convertFileSrc) {
    return window.__APP__convertFileSrc(path);
  }
  console.error('__APP__convertFileSrc not found');
  return path;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = appConvertFileSrc)]
    fn app_convert_file_src(path: &str) -> String;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PreparedMedia {
    pub cached_abs_path: String,
    pub media_kind: String,
    pub mime_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
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
    let is_loading_media = use_state(|| false);
    
    // URL cache to avoid recomputing asset URLs
    let url_cache = use_state(|| HashMap::<String, String>::new());

    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let error_message = error_message.clone();

        use_effect_with(project_id.clone(), move |id| {
            let id = id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // Fetch project details
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project", args).await {
                    result => {
                        if let Ok(p) = serde_wasm_bindgen::from_value(result) {
                            project.set(Some(p));
                        } else {
                            error_message.set(Some("Failed to fetch project details.".to_string()));
                        }
                    }
                }

                // Fetch source files
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_sources", args).await {
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

    // When a source is selected, prepare the media and convert to asset:// URL
    let on_select_source = {
        let selected_source = selected_source.clone();
        let asset_url = asset_url.clone();
        let error_message = error_message.clone();
        let is_loading_media = is_loading_media.clone();
        let url_cache = url_cache.clone();

        Callback::from(move |source: SourceContent| {
            let file_path = source.file_path.clone();
            
            // Check cache first
            if let Some(cached_url) = url_cache.get(&file_path) {
                selected_source.set(Some(source));
                asset_url.set(Some(cached_url.clone()));
                return;
            }
            
            // Not in cache, prepare media
            let selected_source = selected_source.clone();
            let asset_url = asset_url.clone();
            let error_message = error_message.clone();
            let is_loading_media = is_loading_media.clone();
            let url_cache = url_cache.clone();
            let source_clone = source.clone();
            
            is_loading_media.set(true);
            
            wasm_bindgen_futures::spawn_local(async move {
                // Call prepare_media to get cached path
                let args = serde_wasm_bindgen::to_value(&json!({ "path": file_path })).unwrap();
                match tauri_invoke("prepare_media", args).await {
                    result => {
                        if let Ok(prepared) = serde_wasm_bindgen::from_value::<PreparedMedia>(result) {
                            // Convert cached path to asset:// URL
                            let asset_url_str = app_convert_file_src(&prepared.cached_abs_path);
                            
                            // Store in cache
                            let mut cache = (*url_cache).clone();
                            cache.insert(file_path, asset_url_str.clone());
                            url_cache.set(cache);
                            
                            // Update state
                            selected_source.set(Some(source_clone));
                            asset_url.set(Some(asset_url_str));
                        } else {
                            error_message.set(Some("Failed to prepare media file.".to_string()));
                        }
                        is_loading_media.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div class="container project-page">
            <h1>{ project.as_ref().map(|p| p.project_name.clone()).unwrap_or_else(|| "Loading Project...".into()) }</h1>

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
                                let is_selected = selected_source.as_ref()
                                    .map(|s| s.id == file.id)
                                    .unwrap_or(false);
                                let onclick = Callback::from(move |_| on_select.emit(file_clone.clone()));

                                let class_name = if is_selected { "source-item selected" } else { "source-item" };

                                html! {
                                    <div class={class_name} key={file.id.clone()} {onclick}>
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
                                if *is_loading_media {
                                    html! { <div class="loading">{"Loading media..."}</div> }
                                } else if let (Some(source), Some(url)) = (&*selected_source, &*asset_url) {
                                    if source.content_type == "Image" {
                                        html! {
                                            <img
                                                src={url.clone()}
                                                alt="Source Image"
                                                loading="lazy"
                                                decoding="async"
                                                style="max-width:100%;max-height:100%;object-fit:contain;border-radius:8px;"
                                            />
                                        }
                                    } else if source.content_type == "Video" {
                                        html! { <VideoPlayer src={url.clone()} class={classes!("source-video")} /> }
                                    } else {
                                        html! { <span>{"Unsupported file type"}</span> }
                                    }
                                } else {
                                    html! { <span>{"Select a source file to preview"}</span> }
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
