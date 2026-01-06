use yew::prelude::*;
use yew_icons::{Icon, IconId};
use crate::pages::project::SourceContent;
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
pub struct SourceFilesProps {
    pub source_files: Vec<SourceContent>,
    pub selected_source: Option<SourceContent>,
    pub source_files_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub on_select_source: Callback<SourceContent>,
    pub on_add_files: Option<Callback<()>>,
    pub on_delete_file: Option<Callback<SourceContent>>,
    pub on_rename_file: Option<Callback<SourceContent>>,
}

pub struct SourceFiles {
    renaming_id: Option<String>,
    rename_value: String,
    is_saving: bool,
}

pub enum SourceFilesMsg {
    StartRename(String, String),
    UpdateRenameValue(String),
    SaveRename(String, String),
    CancelRename,
    SetSaving(bool),
}

#[derive(Clone, PartialEq)]
pub struct SourceFilesComponent;

impl Component for SourceFiles {
    type Message = SourceFilesMsg;
    type Properties = SourceFilesProps;

    fn create(_: &Context<Self>) -> Self {
        Self {
            renaming_id: None,
            rename_value: String::new(),
            is_saving: false,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            SourceFilesMsg::StartRename(id, current_name) => {
                self.renaming_id = Some(id);
                self.rename_value = current_name;
                true
            }
            SourceFilesMsg::UpdateRenameValue(value) => {
                self.rename_value = value;
                true
            }
            SourceFilesMsg::SaveRename(source_id, new_name) => {
                let source_id_clone = source_id.clone();
                let new_name_clone = if new_name.trim().is_empty() {
                    None
                } else {
                    Some(new_name.trim().to_string())
                };
                
                // Find the source file to pass to the callback
                let source_file = ctx.props().source_files.iter()
                    .find(|f| f.id == source_id_clone)
                    .cloned();
                
                // Get the callback for refreshing
                let on_rename_file = ctx.props().on_rename_file.clone();
                
                self.renaming_id = None;
                self.rename_value = String::new();
                
                wasm_bindgen_futures::spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&json!({
                        "sourceId": source_id_clone,
                        "customName": new_name_clone
                    })).unwrap();
                    let _ = tauri_invoke("rename_source_file", args).await;
                    
                    // Trigger refresh after successful save
                    if let (Some(on_rename_file), Some(file)) = (on_rename_file, source_file) {
                        on_rename_file.emit(file);
                    }
                });
                
