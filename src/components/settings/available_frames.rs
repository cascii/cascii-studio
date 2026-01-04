use yew::prelude::*;
use crate::pages::project::FrameDirectory;
use crate::components::ascii_frames_viewer::ConversionSettings;
use wasm_bindgen::prelude::*;
use serde_json::json;

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

#[derive(Properties, PartialEq)]
pub struct AvailableFramesProps {
    pub frame_directories: Vec<FrameDirectory>,
    pub selected_frame_dir: Option<FrameDirectory>,
    pub selected_frame_settings: Option<ConversionSettings>,
    pub frames_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub on_select_frame_dir: Callback<FrameDirectory>,
    pub on_frame_settings_loaded: Callback<Option<(ConversionSettings, Option<String>)>>,
}

#[function_component(AvailableFrames)]
pub fn available_frames(props: &AvailableFramesProps) -> Html {
    let on_toggle = {
        let on_toggle_collapsed = props.on_toggle_collapsed.clone();
        Callback::from(move |_| {
            on_toggle_collapsed.emit(());
        })
    };

    let frame_directories = &props.frame_directories;
    let selected_frame_dir = &props.selected_frame_dir;
    let on_select_frame_dir = &props.on_select_frame_dir;
    let on_frame_settings_loaded = &props.on_frame_settings_loaded;

    html! {
        <div class="frames-column">
            <h2 class="collapsible-header" onclick={on_toggle}>
                <span class="chevron-icon">
                    {if props.frames_collapsed {
                        html! {<span>{"▶"}</span>}
                    } else {
                        html! {<span>{"▼"}</span>}
                    }}
                </span>
                <span>{"AVAILABLE FRAMES"}</span>
            </h2>
            {
                if !props.frames_collapsed {
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
                                            let is_selected = selected_frame_dir.as_ref()
                                                .map(|s| s.directory_path == frame_dir.directory_path)
                                                .unwrap_or(false);

                                            let onclick = {
                                                let on_select = on_select_frame_dir.clone();
                                                let on_settings_loaded = on_frame_settings_loaded.clone();
                                                let directory_path = frame_dir.directory_path.clone();

                                                Callback::from(move |_| {
                                                    on_select.emit(frame_clone.clone());

                                                    // Fetch conversion settings for this frame directory
                                                    let on_settings_loaded = on_settings_loaded.clone();
                                                    let directory_path = directory_path.clone();
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        let args = serde_wasm_bindgen::to_value(&json!({ "folderPath": directory_path })).unwrap();
                                                        match tauri_invoke("get_conversion_by_folder_path", args).await {
                                                            result => {
                                                                if let Ok(Some(conversion)) = serde_wasm_bindgen::from_value::<Option<serde_json::Value>>(result) {
                                                                    // Extract ID from the conversion
                                                                    let conversion_id = conversion.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());

                                                                    // Extract settings from the conversion
                                                                    if let Some(settings) = conversion.get("settings") {
                                                                        if let Ok(conv_settings) = serde_json::from_value::<ConversionSettings>(settings.clone()) {
                                                                            on_settings_loaded.emit(Some((conv_settings, conversion_id)));
                                                                            return;
                                                                        }
                                                                    }
                                                                }
                                                                // No conversion found or failed to parse
                                                                on_settings_loaded.emit(None);
                                                            }
                                                        }
                                                    });
                                                })
                                            };

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
    }
}
