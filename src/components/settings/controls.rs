use yew::prelude::*;
use yew_icons::{Icon, IconId};
use wasm_bindgen::JsCast;
use crate::pages::project::{SourceContent, FrameDirectory};

#[derive(Properties, PartialEq)]
pub struct ControlsProps {
    pub selected_source: Option<SourceContent>,
    pub selected_frame_dir: Option<FrameDirectory>,
    pub controls_collapsed: bool,
    pub on_toggle_collapsed: Callback<()>,
    pub is_playing: bool,
    pub on_is_playing_change: Callback<bool>,
    pub should_reset: bool,
    pub on_should_reset_change: Callback<bool>,
    pub synced_progress: f64,
    pub on_synced_progress_change: Callback<f64>,
    pub seek_percentage: Option<f64>,
    pub on_seek_percentage_change: Callback<Option<f64>>,
    pub frames_loading: bool,
}

#[function_component(Controls)]
pub fn controls(props: &ControlsProps) -> Html {
    let on_toggle = {
        let on_toggle_collapsed = props.on_toggle_collapsed.clone();
        Callback::from(move |_| {
            on_toggle_collapsed.emit(());
        })
    };

    let on_play_pause = {
        let on_is_playing_change = props.on_is_playing_change.clone();
        let is_playing = props.is_playing;
        Callback::from(move |_| {
            on_is_playing_change.emit(!is_playing);
        })
    };

    let on_reset = {
        let on_should_reset_change = props.on_should_reset_change.clone();
        Callback::from(move |_| {
            on_should_reset_change.emit(true);
            // Reset immediately, then set back to false
            let on_should_reset_change_clone = on_should_reset_change.clone();
            gloo_timers::callback::Timeout::new(100, move || {
                on_should_reset_change_clone.emit(false);
            }).forget();
        })
    };

    let on_progress_input = {
        let on_synced_progress_change = props.on_synced_progress_change.clone();
        let on_seek_percentage_change = props.on_seek_percentage_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let percentage = input.value_as_number();
                    on_synced_progress_change.emit(percentage);
                    on_seek_percentage_change.emit(Some(percentage / 100.0));
                }
            }
        })
    };


    html! {
        <div class="controls-column">
            <h2 class="collapsible-header" onclick={on_toggle}>
                <span class="chevron-icon">
                    {if props.controls_collapsed {
                        html! {<span>{"▶"}</span>}
                    } else {
                        html! {<span>{"▼"}</span>}
                    }}
                </span>
                <span>{"CONTROLS"}</span>
            </h2>
            {
                if !props.controls_collapsed {
                    html! {
                        <>
                            <div class="controls-buttons">
                                <button class="ctrl-btn" disabled={props.selected_source.is_none() || props.selected_frame_dir.is_none() || props.frames_loading} onclick={on_play_pause} title={if props.is_playing {"Pause"} else if props.frames_loading {"Loading frames..."} else {"Play"}}>
                                    <Icon icon_id={if props.is_playing {IconId::LucidePause} else {IconId::LucidePlay}} width={"20"} height={"20"} />
                                </button>
                                <button class="ctrl-btn" disabled={props.selected_source.is_none() && props.selected_frame_dir.is_none() || props.frames_loading} onclick={on_reset} title="Reset to beginning">
                                    <span class="reset-icon">{"↺"}</span>
                                </button>
                            </div>


                            <div class="control-row">
                                <input class="progress synced-progress" type="range" min="0" max="100" value={props.synced_progress.to_string()} oninput={on_progress_input} title="Synced progress control" disabled={props.selected_source.is_none() || props.selected_frame_dir.is_none()} />
                            </div>
                        </>
                    }
                } else {
                    html! {<></>}
                }
            }
        </div>
    }
}
