use yew::prelude::*;
use yew_icons::{Icon, IconId};
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoCut {
    pub id: String,
    pub project_id: String,
    pub source_file_id: String,
    pub file_path: String,
    pub date_added: String,
    pub size: i64,
    pub custom_name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    pub duration: f64,
}

#[derive(Serialize)]
struct RenameCutInvokeArgs {
    request: RenameCutRequest,
}

#[derive(Serialize)]
struct RenameCutRequest {
    cut_id: String,
    custom_name: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct AvailableCutsProps {
    pub cuts: Vec<VideoCut>,
    pub selected_cut: Option<VideoCut>,
    pub cuts_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub on_select_cut: Callback<VideoCut>,
    pub on_delete_cut: Option<Callback<VideoCut>>,
    pub on_rename_cut: Option<Callback<(String, String)>>,
    #[prop_or_default]
    pub on_open_cut: Option<Callback<VideoCut>>,
}

pub struct AvailableCuts {
    renaming_id: Option<String>,
    rename_value: String,
    menu_open_id: Option<String>,
}

pub enum AvailableCutsMsg {
    StartRename(String, String),
    UpdateRenameValue(String),
    SaveRename(String, String),
    CancelRename,
    ToggleMenu(String),
    CloseMenu,
}

impl Component for AvailableCuts {
    type Message = AvailableCutsMsg;
    type Properties = AvailableCutsProps;

