use yew::prelude::*;
use yew_icons::{Icon, IconId};
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

#[derive(serde::Serialize)]
struct UpdateFrameCustomNameInvokeArgs {
    request: UpdateFrameCustomNameRequest,
}

#[derive(serde::Serialize)]
struct UpdateFrameCustomNameRequest {
    #[serde(rename = "folderPath")]
    folder_path: String,
    #[serde(rename = "customName")]
    custom_name: Option<String>,
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
    pub on_rename_frame: Option<Callback<(String, String)>>,
    pub on_delete_frame: Option<Callback<FrameDirectory>>,
    #[prop_or_default]
    pub on_open_frame: Option<Callback<FrameDirectory>>,
}

pub struct AvailableFrames {
    renaming_id: Option<String>,
    rename_value: String,
    is_saving: bool,
    menu_open_id: Option<String>,
}

pub enum AvailableFramesMsg {
    StartRename(String, String),
    UpdateRenameValue(String),
    SaveRename(String, String),
    CancelRename,
    SetSaving(bool),
    ToggleMenu(String),
    CloseMenu,
}

impl Component for AvailableFrames {
    type Message = AvailableFramesMsg;
    type Properties = AvailableFramesProps;

    fn create(_: &Context<Self>) -> Self {
        Self {
            renaming_id: None,
            rename_value: String::new(),
            is_saving: false,
            menu_open_id: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AvailableFramesMsg::StartRename(id, current_name) => {
                self.renaming_id = Some(id);
                self.rename_value = current_name;
                self.menu_open_id = None;
                true
            }
            AvailableFramesMsg::UpdateRenameValue(value) => {
                self.rename_value = value;
                true
            }
            AvailableFramesMsg::SaveRename(frame_path, new_name) => {
                let frame_path_clone = frame_path.clone();
                let new_name_clone = if new_name.trim().is_empty() {
                    None
                } else {
                    Some(new_name.trim().to_string())
                };

                // Get the callback for refreshing
                let on_rename_frame = ctx.props().on_rename_frame.clone();

                self.renaming_id = None;
                self.rename_value = String::new();

                wasm_bindgen_futures::spawn_local(async move {
                    // Call the Tauri command to update custom name
                    let invoke_args = UpdateFrameCustomNameInvokeArgs {
                        request: UpdateFrameCustomNameRequest {
                            folder_path: frame_path_clone.clone(),
                            custom_name: new_name_clone.clone(),
                        },
                    };
                    let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                    let _ = tauri_invoke("update_frame_custom_name", args).await;

                    // Trigger refresh after successful save
                    if let Some(on_rename_frame) = on_rename_frame {
                        on_rename_frame.emit((frame_path_clone, new_name_clone.unwrap_or_default()));
                    }
                });

                true
            }
            AvailableFramesMsg::CancelRename => {
                self.renaming_id = None;
                self.rename_value = String::new();
                true
            }
            AvailableFramesMsg::SetSaving(value) => {
                self.is_saving = value;
                true
            }
            AvailableFramesMsg::ToggleMenu(id) => {
                if self.menu_open_id.as_ref() == Some(&id) {
                    self.menu_open_id = None;
                } else {
                    self.menu_open_id = Some(id);
                }
                true
            }
            AvailableFramesMsg::CloseMenu => {
                self.menu_open_id = None;
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
                                {for props.frame_directories.iter().map(|frame_dir| {
                                    let class_name = if props.selected_frame_dir.as_ref()
                                        .map(|s| s.directory_path == frame_dir.directory_path)
                                        .unwrap_or(false) {
                                        "source-item selected"
                                    } else {
                                        "source-item"
                                    };

                                    let is_renaming = self.renaming_id.as_ref().map(|id| id == &frame_dir.directory_path).unwrap_or(false);
                                    let is_menu_open = self.menu_open_id.as_ref().map(|id| id == &frame_dir.directory_path).unwrap_or(false);

                                    let onclick = {
                                        let on_select = props.on_select_frame_dir.clone();
                                        let on_settings_loaded = props.on_frame_settings_loaded.clone();
                                        let frame_clone = frame_dir.clone();

                                        Callback::from(move |_| {
                                            on_select.emit(frame_clone.clone());

                                            // Fetch conversion settings for this frame directory
                                            let on_settings_loaded = on_settings_loaded.clone();
                                            let directory_path = frame_clone.directory_path.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                let args = serde_wasm_bindgen::to_value(&json!({ "folderPath": directory_path })).unwrap();
                                                match tauri_invoke("get_conversion_by_folder_path", args).await {
                                                    result => {
                                                        if let Ok(Some(conversion)) = serde_wasm_bindgen::from_value::<Option<serde_json::Value>>(result) {
                                                            let conversion_id = conversion.get("id").and_then(|id| id.as_str()).map(|s| s.to_string());
                                                            if let Some(settings) = conversion.get("settings") {
                                                                if let Ok(conv_settings) = serde_json::from_value::<ConversionSettings>(settings.clone()) {
                                                                    on_settings_loaded.emit(Some((conv_settings, conversion_id)));
                                                                    return;
                                                                }
                                                            }
                                                        }
                                                        on_settings_loaded.emit(None);
                                                    }
                                                }
                                            });
                                        })
                                    };

                                    // Rename action
                                    let on_rename_click = {
                                        let link = ctx.link().clone();
                                        let frame_id = frame_dir.directory_path.clone();
                                        let frame_display_name = frame_dir.name.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            link.send_message(AvailableFramesMsg::StartRename(frame_id.clone(), frame_display_name.clone()));
                                        })
                                    };

                                    // Open action
                                    let on_open_click = props.on_open_frame.as_ref().map(|cb| {
                                        let cb = cb.clone();
                                        let frame_clone = frame_dir.clone();
                                        let link = ctx.link().clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            cb.emit(frame_clone.clone());
                                            link.send_message(AvailableFramesMsg::CloseMenu);
                                        })
                                    });

