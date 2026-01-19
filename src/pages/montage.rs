use yew::prelude::*;
use wasm_bindgen::prelude::*;
use serde_json::json;

use super::open::Project;

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
pub struct MontagePageProps {
    pub project_id: String,
}

#[function_component(MontagePage)]
pub fn montage_page(props: &MontagePageProps) -> Html {
    let project = use_state(|| None::<Project>);
    let error_message = use_state(|| Option::<String>::None);

    // Load project details
    {
        let project_id = props.project_id.clone();
        let project = project.clone();
        let error_message = error_message.clone();

        use_effect_with(project_id.clone(), move |id| {
            let id = id.clone();
            wasm_bindgen_futures::spawn_local(async move {
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
            });

            || ()
        });
    }

    html! {
        <div class="container montage-page">
            <h1>{ project.as_ref().map(|p| format!("Montage: {}", p.project_name)).unwrap_or_else(|| "Loading Montage...".into()) }</h1>

            if let Some(error) = &*error_message {
                <div class="alert alert-error">{error}</div>
            }

            <div class="montage-content">
                <p>{"Montage editor coming soon..."}</p>
            </div>
        </div>
    }
}
