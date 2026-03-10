use yew::prelude::*;
use yew_icons::{Icon, IconId};

#[derive(Clone, Debug, PartialEq)]
pub enum OperationKind {
    GeneratingFrames,
    LoadingTextFrames,
    LoadingColorFrames,
    Preprocessing,
    ImportingFile,
    CuttingVideo,
}

impl OperationKind {
    fn label(&self) -> &'static str {
        match self {
            Self::GeneratingFrames => "Generating",
            Self::LoadingTextFrames => "Loading text",
            Self::LoadingColorFrames => "Loading colors",
            Self::Preprocessing => "Preprocessing",
            Self::ImportingFile => "Importing",
            Self::CuttingVideo => "Cutting",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputingOperation {
    pub id: String,
    pub kind: OperationKind,
    pub label: String,
    pub progress: Option<f32>,
}

#[derive(Properties, PartialEq)]
pub struct ComputingProps {
    pub operations: Vec<ComputingOperation>,
    pub collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
}

#[function_component(Computing)]
pub fn computing(props: &ComputingProps) -> Html {
    if props.operations.is_empty() {
        return html! {};
    }

    let on_toggle = {
        let on_toggle_collapsed = props.on_toggle_collapsed.clone();
        Callback::from(move |_| on_toggle_collapsed.emit(()))
    };

    html! {
        <div id="computing-section" class="computing-section">
            <div id="computing-header" class="tree-section-header" onclick={on_toggle}>
                <span class={classes!("tree-section-header__chevron", props.collapsed.then_some("tree-section-header__chevron--collapsed"))}>
                    <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
                </span>
                <span class="tree-section-header__title">{"COMPUTING"}</span>
            </div>
            if !props.collapsed {
                <div id="computing-content" class="computing-section__content">
                    {props.operations.iter().map(|operation| {
                        let label = if operation.label.is_empty() {
                            operation.kind.label().to_string()
                        } else {
                            format!("{} {}", operation.kind.label(), operation.label)
                        };

                        html! {
                            <div class="computing-operation" key={operation.id.clone()}>
                                <div class="computing-operation__label" title={label.clone()}>{label}</div>
                                <div class="computing-operation__bar">
                                    if let Some(progress) = operation.progress {
                                        <div
                                            class="computing-operation__fill"
                                            style={format!("width: {:.2}%;", progress.clamp(0.0, 1.0) * 100.0)}
                                        />
                                    } else {
                                        <div class="computing-operation__fill computing-operation__fill--indeterminate" />
                                    }
                                </div>
                            </div>
                        }
                    }).collect::<Html>()}
                </div>
            }
        </div>
    }
}
