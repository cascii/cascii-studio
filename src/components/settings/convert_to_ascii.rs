use yew::prelude::*;
use yew_icons::{Icon, IconId};
use crate::pages::project::SourceContent;
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiRequest {
    file_path: String,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: Option<u32>,
    project_id: String,
    source_file_id: String,
    color: bool,
}

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiInvokeArgs {
    request: ConvertToAsciiRequest,
}

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
pub struct ConvertToAsciiProps {
    pub selected_source: Option<SourceContent>,
    pub convert_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub luminance: u8,
    pub on_luminance_change: Callback<u8>,
    pub font_ratio: f32,
    pub on_font_ratio_change: Callback<f32>,
    pub columns: u32,
    pub on_columns_change: Callback<u32>,
    pub fps: u32,
    pub on_fps_change: Callback<u32>,
    pub is_converting: bool,
    pub on_is_converting_change: Callback<bool>,
    pub conversion_message: Option<String>,
    pub on_conversion_message_change: Callback<Option<String>>,
    pub error_message: Option<String>,
    pub on_error_message_change: Callback<Option<String>>,
    pub project_id: String,
    pub on_refresh_frames: Callback<()>,
}

#[function_component(ConvertToAscii)]
pub fn convert_to_ascii(props: &ConvertToAsciiProps) -> Html {
    // State for color generation toggle
    let generate_colors = use_state(|| true);

    let on_toggle_colors = {
        let generate_colors = generate_colors.clone();
        Callback::from(move |_| {
            generate_colors.set(!*generate_colors);
        })
    };

    let on_toggle = {
        let on_toggle_collapsed = props.on_toggle_collapsed.clone();
        Callback::from(move |_| {
            on_toggle_collapsed.emit(());
        })
    };

    let on_luminance_input = {
        let on_luminance_change = props.on_luminance_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(val) = input.value().parse::<u8>() {
                        on_luminance_change.emit(val);
                    }
                }
            }
        })
    };

    let on_font_ratio_input = {
        let on_font_ratio_change = props.on_font_ratio_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(val) = input.value().parse::<f32>() {
                        on_font_ratio_change.emit(val);
                    }
                }
            }
        })
    };

    let on_columns_input = {
        let on_columns_change = props.on_columns_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(val) = input.value().parse::<u32>() {
                        on_columns_change.emit(val);
                    }
                }
            }
        })
    };

    let on_fps_input = {
        let on_fps_change = props.on_fps_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(val) = input.value().parse::<u32>() {
                        on_fps_change.emit(val);
                    }
                }
            }
        })
    };

    let on_convert_click = {
        let selected_source = props.selected_source.clone();
        let luminance = props.luminance;
        let font_ratio = props.font_ratio;
        let columns = props.columns;
        let fps = props.fps;
        let project_id = props.project_id.clone();
        let generate_colors = generate_colors.clone();
        let on_is_converting_change = props.on_is_converting_change.clone();
        let on_conversion_message_change = props.on_conversion_message_change.clone();
        let on_error_message_change = props.on_error_message_change.clone();
        let on_refresh_frames = props.on_refresh_frames.clone();

        Callback::from(move |_| {
            let color = *generate_colors;
            if let Some(source) = &selected_source {
                let file_path = source.file_path.clone();
                let source_file_id = source.id.clone();
                let project_id_clone = project_id.clone();
                let on_is_converting_change = on_is_converting_change.clone();
                let on_conversion_message_change = on_conversion_message_change.clone();
                let on_error_message_change = on_error_message_change.clone();
                let on_refresh_frames = on_refresh_frames.clone();

                on_is_converting_change.emit(true);
                on_conversion_message_change.emit(None);

                wasm_bindgen_futures::spawn_local(async move {
                    let invoke_args = ConvertToAsciiInvokeArgs {
                        request: ConvertToAsciiRequest {file_path, luminance, font_ratio, columns, fps: Some(fps), project_id: project_id_clone.clone(), source_file_id, color}
                    };

                    let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();

                    match tauri_invoke("convert_to_ascii", args).await {
                        result => {
                            on_is_converting_change.emit(false);
                            match serde_wasm_bindgen::from_value::<String>(result) {
                                Ok(msg) => {
                                    on_conversion_message_change.emit(Some(msg));
                                    on_error_message_change.emit(None);

                                    // Refresh frame directories after conversion
                                    let args = serde_wasm_bindgen::to_value(&json!({ "projectId": project_id_clone })).unwrap();
                                    match tauri_invoke("get_project_frames", args).await {
                                        result => {
                                            if let Ok(_frames) = serde_wasm_bindgen::from_value::<Vec<serde_json::Value>>(result) {
                                                on_refresh_frames.emit(());
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    on_error_message_change.emit(Some("Failed to convert to ASCII. Please check the file path and try again.".to_string()));
                                    on_conversion_message_change.emit(None);
                                }
                            }
                        }
                    }
                });
            }
        })
    };

    html! {
        <div class="ascii-conversion-column">
            <h2 class="collapsible-header" onclick={on_toggle}>
                <span class="chevron-icon">
                    {if props.convert_collapsed {
                        html! {<span>{"▶"}</span>}
                    } else {
                        html! {<span>{"▼"}</span>}
                    }}
                </span>
                <span>{"CONVERT TO ASCII"}</span>
            </h2>

            {
                if !props.convert_collapsed {
                    html! {
                        <>
                            <div class="conversion-settings">
                                <div class="setting-row">
                                    <label>{"Luminance:"}</label>
                                    <input type="number" class="setting-input" value={props.luminance.to_string()} min="0" max="255" oninput={on_luminance_input} />
                                </div>

                                <div class="setting-row">
                                    <label>{"Font Ratio:"}</label>
                                    <input type="number" class="setting-input" value={props.font_ratio.to_string()} min="0.1" max="2.0" step="0.1" oninput={on_font_ratio_input} />
                                </div>

                                <div class="setting-row">
                                    <label>{"Columns:"}</label>
                                    <input type="number" class="setting-input" value={props.columns.to_string()} min="1" max="2000" oninput={on_columns_input} />
                                </div>

                                {
                                    if props.selected_source.as_ref().map(|s| s.content_type == "Video").unwrap_or(false) {
                                        html! {
                                            <div class="setting-row">
                                                <label>{"FPS:"}</label>
                                                <input type="number" class="setting-input" value={props.fps.to_string()} min="1" max="120" oninput={on_fps_input} />
                                            </div>
                                        }
                                    } else {
                                        html! {<></>}
                                    }
                                }
                            </div>

                            <div class="convert-actions">
                                <button class={classes!("color-toggle-btn", (*generate_colors).then_some("active"))} onclick={on_toggle_colors} title={if *generate_colors { "Color generation enabled" } else { "Color generation disabled" }}>
                                    if *generate_colors {
                                        <Icon icon_id={IconId::LucideBrush} width={"18"} height={"18"} />
                                    } else {
                                        <Icon icon_id={IconId::LucideXCircle} width={"18"} height={"18"} />
                                    }
                                </button>
                                <button class="btn-convert" disabled={props.is_converting || props.selected_source.is_none()} onclick={on_convert_click}>
                                    if props.is_converting {
                                        {"Converting..."}
                                    } else {
                                        {"Convert to ASCII"}
                                    }
                                </button>
                            </div>

                            {
                                if let Some(msg) = &props.conversion_message {
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
    }
}
