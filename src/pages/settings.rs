use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_icons::{Icon, IconId};
use serde_json::json;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum DefaultBehavior { Move, Copy }

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum DeleteMode { Soft, Hard }

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    pub id: Option<i64>,
    pub output_directory: String,
    pub default_behavior: DefaultBehavior,
    pub delete_mode: DeleteMode,
    pub debug_logs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            id: None,
            output_directory: String::new(),
            default_behavior: DefaultBehavior::Move,
            delete_mode: DeleteMode::Soft,
            debug_logs: true,
        }
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[function_component(SettingsPage)]
pub fn settings_page() -> Html {
    let settings = use_state(Settings::default);

    { // load once
        let settings = settings.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                let v = invoke("load_settings", JsValue::NULL).await;
                if let Ok(s) = serde_wasm_bindgen::from_value::<Settings>(v) {
                    settings.set(s);
                }
            });
            || ()
        });
    }

    let on_pick_directory = {
        let settings = settings.clone();
        Callback::from(move |_| {
            let s = (*settings).clone();
            web_sys::window().unwrap().alert_with_message(&format!("Current: {}", s.output_directory)).ok();
        })
    };

    let on_dir_input = {
        let settings = settings.clone();
        Callback::from(move |e: InputEvent| {
            let v = e.target_unchecked_into::<web_sys::HtmlInputElement>().value();
            let mut s = (*settings).clone();
            s.output_directory = v;
            settings.set(s);
        })
    };

    let on_behavior_change = {
        let settings = settings.clone();
        Callback::from(move |e: Event| {
            let v = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            let mut s = (*settings).clone();
            s.default_behavior = if v == "Copy" { DefaultBehavior::Copy } else { DefaultBehavior::Move };
            settings.set(s);
        })
    };

    let on_delete_mode_change = {
        let settings = settings.clone();
        Callback::from(move |e: Event| {
            let v = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            let mut s = (*settings).clone();
            s.delete_mode = if v == "Hard" { DeleteMode::Hard } else { DeleteMode::Soft };
            settings.set(s);
        })
    };

    let on_debug_change = {
        let settings = settings.clone();
        Callback::from(move |e: Event| {
            let v = e.target_unchecked_into::<web_sys::HtmlInputElement>().checked();
            let mut s = (*settings).clone();
            s.debug_logs = v;
            settings.set(s);
        })
    };

    let on_save = {
        let settings = settings.clone();
        Callback::from(move |_| {
            let s = (*settings).clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&json!({ "settings": s })).unwrap();
                let _ = invoke("save_settings", args).await;
            });
        })
    };

    html! {
        <main class="container">
            <h1>{"Settings"}</h1>
            <div class="settings-form">
                <div class="form-group">
                    <label for="out-dir">{"Output directory"}</label>
                    <div class="input-group">
                        <input id="out-dir" readonly=true value={settings.output_directory.clone()} oninput={on_dir_input} />
                        <button type="button" onclick={on_pick_directory}>{"Browse"}</button>
                        <button type="button" class="icon-btn">
                            <Icon icon_id={IconId::LucideFolder} width={"18"} height={"18"} />
                        </button>
                    </div>
                </div>

                <div class="form-group row">
                    <label for="behavior">{"Default behavior"}</label>
                    <select id="behavior" onchange={on_behavior_change}>
                        <option value="Move" selected={settings.default_behavior == DefaultBehavior::Move}>{"Move"}</option>
                        <option value="Copy" selected={settings.default_behavior == DefaultBehavior::Copy}>{"Copy"}</option>
                    </select>
                </div>

                <div class="form-group row">
                    <label for="del">{"Delete mode"}</label>
                    <select id="del" onchange={on_delete_mode_change}>
                        <option value="Soft" selected={settings.delete_mode == DeleteMode::Soft}>{"Soft"}</option>
                        <option value="Hard" selected={settings.delete_mode == DeleteMode::Hard}>{"Hard"}</option>
                    </select>
                </div>

                <div class="form-group row">
                    <label for="dbg">{"Debug logs"}</label>
                    <input id="dbg" type="checkbox" checked={settings.debug_logs} onchange={on_debug_change} />
                </div>

                <div class="form-group center">
                    <button type="button" onclick={on_save}>{"Save"}</button>
                </div>
            </div>
        </main>
    }
}


