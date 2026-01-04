use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;

use super::open::Project;
use crate::components::video_player::VideoPlayer;
use crate::components::ascii_frames_viewer::{AsciiFramesViewer, ConversionSettings};
use crate::components::settings::{SourceFiles, AvailableFrames, ConvertToAscii, Controls};

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
    let is_playing = use_state(|| false);
    let should_reset = use_state(|| false);
    let synced_progress = use_state(|| 0.0f64); // 0-100 percentage
    let seek_percentage = use_state(|| None::<f64>);
    let frames_loading = use_state(|| false);
    let frame_speed = use_state(|| None::<u32>);
    let current_conversion_id = use_state(|| None::<String>);
    let speed_mode = use_state(|| crate::components::ascii_frames_viewer::SpeedMode::CustomFrameSpeed);

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
                    <SourceFiles
                        source_files={(*source_files).clone()}
                        selected_source={(*selected_source).clone()}
                        source_files_collapsed={*source_files_collapsed}
                        on_toggle_collapsed={{
                            let source_files_collapsed = source_files_collapsed.clone();
                            Callback::from(move |_| {
                                source_files_collapsed.set(!*source_files_collapsed);
                            })
                        }}
                        on_select_source={on_select_source.clone()}
                    />

                    <AvailableFrames frame_directories={(*frame_directories).clone()} selected_frame_dir={(*selected_frame_dir).clone()} selected_frame_settings={(*selected_frame_settings).clone()} frames_collapsed={*frames_collapsed} on_toggle_collapsed={{
                            let frames_collapsed = frames_collapsed.clone();
                            Callback::from(move |_| {
                                frames_collapsed.set(!*frames_collapsed);
                            })
                        }}
                        on_select_frame_dir={{
                            let selected_frame_dir = selected_frame_dir.clone();
                            Callback::from(move |frame_dir: FrameDirectory| {
                                selected_frame_dir.set(Some(frame_dir));
                            })
                        }}
                        on_frame_settings_loaded={{
                            let selected_frame_settings = selected_frame_settings.clone();
                            let frame_speed = frame_speed.clone();
                            let current_conversion_id = current_conversion_id.clone();
                            Callback::from(move |data: Option<(ConversionSettings, Option<String>)>| {
                                match data {
                                    Some((settings, conversion_id)) => {
                                        selected_frame_settings.set(Some(settings.clone()));
                                        frame_speed.set(Some(settings.frame_speed));
                                        current_conversion_id.set(conversion_id);
                                    }
                                    None => {
                                        selected_frame_settings.set(None);
                                        frame_speed.set(None);
                                        current_conversion_id.set(None);
                                    }
                                }
                            })
                        }}
                    />

                    <ConvertToAscii selected_source={(*selected_source).clone()} convert_collapsed={*convert_collapsed} on_toggle_collapsed={{
                            let convert_collapsed = convert_collapsed.clone();
                            Callback::from(move |_| {
                                convert_collapsed.set(!*convert_collapsed);
                            })
                        }}
                        luminance={*luminance}
                        on_luminance_change={{
                            let luminance = luminance.clone();
                            Callback::from(move |val: u8| {
                                luminance.set(val);
                            })
                        }}
                        font_ratio={*font_ratio}
                        on_font_ratio_change={{
                            let font_ratio = font_ratio.clone();
                            Callback::from(move |val: f32| {
                                font_ratio.set(val);
                            })
                        }}
                        columns={*columns}
                        on_columns_change={{
                            let columns = columns.clone();
                            Callback::from(move |val: u32| {
                                columns.set(val);
                            })
                        }}
                        fps={*fps}
                        on_fps_change={{
                            let fps = fps.clone();
                            Callback::from(move |val: u32| {
                                fps.set(val);
                            })
                        }}
                        is_converting={*is_converting}
                        on_is_converting_change={{
                            let is_converting = is_converting.clone();
                            Callback::from(move |val: bool| {
                                is_converting.set(val);
                            })
                        }}
                        conversion_message={(*conversion_message).clone()}
                        on_conversion_message_change={{
                            let conversion_message = conversion_message.clone();
                            Callback::from(move |val: Option<String>| {
                                conversion_message.set(val);
                            })
                        }}
                        error_message={(*error_message).clone()}
                        on_error_message_change={{
                            let error_message = error_message.clone();
                            Callback::from(move |val: Option<String>| {
                                error_message.set(val);
                            })
                        }}
                        project_id={project_id.clone()}
                        on_refresh_frames={{
                            let frame_directories = frame_directories.clone();
                            let project_id = project_id.clone();
                            Callback::from(move |_| {
                                let frame_directories = frame_directories.clone();
                                let project_id = project_id.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id })).unwrap();
                                    match tauri_invoke("get_project_frames", args).await {
                                        result => {
                                            if let Ok(frames) = serde_wasm_bindgen::from_value(result) {
                                                frame_directories.set(frames);
                                            }
                                        }
                                    }
                                });
                            })
                        }}
                    />

                    <Controls
                        selected_source={(*selected_source).clone()}
                        selected_frame_dir={(*selected_frame_dir).clone()}
                        controls_collapsed={*controls_collapsed}
                        on_toggle_collapsed={{
                            let controls_collapsed = controls_collapsed.clone();
                            Callback::from(move |_| {
                                controls_collapsed.set(!*controls_collapsed);
                            })
                        }}
                        is_playing={*is_playing}
                        on_is_playing_change={{
                            let is_playing = is_playing.clone();
                            Callback::from(move |val: bool| {
                                is_playing.set(val);
                            })
                        }}
                        should_reset={*should_reset}
                        on_should_reset_change={{
                            let should_reset = should_reset.clone();
                            Callback::from(move |val: bool| {
                                should_reset.set(val);
                            })
                        }}
                        synced_progress={*synced_progress}
                        on_synced_progress_change={{
                            let synced_progress = synced_progress.clone();
                            Callback::from(move |val: f64| {
                                synced_progress.set(val);
                            })
                        }}
                        seek_percentage={*seek_percentage}
                        on_seek_percentage_change={{
                            let seek_percentage = seek_percentage.clone();
                            Callback::from(move |val: Option<f64>| {
                                seek_percentage.set(val);
                            })
                        }}
                        frames_loading={*frames_loading}
                    />
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
                                                    should_play={if *is_playing {Some(true)} else {Some(false)}}
                                                    should_reset={*should_reset}
                                                    seek_percentage={*seek_percentage}
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
                                                fps={{
                                                    // Use speed mode to determine which FPS to use
                                                    match *speed_mode {
                                                        crate::components::ascii_frames_viewer::SpeedMode::BaseFps => {
                                                            selected_frame_settings.as_ref().map(|s| s.fps).unwrap_or(*fps)
                                                        }
                                                        crate::components::ascii_frames_viewer::SpeedMode::CustomFrameSpeed => {
                                                            (*frame_speed).unwrap_or(selected_frame_settings.as_ref().map(|s| s.fps).unwrap_or(*fps))
                                                        }
                                                    }
                                                }}
                                                settings={(*selected_frame_settings).clone()}
                                                should_play={if *is_playing && !*frames_loading {Some(true)} else {Some(false)}}
                                                should_reset={*should_reset}
                                                seek_percentage={*seek_percentage}
                                                on_loading_changed={{
                                                    let frames_loading = frames_loading.clone();
                                                    Callback::from(move |is_loading: bool| {
                                                        frames_loading.set(is_loading);
                                                    })
                                                }}
                                                frame_speed={*frame_speed}
                                                on_frame_speed_change={{
                                                    let frame_speed = frame_speed.clone();
                                                    let current_conversion_id = current_conversion_id.clone();
                                                    let selected_frame_settings = selected_frame_settings.clone();
                                                    Callback::from(move |speed: u32| {
                                                        frame_speed.set(Some(speed));

                                                        // Update the selected_frame_settings to reflect the change
                                                        if let Some(mut settings) = (*selected_frame_settings).clone() {
                                                            settings.frame_speed = speed;
                                                            selected_frame_settings.set(Some(settings));
                                                        }

                                                        // Update the database if we have a conversion_id
                                                        if let Some(conversion_id) = &*current_conversion_id {
                                                            let conversion_id = conversion_id.clone();
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                                                                    "conversionId": conversion_id,
                                                                    "frameSpeed": speed
                                                                })).unwrap();
                                                                let _ = tauri_invoke("update_conversion_frame_speed", args).await;
                                                            });
                                                        }
                                                    })
                                                }}
                                                speed_mode={(*speed_mode).clone()}
                                                on_speed_mode_change={{
                                                    let speed_mode = speed_mode.clone();
                                                    Callback::from(move |mode: crate::components::ascii_frames_viewer::SpeedMode| {
                                                        speed_mode.set(mode);
                                                    })
                                                }}
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