    fn create(_: &Context<Self>) -> Self {
        Self {
            renaming_id: None,
            rename_value: String::new(),
            menu_open_id: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            AvailableCutsMsg::StartRename(id, current_name) => {
                self.renaming_id = Some(id);
                self.rename_value = current_name;
                self.menu_open_id = None;
                true
            }
            AvailableCutsMsg::UpdateRenameValue(value) => {
                self.rename_value = value;
                true
            }
            AvailableCutsMsg::SaveRename(cut_id, new_name) => {
                let cut_id_clone = cut_id.clone();
                let new_name_clone = if new_name.trim().is_empty() {
                    None
                } else {
                    Some(new_name.trim().to_string())
                };

                let on_rename_cut = ctx.props().on_rename_cut.clone();

                self.renaming_id = None;
                self.rename_value = String::new();

                wasm_bindgen_futures::spawn_local(async move {
                    let invoke_args = RenameCutInvokeArgs {
                        request: RenameCutRequest {
                            cut_id: cut_id_clone.clone(),
                            custom_name: new_name_clone.clone(),
                        },
                    };
                    let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                    let _ = tauri_invoke("rename_cut", args).await;

                    if let Some(on_rename_cut) = on_rename_cut {
                        on_rename_cut.emit((cut_id_clone, new_name_clone.unwrap_or_default()));
                    }
                });

                true
            }
            AvailableCutsMsg::CancelRename => {
                self.renaming_id = None;
                self.rename_value = String::new();
                true
            }
            AvailableCutsMsg::ToggleMenu(id) => {
                if self.menu_open_id.as_ref() == Some(&id) {
                    self.menu_open_id = None;
                } else {
                    self.menu_open_id = Some(id);
                }
                true
            }
            AvailableCutsMsg::CloseMenu => {
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

        // Helper to format time as MM:SS
        let format_time = |secs: f64| -> String {
            let total_secs = secs.floor() as u32;
            let mins = total_secs / 60;
            let s = total_secs % 60;
            format!("{:02}:{:02}", mins, s)
        };

        html! {
            <div class="cuts-column">
                <h2 class="collapsible-header" onclick={on_toggle}>
                    <span class="chevron-icon">
                        {if props.cuts_collapsed {
                            html! {<span>{"▶"}</span>}
                        } else {
                            html! {<span>{"▼"}</span>}
                        }}
                    </span>
                    <span>{"VIDEO CUTS"}</span>
                </h2>
                {
                    if !props.cuts_collapsed {
                        html! {
                            <div class="source-list">
                                {for props.cuts.iter().map(|cut| {
                                    let class_name = if props.selected_cut.as_ref()
                                        .map(|s| s.id == cut.id)
                                        .unwrap_or(false) {
                                        "source-item selected"
                                    } else {
                                        "source-item"
                                    };

                                    let is_renaming = self.renaming_id.as_ref().map(|id| id == &cut.id).unwrap_or(false);
                                    let is_menu_open = self.menu_open_id.as_ref().map(|id| id == &cut.id).unwrap_or(false);

                                    let display_name = cut.custom_name.clone()
                                        .unwrap_or_else(|| {
                                            format!("Cut {} - {}",
                                                format_time(cut.start_time),
                                                format_time(cut.end_time))
                                        });

                                    let onclick = {
                                        let on_select = props.on_select_cut.clone();
                                        let cut_clone = cut.clone();
                                        Callback::from(move |_| on_select.emit(cut_clone.clone()))
                                    };

                                    // Rename action
                                    let on_rename_click = {
                                        let link = ctx.link().clone();
                                        let cut_id = cut.id.clone();
                                        let cut_display_name = display_name.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            link.send_message(AvailableCutsMsg::StartRename(cut_id.clone(), cut_display_name.clone()));
                                        })
                                    };

                                    // Open action
                                    let on_open_click = props.on_open_cut.as_ref().map(|cb| {
                                        let cb = cb.clone();
                                        let cut_clone = cut.clone();
                                        let link = ctx.link().clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            cb.emit(cut_clone.clone());
                                            link.send_message(AvailableCutsMsg::CloseMenu);
                                        })
                                    });

                                    // Delete action
                                    let on_delete_click = props.on_delete_cut.as_ref().map(|cb| {
                                        let cb = cb.clone();
                                        let cut_clone = cut.clone();
                                        let link = ctx.link().clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            cb.emit(cut_clone.clone());
                                            link.send_message(AvailableCutsMsg::CloseMenu);
                                        })
                                    });

                                    // Menu toggle handler
                                    let on_menu_toggle = {
                                        let link = ctx.link().clone();
                                        let menu_id = cut.id.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            link.send_message(AvailableCutsMsg::ToggleMenu(menu_id.clone()));
                                        })
                                    };

                                    html! {
                                        <div class={class_name} {onclick}>
                                            {if is_renaming {
                                                let link = ctx.link().clone();
                                                let cut_id = cut.id.clone();
                                                let rename_value = self.rename_value.clone();

                                                html! {
                                                    <textarea
                                                        class="source-item-rename-input"
                                                        value={rename_value}
                                                        oninput={link.callback(move |e: InputEvent| {
                                                            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                            AvailableCutsMsg::UpdateRenameValue(input.value())
                                                        })}
                                                        onkeydown={{
                                                            let link = link.clone();
                                                            let cut_id = cut_id.clone();
                                                            Callback::from({
                                                                let link = link.clone();
                                                                let cut_id = cut_id.clone();
                                                                move |e: KeyboardEvent| {
                                                                    if e.key() == "Enter" && !e.shift_key() {
                                                                        e.prevent_default();
                                                                        let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                                        link.send_message(AvailableCutsMsg::SaveRename(cut_id.clone(), input.value()));
                                                                    } else if e.key() == "Escape" {
                                                                        e.prevent_default();
                                                                        link.send_message(AvailableCutsMsg::CancelRename);
                                                                    }
                                                                }
                                                            })
                                                        }}
                                                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                                                        onblur={{
                                                            let link = link.clone();
                                                            Callback::from(move |_| {
                                                                link.send_message(AvailableCutsMsg::CancelRename);
                                                            })
                                                        }}
                                                        autofocus=true
                                                        rows={2}
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <div class="source-item-name-wrapper"><span class="source-item-name">{display_name}</span></div>
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
                                {if props.cuts.is_empty() {
                                    html! {
                                        <div class="empty-message">{"No cuts yet. Use the scissors button to create one."}</div>
                                    }
                                } else {
                                    html! {<></>}
                                }}
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
