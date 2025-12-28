use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use yew_icons::{Icon, IconId};
use std::collections::HashMap;

#[wasm_bindgen(inline_js = r#"
export async function invoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);   // Tauri v2
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args); // Tauri v1
  throw new Error('Tauri invoke is not available on this page');
}

export async function listen(event, handler) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, handler);
  throw new Error('Tauri listen is not available');
}

export async function unlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    async fn listen(event: &str, handler: &js_sys::Function) -> JsValue;
    async fn unlisten(unlisten_fn: JsValue);
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct FileProgress {
    file_name: String,
    status: String,
    message: String,
    percentage: Option<f32>,
}

#[derive(Properties, PartialEq)]
pub struct NewPageProps {
    /// Called with the new project's ID after successful creation,
    /// to navigate to the Project page.
    pub on_open_project: Callback<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CreateProjectRequest {
    project_name: String,
    file_paths: Vec<String>,
}

#[derive(Serialize)]
struct CreateProjectInvokeArgs {
    request: CreateProjectRequest,
}

// Minimal shape we need back to navigate.
// Extra fields from backend will be ignored by Serde.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Project {
    id: String,
    project_name: String,
    project_type: String,
    project_path: String,
    size: i64,
    frames: i32,
}

#[function_component(NewPage)]
pub fn new_page(props: &NewPageProps) -> Html {
    let project_name = use_state(|| String::new());
    let selected_files = use_state(|| Vec::<String>::new());
    let is_creating = use_state(|| false);
    let error_message = use_state(|| Option::<String>::None);
    let success_message = use_state(|| Option::<String>::None);
    let file_progress_map = use_state(|| HashMap::<String, FileProgress>::new());

    let on_name_input = {
        let project_name = project_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            project_name.set(input.value());
        })
    };

    let on_pick_files = {
        let selected_files = selected_files.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            let selected_files = selected_files.clone();
            let error_message = error_message.clone();
            
            wasm_bindgen_futures::spawn_local(async move {
                error_message.set(None);
                
                match invoke("pick_files", JsValue::NULL).await {
                    result => {
                        if let Ok(files) = serde_wasm_bindgen::from_value::<Vec<String>>(result) {
                            web_sys::console::log_1(&format!("Selected {} files", files.len()).into());
                            selected_files.set(files);
                        } else {
                            error_message.set(Some("Failed to parse selected files".to_string()));
                        }
                    }
                }
            });
        })
    };

    let on_create_project = {
        let project_name = project_name.clone();
        let selected_files = selected_files.clone();
        let is_creating = is_creating.clone();
        let error_message = error_message.clone();
        let success_message = success_message.clone();
        let on_open_project = props.on_open_project.clone();
        let file_progress_map = file_progress_map.clone();
        
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            
            let name = (*project_name).clone();
            let files = (*selected_files).clone();
            
            // Validation
            if name.trim().is_empty() {
                error_message.set(Some("Project name is required".to_string()));
                return;
            }
            
            if files.is_empty() {
                error_message.set(Some("Please select at least one file".to_string()));
                return;
            }
            
            let is_creating = is_creating.clone();
            let error_message = error_message.clone();
            let success_message = success_message.clone();
            let project_name = project_name.clone();
            let selected_files = selected_files.clone();
            let on_open_project = on_open_project.clone();
            let file_progress_map = file_progress_map.clone();
            
            is_creating.set(true);
            error_message.set(None);
            success_message.set(None);
            file_progress_map.set(HashMap::new());
            
            wasm_bindgen_futures::spawn_local(async move {
                // Initialize progress for all files as pending
                let mut initial_map = HashMap::new();
                for file_path in files.iter() {
                    let file_name = std::path::Path::new(file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    initial_map.insert(file_name.clone(), FileProgress {
                        file_name: file_name.clone(),
                        status: "pending".to_string(),
                        message: "Waiting...".to_string(),
                        percentage: None,
                    });
                }
                file_progress_map.set(initial_map);
                
                // Set up event listener using simpler approach
                let file_progress_map_clone = file_progress_map.clone();
                let callback: Closure<dyn Fn(JsValue)> = Closure::new(move |event: JsValue| {
                    if let Ok(payload) = js_sys::Reflect::get(&event, &"payload".into()) {
                        if let Ok(progress) = serde_wasm_bindgen::from_value::<FileProgress>(payload) {
                            let mut map = (*file_progress_map_clone).clone();
                            map.insert(progress.file_name.clone(), progress);
                            file_progress_map_clone.set(map);
                        }
                    }
                });
                
                let unlisten_handle = listen("file-progress", callback.as_ref().unchecked_ref()).await;
                
                let invoke_args = CreateProjectInvokeArgs {
                    request: CreateProjectRequest {
                        project_name: name.clone(),
                        file_paths: files,
                    }
                };
                
                let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                
                let result = invoke("create_project", args).await;
                
                // Clean up listener
                unlisten(unlisten_handle).await;
                drop(callback);
                
                is_creating.set(false);
                
                // Try to parse as successful project response first
                if let Ok(project) = serde_wasm_bindgen::from_value::<Project>(result.clone()) {
                    // optional toast
                    success_message.set(Some(format!("Project '{}' created successfully!", project.project_name)));
                    // navigate to the new project page
                    on_open_project.emit(project.id.clone());
                    // clear local state
                    project_name.set(String::new());
                    selected_files.set(Vec::new());
                    file_progress_map.set(HashMap::new());
                } else {
                    // Try to extract error message
                    if let Ok(err) = serde_wasm_bindgen::from_value::<String>(result) {
                        error_message.set(Some(err));
                    } else {
                        error_message.set(Some("Failed to create project".to_string()));
                    }
                }
            });
        })
    };

    let remove_file = {
        let selected_files = selected_files.clone();
        Callback::from(move |index: usize| {
            let mut files = (*selected_files).clone();
            if index < files.len() {
                files.remove(index);
                selected_files.set(files);
            }
        })
    };

    html! {
        <div class="container new-project-page">
            <h1>{"New Project"}</h1>
            
            <form onsubmit={on_create_project} class="new-project-form">
                // Project Name Input
                <div class="form-group">
                    <label for="project-name">{"Project Title"}</label>
                    <input id="project-name" type="text" class="form-input" placeholder="Enter project name" value={(*project_name).clone()} oninput={on_name_input} disabled={*is_creating} />
                </div>

                // File Picker
                <div class="form-group">
                    <label>{"Source Files"}</label>
                    <button type="button" class="btn btn-secondary" onclick={on_pick_files} disabled={*is_creating}>
                        <Icon icon_id={IconId::LucideFolderOpen} width="20" height="20" />
                        <span>{"Select Images/Videos"}</span>
                    </button>
                    <p class="form-hint">{"Supported formats: JPG, PNG, GIF, WEBP, MP4, MOV, AVI, WEBM, MKV"}</p>
                </div>

                // Selected Files List
                if !selected_files.is_empty() {
                    <div class="form-group">
                        <label>{format!("Selected Files ({})", selected_files.len())}</label>
                        <div class="file-list">
                            {
                                selected_files.iter().enumerate().map(|(index, file)| {
                                    let file_name = std::path::Path::new(file)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or(file);
                                    
                                    let is_mkv = file.to_lowercase().ends_with(".mkv");
                                    
                                    let remove_file = remove_file.clone();
                                    let on_remove = Callback::from(move |_| remove_file.emit(index));
                                    
                                    html! {
                                        <div class="file-item" key={index}>
                                            <span class="file-name">
                                                {file_name}
                                                if is_mkv {
                                                    <span class="convert-indicator" title="Will be converted to MP4">
                                                        {"🔄"}
                                                    </span>
                                                }
                                            </span>
                                            <button type="button" class="btn-remove" onclick={on_remove} disabled={*is_creating} title="Remove file">{"×"}</button>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    </div>
                }

                // Error Message
                if let Some(error) = (*error_message).clone() {
                    <div class="alert alert-error">{error}</div>
                }

                // Success Message
                if let Some(success) = (*success_message).clone() {
                    <div class="alert alert-success">{success}</div>
                }

                // Progress Status (inline, above button)
                if *is_creating && !file_progress_map.is_empty() {
                    <div class="progress-container">
                        <h3>{"Processing Files"}</h3>
                        <div class="progress-list">
                            {
                                file_progress_map.iter().map(|(file_name, progress)| {
                                    let status_class = match progress.status.as_str() {
                                        "completed"     => "status-completed",
                                        "error"         => "status-error",
                                        "processing"    => "status-processing",
                                        _               => "status-pending"
                                    };
                                    
                                    let icon = match progress.status.as_str() {
                                        "completed"     => "✓",
                                        "error"         => "✗",
                                        "processing"    => "⟳",
                                        _               => "○"
                                    };
                                    
                                    html! {
                                        <div class={classes!("progress-item", status_class)} key={file_name.clone()}>
                                            <div class="progress-icon">{icon}</div>
                                            <div class="progress-info">
                                                <div class="progress-filename">{file_name}</div>
                                                <div class="progress-message">{&progress.message}</div>
                                                if let Some(percentage) = progress.percentage {
                                                    <div class="progress-bar-container">
                                                        <div class="progress-bar" style={format!("width: {}%", percentage)}></div>
                                                    </div>
                                                }
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                        <p class="progress-note">{"Please wait while files are being processed..."}</p>
                    </div>
                }

                // Submit Button
                <div class="form-actions">
                    <button type="submit" class="btn btn-primary" disabled={*is_creating || project_name.trim().is_empty() || selected_files.is_empty()}>
                        if *is_creating {
                            {"Creating Project..."}
                        } else {
                            {"Create New Project"}
                        }
                    </button>
                </div>
            </form>
        </div>
    }
}
