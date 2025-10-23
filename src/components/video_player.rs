use yew::prelude::*;
use web_sys::HtmlVideoElement;
use yew_icons::{Icon, IconId};

#[derive(Properties, PartialEq, Clone)]
pub struct VideoPlayerProps {
    /// A `convertFileSrc`-style URL (asset-friendly) pointing to the local file.
    pub src: String,
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(VideoPlayer)]
pub fn video_player(props: &VideoPlayerProps) -> Html {
    let video_ref = use_node_ref();

    let is_playing = use_state(|| false);
    let duration = use_state(|| 0.0f64);
    let current_time = use_state(|| 0.0f64);
    let volume = use_state(|| 1.0f64); // 0.0..1.0

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

    // Time update (progress)
    let on_time_update = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                current_time.set(v.current_time());
            }
        })
    };

    // Loaded metadata (duration)
    let on_loaded_metadata = {
        let video_ref = video_ref.clone();
        let duration = duration.clone();
        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                duration.set(v.duration());
            }
        })
    };

    // Play & pause events (keep icon in sync even if user clicks on video)
    let on_play = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| is_playing.set(true))
    };
    let on_pause = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| is_playing.set(false))
    };

    // Seek by range input
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

    // Volume change (slider)
    let on_volume_input = {
        let video_ref = video_ref.clone();
        let volume_state = volume.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let val = e.target_unchecked_into::<web_sys::HtmlInputElement>().value_as_number();
                if val.is_finite() {
                    let clamped = val.clamp(0.0, 1.0);
                    v.set_volume(clamped);
                    volume_state.set(clamped);
                }
            }
        })
    };

    // Choose an icon for current volume
    let vol_icon = if *volume == 0.0 {
        IconId::LucideVolumeX
    } else if *volume < 0.5 {
        IconId::LucideVolume1
    } else {
        IconId::LucideVolume2
    };

    html! {
        <div class={classes!("video-player", props.class.clone())}>
            <video
                ref={video_ref.clone()}
                class="video"
                src={props.src.clone()}
                ontimeupdate={on_time_update}
                onloadedmetadata={on_loaded_metadata}
                onplay={on_play}
                onpause={on_pause}
                // Allow clicking the video to toggle play/pause
                onclick={on_toggle.clone()}
            />
            <div class="controls">
                <button class="ctrl-btn" type="button" onclick={on_toggle.clone()} title="Play/Pause">
                    {
                        if *is_playing {
                            html! { <Icon icon_id={IconId::LucidePause} width={"20"} height={"20"} /> }
                        } else {
                            html! { <Icon icon_id={IconId::LucidePlay} width={"20"} height={"20"} /> }
                        }
                    }
                </button>

                <input
                    class="progress"
                    type="range"
                    min="0"
                    step="0.01"
                    max={duration.to_string()}
                    value={current_time.to_string()}
                    oninput={on_seek_input}
                />

                <div class="volume">
                    <Icon icon_id={vol_icon} width={"18"} height={"18"} />
                    <input
                        class="volume-bar"
                        type="range"
                        min="0"
                        max="1"
                        step="0.01"
                        value={volume.to_string()}
                        oninput={on_volume_input}
                    />
                </div>
            </div>
        </div>
    }
}
