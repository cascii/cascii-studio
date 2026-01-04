use yew::prelude::*;
use crate::pages::project::SourceContent;

#[derive(Properties, PartialEq)]
pub struct SourceFilesProps {
    pub source_files: Vec<SourceContent>,
    pub selected_source: Option<SourceContent>,
    pub source_files_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub on_select_source: Callback<SourceContent>,
    pub on_add_files: Option<Callback<()>>,
}

#[function_component(SourceFiles)]
pub fn source_files(props: &SourceFilesProps) -> Html {
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
                            on_add_files.emit(());
                        })
                    };
                    html! {
                        <button type="button" class="add-files-btn" onclick={on_add} title="Add files">
                            {"+"}
                        </button>
                    }
                } else {
                    html! {}
                }}
            </h2>
            {
                if !props.source_files_collapsed {
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
    }
}
