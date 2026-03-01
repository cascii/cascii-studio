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
    pub loop_enabled: bool,
    pub on_loop_change: Callback<bool>,
    pub volume: f64,
    pub is_muted: bool,
    pub on_volume_change: Callback<f64>,
    pub on_is_muted_change: Callback<bool>,
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
        let on_is_playing_change = props.on_is_playing_change.clone();
        Callback::from(move |_| {
            // Reset should always leave transport paused so next click is a single "Play".
            on_is_playing_change.emit(false);
            on_should_reset_change.emit(true);
            // Reset immediately, then set back to false
            let on_should_reset_change_clone = on_should_reset_change.clone();
            gloo_timers::callback::Timeout::new(0, move || {
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

    let on_toggle_loop = {
        let on_loop_change = props.on_loop_change.clone();
        let loop_enabled = props.loop_enabled;
        Callback::from(move |_| {
            on_loop_change.emit(!loop_enabled);
        })
    };

    let on_volume_input = {
        let on_volume_change = props.on_volume_change.clone();
        let on_is_muted_change = props.on_is_muted_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let value = input.value_as_number();
                    if value.is_finite() {
                        let clamped = value.clamp(0.0, 1.0);
                        on_volume_change.emit(clamped);
                        if clamped > 0.0 {
                            on_is_muted_change.emit(false);
                        }
                    }
                }
            }
        })
    };

    let on_toggle_mute = {
        let on_is_muted_change = props.on_is_muted_change.clone();
        let is_muted = props.is_muted;
        Callback::from(move |_| {
            on_is_muted_change.emit(!is_muted);
        })
    };

    let volume_icon = if props.is_muted || props.volume == 0.0 {
        IconId::LucideVolumeX
    } else if props.volume < 0.5 {
        IconId::LucideVolume1
    } else {
        IconId::LucideVolume2
    };

    html! {
        <div class="controls-section">
            <div class="tree-section-header" onclick={on_toggle}>
                <span class={classes!("tree-section-header__chevron", props.controls_collapsed.then_some("tree-section-header__chevron--collapsed"))}>
                    <Icon icon_id={IconId::LucideChevronRight} width={"16"} height={"16"} />
                </span>
                <span class="tree-section-header__title">{"CONTROLS"}</span>
            </div>
            if !props.controls_collapsed {
                <div class="controls-section__content">
                    <div class="controls-section__buttons">
                        <button class="ctrl-btn" disabled={props.selected_source.is_none() || props.selected_frame_dir.is_none() || props.frames_loading} onclick={on_play_pause} title={if props.is_playing {"Pause"} else if props.frames_loading {"Loading frames..."} else {"Play"}}>
                            <Icon icon_id={if props.is_playing {IconId::LucidePause} else {IconId::LucidePlay}} width={"16"} height={"16"} />
                        </button>
                        <button class="ctrl-btn" disabled={props.selected_source.is_none() && props.selected_frame_dir.is_none() || props.frames_loading} onclick={on_reset} title="Reset to beginning">
                            <span class="reset-icon">{"↺"}</span>
                        </button>
                        <button class={classes!("ctrl-btn", "loop-btn", props.loop_enabled.then_some("active"))} onclick={on_toggle_loop} title={if props.loop_enabled {"Loop enabled"} else {"Loop disabled"}}>
                            <Icon icon_id={IconId::LucideRepeat} width={"14"} height={"14"} />
                        </button>
                        <button class="ctrl-btn" type="button" onclick={on_toggle_mute} title={if props.is_muted {"Unmute"} else {"Mute"}}>
                            <Icon icon_id={volume_icon} width={"16"} height={"16"} />
                        </button>
                    </div>
                    <div class="controls-section__slider">
                        <input class="volume-bar" type="range" min="0" max="1" step="0.01" value={props.volume.to_string()} oninput={on_volume_input} title="Volume" />
                    </div>
                    <div class="controls-section__slider">
                        <input class="synced-progress" type="range" min="0" max="100" value={props.synced_progress.to_string()} oninput={on_progress_input} title="Synced progress control" disabled={props.selected_source.is_none() || props.selected_frame_dir.is_none()} />
                    </div>
                </div>
            }
        </div>
    }
}