                true
            }
            SourceFilesMsg::CancelRename => {
                self.renaming_id = None;
                self.rename_value = String::new();
                true
            }
            SourceFilesMsg::SetSaving(value) => {
                self.is_saving = value;
                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let props = ctx.props();
        
        let on_toggle = {
            let on_toggle_collapsed = props.on_toggle_collapsed.clone();
            Callback::from(move |_| {
                on_toggle_collapsed.emit(());
            })
        };

        let source_files = &props.source_files;
        let selected_source = &props.selected_source;
        let on_select_source = &props.on_select_source;

        html! {
            <div class="source-files-column">
                <h2 class="collapsible-header">
                    <span class="chevron-icon" onclick={&on_toggle}>
                        {if props.source_files_collapsed {
                            html! {<span>{"▶"}</span>}
                        } else {
                            html! {<span>{"▼"}</span>}
                        }}
                    </span>
                    <span onclick={&on_toggle}>{"SOURCE FILES"}</span>
                    {if let Some(on_add_files) = &props.on_add_files {
                        let on_add = {
                            let on_add_files = on_add_files.clone();
                            Callback::from(move |_| {
                                web_sys::console::log_1(&"➕ Add files button clicked in SourceFiles component".into());
                                on_add_files.emit(());
                            })
                        };
                        html! {
                            <button type="button" class="add-files-btn" onclick={on_add} title="Add files">
                                {"+"}
                            </button>
                        }
                    } else {
                        web_sys::console::log_1(&"⚠️ No on_add_files callback provided to SourceFiles".into());
                        html! {}
                    }}
                </h2>
                {
                    if !props.source_files_collapsed {
                        html! {
                            <div class="source-list">
                            {
                                source_files.iter().map(|file| {
                                    let display_name = file.custom_name.as_ref()
                                        .map(|n| n.as_str())
                                        .unwrap_or_else(|| {
                                            std::path::Path::new(&file.file_path)
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or(&file.file_path)
                                        });

                                    let on_select = on_select_source.clone();
                                    let file_clone = file.clone();
                                    let is_selected = selected_source.as_ref().map(|s| s.id == file.id).unwrap_or(false);
                                    let onclick = Callback::from(move |_| on_select.emit(file_clone.clone()));

                                    let class_name = if is_selected {"source-item selected"} else {"source-item"};

                                    let is_renaming = self.renaming_id.as_ref().map(|id| id == &file.id).unwrap_or(false);
                                    let link = ctx.link().clone();
                                    let file_id = file.id.clone();
                                    let file_display_name = display_name.to_string();

                                    // Delete button handler
                                    let on_delete = if let Some(on_delete_file) = &props.on_delete_file {
                                        let on_delete_file = on_delete_file.clone();
                                        let file_clone = file.clone();
                                        Some(Callback::from(move |e: web_sys::MouseEvent| {
                                            e.stop_propagation();
                                            on_delete_file.emit(file_clone.clone());
                                        }))
                                    } else {
                                        None
                                    };

                                    // Rename button handler
                                    let on_rename = {
                                        let link = link.clone();
                                        let file_id = file_id.clone();
                                        let file_display_name = file_display_name.clone();
                                        Some(Callback::from(move |e: web_sys::MouseEvent| {
                                            e.stop_propagation();
                                            link.send_message(SourceFilesMsg::StartRename(file_id.clone(), file_display_name.clone()));
                                        }))
                                    };

                                    html! {
                                        <div
                                            class={class_name}
                                            key={file.id.clone()}
                                            {onclick}
                                        >
                                            {if is_renaming {
                                                let link = link.clone();
                                                let file_id = file_id.clone();
                                                let rename_value = self.rename_value.clone();
                                                html! {
                                                    <textarea
                                                        class="source-item-rename-input"
                                                        value={rename_value.clone()}
                                                        oninput={link.callback(move |e: InputEvent| {
                                                            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                            SourceFilesMsg::UpdateRenameValue(input.value())
                                                        })}
                                                        onkeydown={{
                                                            let link = link.clone();
                                                            let file_id = file_id.clone();
                                                            Callback::from({
                                                                let link = link.clone();
                                                                let file_id = file_id.clone();
                                                                move |e: KeyboardEvent| {
                                                                    if e.key() == "Enter" && !e.shift_key() {
                                                                        e.prevent_default();
                                                                        let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                                        link.send_message(SourceFilesMsg::SetSaving(true));
                                                                        link.send_message(SourceFilesMsg::SaveRename(file_id.clone(), input.value()));
                                                                    } else if e.key() == "Escape" {
                                                                        e.prevent_default();
                                                                        link.send_message(SourceFilesMsg::CancelRename);
                                                                    }
                                                                }
                                                            })
                                                        }}
                                                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                                                        onblur={{
                                                            let link = link.clone();
                                                            let is_saving = self.is_saving;
                                                            Callback::from(move |_| {
                                                                if !is_saving {
                                                                    link.send_message(SourceFilesMsg::CancelRename);
                                                                } else {
                                                                    link.send_message(SourceFilesMsg::SetSaving(false));
                                                                }
                                                            })
                                                        }}
                                                        autofocus=true
                                                        rows={3}
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <span class="source-item-name">{display_name}</span>
                                                        <div class="source-item-buttons">
                                                            <button
                                                                type="button"
                                                                class="source-item-btn rename-btn"
                                                                onclick={on_rename}
                                                                title="Rename file"
                                                            >
                                                                <Icon icon_id={IconId::LucidePencil} width="30px" height="30px" />
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="source-item-btn delete-btn"
                                                                onclick={on_delete}
                                                                title={if on_delete.is_none() {"Delete functionality not yet implemented"} else {"Delete file"}}
                                                                disabled={on_delete.is_none()}
                                                            >
                                                                <Icon icon_id={IconId::LucideXCircle} width="30px" height="30px" />
                                                            </button>
                                                        </div>
                                                    </>
                                                }
                                            }}
                                        </div>
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
        }
    }
}
