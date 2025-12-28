use yew::prelude::*;
use web_sys::HtmlVideoElement;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq, Clone)]
pub struct VideoPlayerProps {
    /// A convertFileSrc-safe URL (local file via Tauri).
    pub src: String,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(VideoPlayer)]
pub fn video_player(props: &VideoPlayerProps) -> Html {
    let video_ref = use_node_ref();

    let is_playing = use_state(|| false);
    let is_muted = use_state(|| false);
    let duration = use_state(|| 0.0f64);
    let current_time = use_state(|| 0.0f64);
    let volume = use_state(|| 1.0f64);
    let error_text = use_state(|| None::<String>);

    // Toggle play/pause
    let on_toggle = {
        let video_ref = video_ref.clone();
        let is_playing = is_playing.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                if v.paused() {
                    let _ = v.play();
                    is_playing.set(true);
                } else {
                    v.pause().ok();
                    is_playing.set(false);
                }
            }
        })
    };

    // Time update
    let on_time_update = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                current_time.set(v.current_time());
            }
        })
    };

    // Metadata (duration) - also seek to first frame to show preview
    let on_loaded_metadata = {
        let video_ref = video_ref.clone();
        let duration = duration.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                duration.set(v.duration());
                // Seek to first frame (0.1s) to show preview instead of black screen
                if v.current_time() == 0.0 {
                    v.set_current_time(0.1);
                }
            }
        })
    };

    // Keep icon in sync
    let on_play = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| is_playing.set(true))
    };
    let on_pause = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| is_playing.set(false))
    };

    // Error overlay
    let on_error = {
        let error_text = error_text.clone();
        Callback::from(move |_| {
            error_text.set(Some("Cannot play this video in the system webview (try MP4/H.264 or WebM).".into()));
        })
    };

    // Seek
    let on_seek_input = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let val = e.target_unchecked_into::<web_sys::HtmlInputElement>().value_as_number();
                if val.is_finite() {
                    v.set_current_time(val);
                    current_time.set(val);
                }
            }
        })
    };

    // Volume slider
    let on_volume_input = {
        let video_ref = video_ref.clone();
        let volume_state = volume.clone();
        let is_muted = is_muted.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let val = e.target_unchecked_into::<web_sys::HtmlInputElement>().value_as_number();
                if val.is_finite() {
                    let clamped = val.clamp(0.0, 1.0);
                    v.set_volume(clamped);
                    volume_state.set(clamped);
                    if clamped > 0.0 && v.muted() {
                        v.set_muted(false);
                        is_muted.set(false);
                    }
                }
            }
        })
    };

    // Mute toggle
    let on_toggle_mute = {
        let video_ref = video_ref.clone();
        let is_muted = is_muted.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let new_state = !v.muted();
                v.set_muted(new_state);
                is_muted.set(new_state);
            }
        })
    };

    // Icon choices
    let play_icon = if *is_playing { IconId::LucidePause } else { IconId::LucidePlay };
    let vol_icon = if *is_muted || *volume == 0.0 {
        IconId::LucideVolumeX
    } else if *volume < 0.5 {
        IconId::LucideVolume1
    } else {
        IconId::LucideVolume2
    };

    // Format timestamp with milliseconds for accuracy
    let format_time = |seconds: f64| -> String {
        if seconds.is_finite() && seconds >= 0.0 {
            let total_secs = seconds.floor() as u32;
            let millis = ((seconds - seconds.floor()) * 100.0).floor() as u32;
            let mins = total_secs / 60;
            let secs = total_secs % 60;
            format!("{:02}:{:02}.{:02}", mins, secs, millis)
        } else {
            "00:00.00".to_string()
        }
    };

    let current_time_str = format_time(*current_time);
    let duration_str = format_time(*duration);
    let timestamp = format!("{} / {}", current_time_str, duration_str);

    html! {
        <div class={classes!("video-player", props.class.clone())}>
            <div class="video-wrap">
                <video
                    ref={video_ref.clone()}
                    class="video"
                    src={props.src.clone()}
                    preload="metadata"
                    playsinline=true
                    ontimeupdate={on_time_update}
                    onloadedmetadata={on_loaded_metadata}
                    onplay={on_play}
                    onpause={on_pause}
                    onerror={on_error}
                    onclick={on_toggle.clone()}
                />
                if let Some(msg) = &*error_text {
                    <div class="error-overlay">{ msg }</div>
                }
                <div class="timestamp-overlay">{ timestamp }</div>
            </div>

            <div class="controls">
                <div class="control-row">
                    <input
                        class="progress"
                        type="range"
                        min="0"
                        step="0.01"
                        max={duration.to_string()}
                        value={current_time.to_string()}
                        oninput={on_seek_input}
                        title="Seek"
                    />
                    <button class="ctrl-btn" type="button" onclick={on_toggle.clone()} title="Play/Pause">
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row">
                    <input
                        class="volume-bar"
                        type="range"
                        min="0"
                        max="1"
                        step="0.01"
                        value={volume.to_string()}
                        oninput={on_volume_input}
                        title="Volume"
                    />
                    <button class="ctrl-btn" type="button" onclick={on_toggle_mute} title="Mute/Unmute">
                        <Icon icon_id={vol_icon} width={"20"} height={"20"} />
                    </button>
                </div>
            </div>
        </div>
    }
}
