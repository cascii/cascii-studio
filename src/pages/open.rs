use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[wasm_bindgen(inline_js = r#"
export async function openProjectsTauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args);
  throw new Error('Tauri invoke is not available on this page');
}

export async function openProjectsTauriListen(event, handler) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, handler);
  throw new Error('Tauri event.listen is not available on this page');
}

export async function openProjectsTauriUnlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch, js_name = openProjectsTauriInvoke)]
    async fn open_projects_tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = openProjectsTauriListen)]
    async fn open_projects_tauri_listen(
        event: &str,
        handler: &js_sys::Function,
    ) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = openProjectsTauriUnlisten)]
    async fn open_projects_tauri_unlisten(unlisten_fn: JsValue) -> Result<(), JsValue>;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProjectType {
    Image,
    Animation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub project_name: String,
    pub project_type: ProjectType,
    pub project_path: String,
    pub size: i64,
    pub frames: i32,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Properties, PartialEq)]
pub struct OpenPageProps {
    pub on_open_project: Callback<String>,
    pub explorer_on_left: bool,
    #[prop_or_default]
    pub on_open_montage: Option<Callback<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShowOpenProjectsContextMenuRequest {
    project_id: String,
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenProjectsContextMenuActionPayload {
    project_id: String,
    action: String,
}

fn js_error_to_string(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "Unknown Tauri error".to_string())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenProjectSortKey {
    Name,
    CreationDate,
    LastUpdated,
}

#[function_component(OpenPage)]
pub fn open_page(props: &OpenPageProps) -> Html {
    let projects = use_state(|| Vec::<Project>::new());
    let error_message = use_state(|| Option::<String>::None);
    let search_query = use_state(String::new);
    let sort_key = use_state(|| OpenProjectSortKey::LastUpdated);
    let sort_desc = use_state(|| true);
    let rename_project_id = use_state(|| Option::<String>::None);
    let rename_value = use_state(String::new);
    let refresh_trigger = use_state(|| 0u32);

    let menu_listener_handle = use_mut_ref(|| None::<JsValue>);
    let menu_listener_closure = use_mut_ref(|| None::<Closure<dyn Fn(JsValue)>>);

    {
        let projects = projects.clone();
        let error_message = error_message.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                match open_projects_tauri_invoke("get_all_projects", JsValue::NULL).await {
                    Ok(result) => match serde_wasm_bindgen::from_value::<Vec<Project>>(result) {
                        Ok(mut fetched_projects) => {
                            fetched_projects.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
                            projects.set(fetched_projects);
                            error_message.set(None);
                        }
                        Err(_) => error_message.set(Some("Failed to fetch projects.".to_string())),
                    },
                    Err(err) => {
                        error_message.set(Some(format!(
                            "Failed to fetch projects: {}",
                            js_error_to_string(err)
                        )));
                    }
                }
            });

            || ()
        });
    }

    {
        let projects = projects.clone();
        let error_message = error_message.clone();
        let rename_project_id = rename_project_id.clone();
        let rename_value = rename_value.clone();
        let refresh_trigger = refresh_trigger.clone();
        let menu_listener_handle = menu_listener_handle.clone();
        let menu_listener_closure = menu_listener_closure.clone();

        use_effect_with((), move |_| {
            let projects = projects.clone();
            let error_message = error_message.clone();
            let rename_project_id = rename_project_id.clone();
            let rename_value = rename_value.clone();
            let refresh_trigger = refresh_trigger.clone();
            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure_storage = menu_listener_closure.clone();

            let on_menu_action = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let payload_key = JsValue::from_str("payload");
                if let Ok(payload_js) = js_sys::Reflect::get(&event, &payload_key) {
                    if let Ok(payload) = serde_wasm_bindgen::from_value::<
                        OpenProjectsContextMenuActionPayload,
                    >(payload_js)
                    {
                        match payload.action.as_str() {
                            "rename" => {
                                if let Some(project) = (*projects)
                                    .iter()
                                    .find(|project| project.id == payload.project_id)
                                {
                                    rename_value.set(project.project_name.clone());
                                    rename_project_id.set(Some(project.id.clone()));
                                }
                            }
                            "duplicate" => {
                                let project_id = payload.project_id.clone();
                                let error_message = error_message.clone();
                                let refresh_trigger = refresh_trigger.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = match serde_wasm_bindgen::to_value(&json!({
                                        "projectId": project_id
                                    })) {
                                        Ok(args) => args,
                                        Err(_) => {
                                            error_message.set(Some(
                                                "Failed to duplicate project.".to_string(),
                                            ));
                                            return;
                                        }
                                    };

                                    match open_projects_tauri_invoke("duplicate_project", args)
                                        .await
                                    {
                                        Ok(_) => {
                                            error_message.set(None);
                                            refresh_trigger.set(*refresh_trigger + 1);
                                        }
                                        Err(err) => {
                                            error_message.set(Some(format!(
                                                "Failed to duplicate project: {}",
                                                js_error_to_string(err)
                                            )));
                                        }
                                    }
                                });
                            }
                            "open-folder" => {
                                let project_id = payload.project_id.clone();
                                let error_message = error_message.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = match serde_wasm_bindgen::to_value(&json!({
                                        "projectId": project_id
                                    })) {
                                        Ok(args) => args,
                                        Err(_) => {
                                            error_message.set(Some(
                                                "Failed to open project folder.".to_string(),
                                            ));
                                            return;
                                        }
                                    };

                                    if let Err(err) =
                                        open_projects_tauri_invoke("open_project_folder", args)
                                            .await
                                    {
                                        error_message.set(Some(format!(
                                            "Failed to open project folder: {}",
                                            js_error_to_string(err)
                                        )));
                                    }
                                });
                            }
                            "delete" => {
                                let project_id = payload.project_id.clone();

                                if rename_project_id.as_deref() == Some(project_id.as_str()) {
                                    rename_project_id.set(None);
                                    rename_value.set(String::new());
                                }

                                let error_message = error_message.clone();
                                let refresh_trigger = refresh_trigger.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    let args = match serde_wasm_bindgen::to_value(&json!({
                                        "projectId": project_id
                                    })) {
                                        Ok(args) => args,
                                        Err(_) => {
                                            error_message
                                                .set(Some("Failed to delete project.".to_string()));
                                            return;
                                        }
                                    };

                                    match open_projects_tauri_invoke("delete_project", args).await {
                                        Ok(_) => {
                                            error_message.set(None);
                                            refresh_trigger.set(*refresh_trigger + 1);
                                        }
                                        Err(err) => {
                                            error_message.set(Some(format!(
                                                "Failed to delete project: {}",
                                                js_error_to_string(err)
                                            )));
                                        }
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                }
            });

            let js_callback = on_menu_action
                .as_ref()
                .unchecked_ref::<js_sys::Function>()
                .clone();
            *menu_listener_closure_storage.borrow_mut() = Some(on_menu_action);

            let handle_storage = menu_listener_handle.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(unlisten) =
                    open_projects_tauri_listen("open-projects-context-menu-action", &js_callback)
                        .await
                {
                    *handle_storage.borrow_mut() = Some(unlisten);
                }
            });

            let menu_listener_handle = menu_listener_handle.clone();
            let menu_listener_closure = menu_listener_closure.clone();
            move || {
                if let Some(unlisten) = menu_listener_handle.borrow_mut().take() {
                    wasm_bindgen_futures::spawn_local(async move {
                        let _ = open_projects_tauri_unlisten(unlisten).await;
                    });
                }
                menu_listener_closure.borrow_mut().take();
            }
        });
    }

    let on_cancel_rename = {
        let rename_project_id = rename_project_id.clone();
        let rename_value = rename_value.clone();
        Callback::from(move |_| {
            rename_project_id.set(None);
            rename_value.set(String::new());
        })
    };

    let commit_rename = {
        let rename_project_id = rename_project_id.clone();
        let rename_value = rename_value.clone();
        let error_message = error_message.clone();
        let refresh_trigger = refresh_trigger.clone();

        Callback::from(move |(project_id, next_name): (String, String)| {
            if rename_project_id.as_deref() != Some(project_id.as_str()) {
                return;
            }

            let trimmed_name = next_name.trim().to_string();
            rename_project_id.set(None);
            rename_value.set(String::new());

            if trimmed_name.is_empty() {
                error_message.set(Some("Project name cannot be empty.".to_string()));
                return;
            }

            let error_message = error_message.clone();
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let args = match serde_wasm_bindgen::to_value(&json!({
                    "projectId": project_id,
                    "projectName": trimmed_name
                })) {
                    Ok(args) => args,
                    Err(_) => {
                        error_message.set(Some("Failed to rename project.".to_string()));
                        return;
                    }
                };

                match open_projects_tauri_invoke("rename_project", args).await {
                    Ok(_) => {
                        error_message.set(None);
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                    Err(err) => {
                        error_message.set(Some(format!(
                            "Failed to rename project: {}",
                            js_error_to_string(err)
                        )));
                    }
                }
            });
        })
    };

    let on_search_input = {
        let search_query = search_query.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            search_query.set(input.value());
        })
    };

    let on_cycle_sort_key = {
        let sort_key = sort_key.clone();
        Callback::from(move |_| {
            let next = match *sort_key {
                OpenProjectSortKey::Name => OpenProjectSortKey::CreationDate,
                OpenProjectSortKey::CreationDate => OpenProjectSortKey::LastUpdated,
                OpenProjectSortKey::LastUpdated => OpenProjectSortKey::Name,
            };
            sort_key.set(next);
        })
    };

    let on_toggle_sort_direction = {
        let sort_desc = sort_desc.clone();
        Callback::from(move |_| {
            sort_desc.set(!*sort_desc);
        })
    };

    let search_normalized = search_query.trim().to_lowercase();
    let mut visible_projects: Vec<Project> = if search_normalized.is_empty() {
        (*projects).clone()
    } else {
        projects
            .iter()
            .filter(|project| {
                project
                    .project_name
                    .to_lowercase()
                    .contains(search_normalized.as_str())
            })
            .cloned()
            .collect()
    };
    visible_projects.sort_by(|a, b| match *sort_key {
        OpenProjectSortKey::Name => a
            .project_name
            .to_lowercase()
            .cmp(&b.project_name.to_lowercase()),
        OpenProjectSortKey::CreationDate => a.creation_date.cmp(&b.creation_date),
        OpenProjectSortKey::LastUpdated => a.last_modified.cmp(&b.last_modified),
    });
    if *sort_desc {
        visible_projects.reverse();
    }
    let sort_key_label = match *sort_key {
        OpenProjectSortKey::Name => "a-z",
        OpenProjectSortKey::CreationDate => "creation date",
        OpenProjectSortKey::LastUpdated => "last updated",
    };

    html! {
        <div id="open-page" class="container open-page">
            <div id="open-layout" class={classes!("open-layout", props.explorer_on_left.then_some("open-layout--explorer-left"))}>
                <div id="open-projects-sidebar" class="explorer-sidebar open-projects-sidebar">
                    <div id="open-projects-scroll" class="explorer-sidebar__scroll-area open-projects-scroll">
                        <div id="open-projects-section" class="tree-section open-projects-section">
                            <div id="open-projects-header" class="tree-section-header open-projects-header">
                                <span class="tree-section-header__title open-projects-header__title">{"Open"}</span>
                                <div id="open-projects-header-actions" class="tree-section-header__actions open-projects-header__actions">
                                    <button id="open-projects-sort-key-btn" type="button" class="open-projects-sort-key-btn" onclick={on_cycle_sort_key} title="Change sort field">{sort_key_label}</button>
                                    <button id="open-projects-sort-direction-btn" type="button" class="open-projects-sort-direction-btn" onclick={on_toggle_sort_direction} title={if *sort_desc {"Descending"} else {"Ascending"}} aria-label="Toggle sort direction">
                                        <span class={classes!("open-projects-sort-direction-icon", (!*sort_desc).then_some("open-projects-sort-direction-icon--asc"))}>
                                            <Icon icon_id={IconId::LucideChevronRight} width={"14"} height={"14"} />
                                        </span>
                                    </button>
                                </div>
                            </div>
                            <div id="open-projects-content" class="tree-section__content open-projects-content">
                                if let Some(error) = &*error_message {
                                    <div id="open-projects-error" class="tree-section__empty open-projects-error">{error}</div>
                                } else if visible_projects.is_empty() {
                                    <div id="open-projects-empty" class="tree-section__empty">
                                        {
                                            if search_normalized.is_empty() {
                                                "No projects found."
                                            } else {
                                                "No projects match your search."
                                            }
                                        }
                                    </div>
                                } else {
                                    <div id="open-projects-list" class="open-projects-list">
                                        {
                                            visible_projects.iter().map(|project| {
                                                let project_id = project.id.clone();
                                                let project_name = project.project_name.clone();
                                                let is_renaming = rename_project_id.as_deref() == Some(project.id.as_str());

                                                if is_renaming {
                                                    let project_id_for_commit = project_id.clone();
                                                    let on_rename_input = {
                                                        let rename_value = rename_value.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                            rename_value.set(input.value());
                                                        })
                                                    };
                                                    let on_rename_keydown = {
                                                        let commit_rename = commit_rename.clone();
                                                        let on_cancel_rename = on_cancel_rename.clone();
                                                        let project_id = project_id_for_commit.clone();
                                                        Callback::from(move |e: KeyboardEvent| {
                                                            if e.key() == "Enter" {
                                                                e.prevent_default();
                                                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                                commit_rename.emit((project_id.clone(), input.value()));
                                                            } else if e.key() == "Escape" {
                                                                e.prevent_default();
                                                                on_cancel_rename.emit(());
                                                            }
                                                        })
                                                    };
                                                    let on_rename_blur = {
                                                        let commit_rename = commit_rename.clone();
                                                        let project_id = project_id_for_commit.clone();
                                                        Callback::from(move |e: FocusEvent| {
                                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                            commit_rename.emit((project_id.clone(), input.value()));
                                                        })
                                                    };

                                                    html! {
                                                        <div id={format!("open-project-item-{}", project.id)} class="tree-node open-project-item open-project-item--renaming">
                                                            <span class="tree-node__chevron-spacer"></span>
                                                            <span class="tree-node__icon">
                                                                <Icon icon_id={IconId::LucideFolder} width={"16"} height={"16"} />
                                                            </span>
                                                            <input id="open-project-rename-input" class="tree-node__rename-input open-project-rename-input" type="text" value={(*rename_value).clone()} oninput={on_rename_input} onkeydown={on_rename_keydown} onblur={on_rename_blur} onclick={Callback::from(|e: MouseEvent| e.stop_propagation())} autofocus=true />
                                                        </div>
                                                    }
                                                } else {
                                                    let on_open_project = props.on_open_project.clone();
                                                    let project_id_for_open = project_id.clone();
                                                    let on_click = Callback::from(move |_| {
                                                        on_open_project.emit(project_id_for_open.clone());
                                                    });

                                                    let project_id_for_menu = project_id.clone();
                                                    let on_context_menu = Callback::from(move |e: MouseEvent| {
                                                        e.prevent_default();
                                                        e.stop_propagation();

                                                        let request = ShowOpenProjectsContextMenuRequest {
                                                            project_id: project_id_for_menu.clone(),
                                                            x: e.client_x() as f64,
                                                            y: e.client_y() as f64,
                                                        };

                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            if let Ok(args) = serde_wasm_bindgen::to_value(&json!({
                                                                "request": request
                                                            })) {
                                                                let _ = open_projects_tauri_invoke(
                                                                    "show_open_projects_context_menu",
                                                                    args,
                                                                )
                                                                .await;
                                                            }
                                                        });
                                                    });

                                                    html! {
                                                        <button id={format!("open-project-item-{}", project.id)} type="button" class="tree-node open-project-item" title={project_name.clone()} onclick={on_click} oncontextmenu={on_context_menu}>
                                                            <span class="tree-node__chevron-spacer"></span>
                                                            <span class="tree-node__icon">
                                                                <Icon icon_id={IconId::LucideFolder} width={"16"} height={"16"} />
                                                            </span>
                                                            <span class="tree-node__label">{project_name}</span>
                                                        </button>
                                                    }
                                                }
                                            }).collect::<Html>()
                                        }
                                    </div>
                                }
                            </div>
                        </div>

                        <div id="open-projects-search-container" class="open-projects-search-container">
                            <input id="open-projects-search-input" class="open-projects-search-input" type="search" placeholder="Search projects" value={(*search_query).clone()} oninput={on_search_input} autocomplete="off" spellcheck="false" />
                        </div>
                    </div>
                </div>
                <div id="open-main-content" class="open-main-content"></div>
            </div>
        </div>
    }
}