                                    // Delete action
                                    let on_delete_click = props.on_delete_frame.as_ref().map(|cb| {
                                        let cb = cb.clone();
                                        let frame_clone = frame_dir.clone();
                                        let link = ctx.link().clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            cb.emit(frame_clone.clone());
                                            link.send_message(AvailableFramesMsg::CloseMenu);
                                        })
                                    });

                                    // Menu toggle handler
                                    let on_menu_toggle = {
                                        let link = ctx.link().clone();
                                        let menu_id = frame_dir.directory_path.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            link.send_message(AvailableFramesMsg::ToggleMenu(menu_id.clone()));
                                        })
                                    };

                                    html! {
                                        <div class={class_name} {onclick}>
                                            {if is_renaming {
                                                let link = ctx.link().clone();
                                                let frame_id = frame_dir.directory_path.clone();
                                                let rename_value = self.rename_value.clone();

                                                html! {
                                                    <textarea
                                                        class="source-item-rename-input"
                                                        value={rename_value}
                                                        oninput={link.callback(move |e: InputEvent| {
                                                            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                            AvailableFramesMsg::UpdateRenameValue(input.value())
                                                        })}
                                                        onkeydown={{
                                                            let link = link.clone();
                                                            let frame_id = frame_id.clone();
                                                            Callback::from({
                                                                let link = link.clone();
                                                                let frame_id = frame_id.clone();
                                                                move |e: KeyboardEvent| {
                                                                    if e.key() == "Enter" && !e.shift_key() {
                                                                        e.prevent_default();
                                                                        let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                                        link.send_message(AvailableFramesMsg::SetSaving(true));
                                                                        link.send_message(AvailableFramesMsg::SaveRename(frame_id.clone(), input.value()));
                                                                    } else if e.key() == "Escape" {
                                                                        e.prevent_default();
                                                                        link.send_message(AvailableFramesMsg::CancelRename);
                                                                    }
                                                                }
                                                            })
                                                        }}
                                                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                                                        onblur={{
                                                            let link = link.clone();
                                                            Callback::from(move |_| {
                                                                link.send_message(AvailableFramesMsg::CancelRename);
                                                            })
                                                        }}
                                                        autofocus=true
                                                        rows={3}
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <div class="source-item-name-wrapper"><span class="source-item-name">{ &frame_dir.name }</span></div>
                                                        <div class="source-item-buttons">
                                                            <div class="item-menu-container">
                                                                <button type="button" class="source-item-btn menu-btn" onclick={on_menu_toggle} title="More options">
                                                                    <Icon icon_id={IconId::LucideMoreHorizontal} width="14px" height="14px" />
                                                                </button>
                                                                {if is_menu_open {
                                                                    html! {
                                                                        <div class="item-dropdown-menu">
                                                                            <button type="button" class="dropdown-menu-item" onclick={on_rename_click}>
                                                                                <Icon icon_id={IconId::LucidePencil} width="14px" height="14px" />
                                                                                <span>{"Rename"}</span>
                                                                            </button>
                                                                            {if let Some(on_open) = on_open_click {
                                                                                html! {
                                                                                    <button type="button" class="dropdown-menu-item" onclick={on_open}>
                                                                                        <Icon icon_id={IconId::LucideFolderOpen} width="14px" height="14px" />
                                                                                        <span>{"Open"}</span>
                                                                                    </button>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }}
                                                                            {if let Some(on_delete) = on_delete_click {
                                                                                html! {
                                                                                    <button type="button" class="dropdown-menu-item delete" onclick={on_delete}>
                                                                                        <Icon icon_id={IconId::LucideTrash2} width="14px" height="14px" />
                                                                                        <span>{"Delete"}</span>
                                                                                    </button>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }}
                                                                        </div>
                                                                    }
                                                                } else {
                                                                    html! {}
                                                                }}
                                                            </div>
                                                        </div>
                                                    </>
                                                }
                                            }}
                                        </div>
                                    }
                                })}
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
