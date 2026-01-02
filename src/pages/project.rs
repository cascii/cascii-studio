use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;

use super::open::Project;
use crate::components::video_player::VideoPlayer;
use crate::components::ascii_frames_viewer::{AsciiFramesViewer, ConversionSettings};

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FrameDirectory {
    pub name: String,
    pub directory_path: String,
    pub source_file_name: String,
}

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiRequest {
    file_path: String,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: Option<u32>,
    project_id: String,
    source_file_id: String,
}

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiInvokeArgs {
    request: ConvertToAsciiRequest,
}

#[derive(Properties, PartialEq)]
pub struct ProjectPageProps {
    pub project_id: String,
}

#[function_component(ProjectPage)]
pub fn project_page(props: &ProjectPageProps) -> Html {
    let project_id = props.project_id.clone();
    let project = use_state(|| None::<Project>);
    let source_files = use_state(|| Vec::<SourceContent>::new());
    let frame_directories = use_state(|| Vec::<FrameDirectory>::new());
    let selected_source = use_state(|| None::<SourceContent>);
    let selected_frame_dir = use_state(|| None::<FrameDirectory>);
    let selected_frame_settings = use_state(|| None::<ConversionSettings>);
    let asset_url = use_state(|| None::<String>);
    let error_message = use_state(|| Option::<String>::None);
    let is_loading_media = use_state(|| false);
    let url_cache = use_state(|| HashMap::<String, String>::new());    // URL cache to avoid recomputing asset URLs
    
    // ASCII conversion settings
    let luminance = use_state(|| 1u8);
    let font_ratio = use_state(|| 0.7f32);
    let columns = use_state(|| 200u32);
    let fps = use_state(|| 30u32);
    let is_converting = use_state(|| false);
    let conversion_message = use_state(|| Option::<String>::None);
    let is_comparing = use_state(|| false);
    
    // Collapsible section states
    let source_files_collapsed = use_state(|| false);
    let frames_collapsed = use_state(|| false);
    let convert_collapsed = use_state(|| false);
    let controls_collapsed = use_state(|| false);

    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let source_files = source_files.clone();
        let frame_directories = frame_directories.clone();
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

                // Fetch frame directories
                let args = serde_wasm_bindgen::to_value(&json!({ "projectId": id })).unwrap();
                match tauri_invoke("get_project_frames", args).await {
                    result => {
                        if let Ok(frames) = serde_wasm_bindgen::from_value(result) {
                            frame_directories.set(frames);
                        } else {
                            // Not critical, just log silently
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
            <div class="project-layout">
                <div class="left-sidebar">
                    <div class="source-files-column">
                        <h2 
                            class="collapsible-header"
                            onclick={{
                                let source_files_collapsed = source_files_collapsed.clone();
                                Callback::from(move |_| {
                                    source_files_collapsed.set(!*source_files_collapsed);
                                })
                            }}
                        >
                            <span class="chevron-icon">
                                {if *source_files_collapsed {
                                    html! {<span>{"▶"}</span>}
                                } else {
                                    html! {<span>{"▼"}</span>}
                                }}
                            </span>
                            <span>{"SOURCE FILES"}</span>
                        </h2>
                        {
                            if !*source_files_collapsed {
                                html! {
                                    <div class="source-list">
                                    {
                                        source_files.iter().map(|file| {
                                    let file_name = std::path::Path::new(&file.file_path).file_name().and_then(|n| n.to_str()).unwrap_or(&file.file_path);

                                    let on_select = on_select_source.clone();
                                    let file_clone = file.clone();
                                    let is_selected = selected_source.as_ref().map(|s| s.id == file.id).unwrap_or(false);
                                    let onclick = Callback::from(move |_| on_select.emit(file_clone.clone()));

                                    let class_name = if is_selected {"source-item selected"} else {"source-item"};

                                    html! {
                                        <div class={class_name} key={file.id.clone()} {onclick}>{file_name}</div>
                                        }
                                    }).collect::<Html>()
                                    }
                                    </div>
                                }
                            } else {
                                html! {<></>}
                            }
                        }
                    </div>
                    
                    <div class="frames-column">
                        <h2 
                            class="collapsible-header"
                            onclick={{
                                let frames_collapsed = frames_collapsed.clone();
                                Callback::from(move |_| {
                                    frames_collapsed.set(!*frames_collapsed);
                                })
                            }}
                        >
                            <span class="chevron-icon">
                                {if *frames_collapsed {
                                    html! {<span>{"▶"}</span>}
                                } else {
                                    html! {<span>{"▼"}</span>}
                                }}
                            </span>
                            <span>{"AVAILABLE FRAMES"}</span>
                        </h2>
                        {
                            if !*frames_collapsed {
                                let selected_frame_dir_clone = selected_frame_dir.clone();
                                let selected_frame_settings_clone = selected_frame_settings.clone();
                                html! {
                                    <div class="source-list">
                                    {
                                        if frame_directories.is_empty() {
                                            html! {
                                                <div class="frames-empty">{"No frames generated yet"}</div>
                                            }
                                        } else {
                                            html! {
                                                {
                                                    frame_directories.iter().map(|frame_dir| {
                                                        let frame_clone = frame_dir.clone();
                                                        let is_selected = selected_frame_dir_clone.as_ref()
                                                            .map(|s| s.directory_path == frame_dir.directory_path)
                                                            .unwrap_or(false);
                                                        let onclick = Callback::from({
                                                            let selected_frame_dir = selected_frame_dir_clone.clone();
                                                            let selected_frame_settings = selected_frame_settings_clone.clone();
                                                            let directory_path = frame_dir.directory_path.clone();
                                                            move |_| {
                                                                selected_frame_dir.set(Some(frame_clone.clone()));
                                                                
                                                                // Fetch conversion settings for this frame directory
                                                                let selected_frame_settings = selected_frame_settings.clone();
                                                                let directory_path = directory_path.clone();
                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                    let args = serde_wasm_bindgen::to_value(&json!({ "folderPath": directory_path })).unwrap();
                                                                    match tauri_invoke("get_conversion_by_folder_path", args).await {
                                                                        result => {
                                                                            if let Ok(Some(conversion)) = serde_wasm_bindgen::from_value::<Option<serde_json::Value>>(result) {
                                                                                // Extract settings from the conversion
                                                                                if let Some(settings) = conversion.get("settings") {
                                                                                    if let Ok(conv_settings) = serde_json::from_value::<ConversionSettings>(settings.clone()) {
                                                                                        selected_frame_settings.set(Some(conv_settings));
                                                                                        return;
                                                                                    }
                                                                                }
                                                                            }
                                                                            // No conversion found or failed to parse
                                                                            selected_frame_settings.set(None);
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        });

                                                        let class_name = if is_selected { "source-item selected" } else { "source-item" };

                                                        html! {
                                                            <div class={class_name} key={frame_dir.directory_path.clone()} {onclick}>
                                                                { frame_dir.name.clone() }
                                                            </div>
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            }
                                        }
                                    }
                                    </div>
                                }
                            } else {
                                html! {<></>}
                            }
                        }
                    </div>
                    
                    <div class="ascii-conversion-column">
                        <h2 
                            class="collapsible-header"
                            onclick={{
                                let convert_collapsed = convert_collapsed.clone();
                                Callback::from(move |_| {
                                    convert_collapsed.set(!*convert_collapsed);
                                })
                            }}
                        >
                            <span class="chevron-icon">
                                {if *convert_collapsed {
                                    html! {<span>{"▶"}</span>}
                                } else {
                                    html! {<span>{"▼"}</span>}
                                }}
                            </span>
                            <span>{"CONVERT TO ASCII"}</span>
                        </h2>
                        
                        {
                            if !*convert_collapsed {
                                html! {
                                    <>
                                        <div class="conversion-settings">
                                            <div class="setting-row">
                                                <label>{"Luminance:"}</label>
                                                <input 
                                                    type="number" 
                                                    class="setting-input"
                                                    value={(*luminance).to_string()}
                                                    min="0"
                                                    max="255"
                                                    oninput={Callback::from({
                                                        let luminance = luminance.clone();
                                                        move |e: web_sys::InputEvent| {
                                                            if let Some(target) = e.target() {
                                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                                    if let Ok(val) = input.value().parse::<u8>() {
                                                                        luminance.set(val);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    })}
                                                />
                                            </div>
                                            
                                            <div class="setting-row">
                                                <label>{"Font Ratio:"}</label>
                                                <input 
                                                    type="number" 
                                                    class="setting-input"
                                                    value={(*font_ratio).to_string()}
                                                    min="0.1"
                                                    max="2.0"
                                                    step="0.1"
                                                    oninput={Callback::from({
                                                        let font_ratio = font_ratio.clone();
                                                        move |e: web_sys::InputEvent| {
                                                            if let Some(target) = e.target() {
                                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                                    if let Ok(val) = input.value().parse::<f32>() {
                                                                        font_ratio.set(val);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    })}
                                                />
                                            </div>
                                            
                                            <div class="setting-row">
                                                <label>{"Columns:"}</label>
                                                <input 
                                                    type="number" 
                                                    class="setting-input"
                                                    value={(*columns).to_string()}
                                                    min="1"
                                                    max="2000"
                                                    oninput={Callback::from({
                                                        let columns = columns.clone();
                                                        move |e: web_sys::InputEvent| {
                                                            if let Some(target) = e.target() {
                                                                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                                    if let Ok(val) = input.value().parse::<u32>() {
                                                                        columns.set(val);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    })}
                                                />
                                            </div>
                                            
                                            {
                                                if selected_source.as_ref().map(|s| s.content_type == "Video").unwrap_or(false) {
                                                    html! {
                                                        <div class="setting-row">
                                                            <label>{"FPS:"}</label>
                                                            <input 
                                                                type="number" 
                                                                class="setting-input"
                                                                value={(*fps).to_string()}
                                                                min="1"
                                                                max="120"
                                                                oninput={Callback::from({
                                                                    let fps = fps.clone();
                                                                    move |e: web_sys::InputEvent| {
                                                                        if let Some(target) = e.target() {
                                                                            if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                                                if let Ok(val) = input.value().parse::<u32>() {
                                                                                    fps.set(val);
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                })}
                                                            />
                                                        </div>
                                                    }
                                                } else {
                                                    html! {<></>}
                                                }
                                            }
                                        </div>
                                        
                                        <button 
                                            class="btn-convert"
                                            disabled={*is_converting || selected_source.is_none()}
                                            onclick={{
                                                let selected_source = selected_source.clone();
                                                let luminance = luminance.clone();
                                                let font_ratio = font_ratio.clone();
                                                let columns = columns.clone();
                                                let fps = fps.clone();
                                                let is_converting = is_converting.clone();
                                                let conversion_message = conversion_message.clone();
                                                let error_message = error_message.clone();
                                                let frame_directories = frame_directories.clone();
                                                let project_id_clone = project_id.clone();
                                                
                                                Callback::from(move |_e: yew::MouseEvent| {
                                                    if let Some(source) = &*selected_source {
                                                        let file_path = source.file_path.clone();
                                                        let source_file_id = source.id.clone();
                                                        let luminance_val = *luminance;
                                                        let font_ratio_val = *font_ratio;
                                                        let columns_val = *columns;
                                                        let fps_val = *fps;
                                                        
                                                        is_converting.set(true);
                                                        conversion_message.set(None);
                                                        
                                                        let is_converting = is_converting.clone();
                                                        let conversion_message = conversion_message.clone();
                                                        let error_message = error_message.clone();
                                                        let frame_directories = frame_directories.clone();
                                                        let project_id_clone = project_id_clone.clone();
                                                        
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            let invoke_args = ConvertToAsciiInvokeArgs {
                                                                request: ConvertToAsciiRequest {
                                                                    file_path,
                                                                    luminance: luminance_val,
                                                                    font_ratio: font_ratio_val,
                                                                    columns: columns_val,
                                                                    fps: Some(fps_val),
                                                                    project_id: project_id_clone.clone(),
                                                                    source_file_id,
                                                                }
                                                            };
                                                            
                                                            let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                                                            
                                                            match tauri_invoke("convert_to_ascii", args).await {
                                                                result => {
                                                                    is_converting.set(false);
                                                                    match serde_wasm_bindgen::from_value::<String>(result) {
                                                                        Ok(msg) => {
                                                                            conversion_message.set(Some(msg));
                                                                            error_message.set(None);
                                                                            
                                                                            // Refresh frame directories after conversion
                                                                            let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id_clone })).unwrap();
                                                                            match tauri_invoke("get_project_frames", args).await {
                                                                                result => {
                                                                                    if let Ok(frames) = serde_wasm_bindgen::from_value(result) {
                                                                                        frame_directories.set(frames);
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error_message.set(Some("Failed to convert to ASCII. Please check the file path and try again.".to_string()));
                                                                            conversion_message.set(None);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        });
                                                    }
                                                })
                                            }}
                                        >
                                            if *is_converting {
                                                {"Converting..."}
                                            } else {
                                                {"Convert to ASCII"}
                                            }
                                        </button>
                                        
                                        {
                                            if let Some(msg) = &*conversion_message {
                                                html! { <div class="conversion-success">{msg}</div> }
                                            } else {
                                                html! {<></>}
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! {<></>}
                            }
                        }
                    </div>
                    
                    <div class="controls-column">
                        <h2 
                            class="collapsible-header"
                            onclick={{
                                let controls_collapsed = controls_collapsed.clone();
                                Callback::from(move |_| {
                                    controls_collapsed.set(!*controls_collapsed);
                                })
                            }}
                        >
                            <span class="chevron-icon">
                                {if *controls_collapsed {
                                    html! {<span>{"▶"}</span>}
                                } else {
                                    html! {<span>{"▼"}</span>}
                                }}
                            </span>
                            <span>{"CONTROLS"}</span>
                        </h2>
                        {
                            if !*controls_collapsed {
                                html! {
                                    <button 
                                        class="btn-compare"
                                        disabled={selected_source.is_none() || selected_frame_dir.is_none()}
                                        onclick={{
                                            let is_comparing = is_comparing.clone();
                                            Callback::from(move |_| {
                                                is_comparing.set(!*is_comparing);
                                            })
                                        }}
                                    >
                                        if *is_comparing {
                                            {"Stop Compare"}
                                        } else {
                                            {"Compare"}
                                        }
                                    </button>
                                }
                            } else {
                                html! {<></>}
                            }
                        }
                    </div>
                </div>

                <div class="main-content">
                    <h1>{ project.as_ref().map(|p| p.project_name.clone()).unwrap_or_else(|| "Loading Project...".into()) }</h1>

                    if let Some(error) = &*error_message {
                        <div class="alert alert-error">{error}</div>
                    }

                    <div class="preview-container">
                        <div class="preview-column">
                            <div class="preview-label">{"Source Video"}</div>
                            <div class="square">
                                {
                                    if *is_loading_media {
                                        html! { <div class="loading">{"Loading media..."}</div> }
                                    } else if let (Some(source), Some(url)) = (&*selected_source, &*asset_url) {
                                        if source.content_type == "Image" {
                                            html! {
                                                <img src={url.clone()} alt="Source Image" loading="lazy" decoding="async" style="max-width:100%;max-height:100%;object-fit:contain;border-radius:8px;" />
                                            }
                                        } else if source.content_type == "Video" {
                                            html! { 
                                                <VideoPlayer 
                                                    src={url.clone()} 
                                                    class={classes!("source-video")}
                                                    should_play={if *is_comparing {Some(true)} else {Some(false)}}
                                                /> 
                                            }
                                        } else {
                                            html! { <span>{"Unsupported file type"}</span> }
                                        }
                                    } else {
                                        html! { <span>{"Select a source file to preview"}</span> }
                                    }
                                }
                            </div>
                        </div>
                        
                        <div class="preview-column">
                            <div class="preview-label">{"Frames"}</div>
                            <div class="square">
                                {
                                    if frame_directories.is_empty() {
                                        html! { <span>{"No frames generated yet"}</span> }
                                    } else if let Some(frame_dir) = &*selected_frame_dir {
                                        html! {
                                            <AsciiFramesViewer 
                                                directory_path={frame_dir.directory_path.clone()}
                                                fps={selected_frame_settings.as_ref().map(|s| s.fps).unwrap_or(*fps)}
                                                settings={(*selected_frame_settings).clone()}
                                                should_play={if *is_comparing {Some(true)} else {Some(false)}}
                                            />
                                        }
                                    } else {
                                        html! { <span>{"Select a frame directory to preview"}</span> }
                                    }
                                }
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
