use yew::prelude::*;
use web_sys::HtmlVideoElement;
use yew_icons::{Icon, IconId};
use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiRequest {
    file_path: String,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: Option<u32>,
    project_id: String,
    source_file_id: String,
    custom_name: Option<String>,
    color: bool,
    extract_audio: bool,
}

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiInvokeArgs {
    request: ConvertToAsciiRequest,
}

#[wasm_bindgen(inline_js = r#"
export async function tauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args);
  throw new Error('Tauri invoke is not available');
}

export async function tauriListen(event, callback) {
  const g = globalThis.__TAURI__;
  if (g?.event?.listen) return g.event.listen(event, callback);
  throw new Error('Tauri listen is not available');
}

export async function tauriUnlisten(unlistenFn) {
  if (unlistenFn) await unlistenFn();
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_name = tauriListen)]
    async fn tauri_listen(event: &str, callback: &js_sys::Function) -> JsValue;

    #[wasm_bindgen(js_name = tauriUnlisten)]
    async fn tauri_unlisten(unlisten_fn: JsValue);
}

#[derive(Properties, PartialEq, Clone)]
pub struct VideoPlayerProps {
    /// A convertFileSrc-safe URL (local file via Tauri).
    pub src: String,
    #[prop_or_default]
    pub class: Classes,

    /// External control: when Some(true), play; when Some(false), pause; when None, no external control
    #[prop_or_default]
    pub should_play: Option<bool>,
    /// External control: when true, reset to beginning
    #[prop_or(false)]
    pub should_reset: bool,
    /// External control: seek to percentage (0.0-1.0) — interpreted RELATIVE TO TRIM WINDOW
    #[prop_or_default]
    pub seek_percentage: Option<f64>,
    /// Callback to report current progress (0.0-1.0) — emitted RELATIVE TO TRIM WINDOW
    #[prop_or_default]
    pub on_progress: Option<Callback<f64>>,

    // ---- Inline "Convert to ASCII" controls (rendered under trim bar) ----
    #[prop_or_default]
    pub project_id: Option<String>,
    #[prop_or_default]
    pub source_file_id: Option<String>,
    /// IMPORTANT: this is the ORIGINAL file path (not asset://)
    #[prop_or_default]
    pub source_file_path: Option<String>,

    #[prop_or(1)]
    pub luminance: u8,
    #[prop_or(0.7)]
    pub font_ratio: f32,
    #[prop_or(200)]
    pub columns: u32,
    #[prop_or(30)]
    pub fps: u32,

    #[prop_or_default]
    pub on_luminance_change: Option<Callback<u8>>,
    #[prop_or_default]
    pub on_font_ratio_change: Option<Callback<f32>>,
    #[prop_or_default]
    pub on_columns_change: Option<Callback<u32>>,
    #[prop_or_default]
    pub on_fps_change: Option<Callback<u32>>,

    #[prop_or_default]
    pub is_converting: Option<bool>,
    #[prop_or_default]
    pub on_conversion_start: Option<Callback<(String, String)>>, // (source_id, name)
    #[prop_or_default]
    pub on_conversion_complete: Option<Callback<String>>, // source_id

    #[prop_or_default]
    pub conversion_message: Option<String>,
    #[prop_or_default]
    pub on_conversion_message_change: Option<Callback<Option<String>>>,
    #[prop_or_default]
    pub on_error_message_change: Option<Callback<Option<String>>>,

    #[prop_or_default]
    pub on_refresh_frames: Option<Callback<()>>,

    /// Custom name to use for the conversion (e.g., from a cut's custom_name)
    #[prop_or_default]
    pub custom_name: Option<String>,

    // ---- Video cutting controls ----
    /// Callback to cut video: emits (start_time, end_time) in seconds
    #[prop_or_default]
    pub on_cut_video: Option<Callback<(f64, f64)>>,
    #[prop_or_default]
    pub is_cutting: Option<bool>,
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

    // Dual range selector state (0..1)
    let left_value = use_state(|| 0.0f64);
    let right_value = use_state(|| 1.0f64);

    // Color generation toggle state
    let generate_colors = use_state(|| true);

    // Audio extraction toggle state
    let extract_audio = use_state(|| false);

    // --- Derived trim window (seconds) for rendering + inputs ---
    let dur = *duration;
    let trim_start = if dur > 0.0 { (*left_value) * dur } else { 0.0 };
    let trim_end = if dur > 0.0 { (*right_value) * dur } else { 0.0 };
    let trim_len = (trim_end - trim_start).max(0.000_1);
    let progress_in_trim = if dur > 0.0 {
        ((*current_time - trim_start) / trim_len).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Toggle play/pause (trim-aware)
    let on_toggle = {
        let video_ref = video_ref.clone();
        let is_playing = is_playing.clone();
        let current_time = current_time.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();

        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                if v.paused() {
                    let dur = v.duration();
                    if dur.is_finite() && dur > 0.0 {
                        let trim_start = (*left_value) * dur;
                        let trim_end = (*right_value) * dur;
                        let t = v.current_time();

                        // If outside trim window, jump to trim start
                        if t < trim_start || t >= trim_end {
                            v.set_current_time(trim_start);
                            current_time.set(trim_start);
                        }
                    }

                    let _ = v.play();
                    is_playing.set(true);
                } else {
                    v.pause().ok();
                    is_playing.set(false);
                }
            }
        })
    };

    // Time update (clamp to trim end + emit trim-relative progress)
    let on_time_update = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let is_playing = is_playing.clone();
        let on_progress = props.on_progress.clone();

        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let dur = *duration;
                if dur <= 0.0 {
                    return;
                }

                let trim_start = (*left_value) * dur;
                let trim_end = (*right_value) * dur;
                let trim_len = (trim_end - trim_start).max(0.000_1);

                let mut t = v.current_time();

                // Stop at trim end
                if t >= trim_end {
                    t = trim_end;
                    v.set_current_time(t);
                    v.pause().ok();
                    is_playing.set(false);
                }

                current_time.set(t);

                // Emit progress RELATIVE TO TRIM WINDOW
                if let Some(callback) = &on_progress {
                    let progress = ((t - trim_start) / trim_len).clamp(0.0, 1.0);
                    callback.emit(progress);
                }
            }
        })
    };

    // Metadata (duration) - also seek to a visible frame (trim-aware)
    let on_loaded_metadata = {
        let video_ref = video_ref.clone();
        let duration = duration.clone();
        let current_time = current_time.clone();
        let left_value = left_value.clone();

        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let dur = v.duration();
                duration.set(dur);

                // Show something other than black:
                // - if trim starts > 0 -> go there
                // - else -> 0.1s
                if v.current_time() == 0.0 {
                    let t = if dur.is_finite() && dur > 0.0 && *left_value > 0.0 {
                        (*left_value) * dur
                    } else {
                        0.1
                    };
                    v.set_current_time(t);
                    current_time.set(t);
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
            error_text.set(Some(
                "Cannot play this video in the system webview (try MP4/H.264 or WebM).".into(),
            ));
        })
    };

    // Seek (clamped to trim window)
    let on_seek_input_trim = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
    
        Callback::from(move |e: InputEvent| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let dur = *duration;
                if dur <= 0.0 { return; }
    
                let trim_start = (*left_value) * dur;
                let trim_end = (*right_value) * dur;
                let trim_len = (trim_end - trim_start).max(0.000_1);
    
                // slider gives 0..1 within trim
                let p = e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value_as_number()
                    .clamp(0.0, 1.0);
    
                let t = trim_start + p * trim_len;
                v.set_current_time(t);
                current_time.set(t);
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
                let val = e
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value_as_number();
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
    let play_icon = if *is_playing {
        IconId::LucidePause
    } else {
        IconId::LucidePlay
    };
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

    // External play/pause control (trim-aware)
    {
        let video_ref = video_ref.clone();
        let is_playing = is_playing.clone();
        let current_time = current_time.clone();
        let should_play = props.should_play;
        let prev_should_play = use_mut_ref(|| None::<bool>);
        let left_value = left_value.clone();
        let right_value = right_value.clone();

        use_effect_with(should_play, move |should_play| {
            let current = *should_play;
            let prev = *prev_should_play.borrow();

            if current != prev {
                if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                    let dur = v.duration();
                    let (trim_start, trim_end) = if dur.is_finite() && dur > 0.0 {
                        ((*left_value) * dur, (*right_value) * dur)
                    } else {
                        (0.0, f64::INFINITY)
                    };

                    match current {
                        Some(true) => {
                            // Ensure we're in trim window
                            let t = v.current_time();
                            if t < trim_start || t >= trim_end {
                                v.set_current_time(trim_start);
                                current_time.set(trim_start);
                            }
                            let _ = v.play();
                            is_playing.set(true);
                        }
                        Some(false) => {
                            v.pause().ok();
                            is_playing.set(false);
                        }
                        None => {}
                    }
                }

                *prev_should_play.borrow_mut() = current;
            }

            || ()
        });
    }

    // Handle reset (go to trim start)
    {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let should_reset = props.should_reset;
        let left_value = left_value.clone();

        use_effect_with(should_reset, move |should_reset| {
            if *should_reset {
                if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                    let dur = v.duration();
                    if dur.is_finite() && dur > 0.0 {
                        let trim_start = (*left_value) * dur;
                        v.set_current_time(trim_start);
                        current_time.set(trim_start);
                    } else {
                        v.set_current_time(0.0);
                        current_time.set(0.0);
                    }
                }
            }
        });
    }

    // Handle seek percentage (0..1 relative to trim window)
    {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let seek_percentage = props.seek_percentage;
        let left_value = left_value.clone();
        let right_value = right_value.clone();

        use_effect_with(seek_percentage, move |seek_percentage| {
            if let Some(percentage) = seek_percentage {
                if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                    let dur = v.duration();
                    if dur.is_finite() && dur > 0.0 {
                        let trim_start = (*left_value) * dur;
                        let trim_end = (*right_value) * dur;
                        let trim_len = (trim_end - trim_start).max(0.000_1);

                        let seek_time =
                            trim_start + (percentage.clamp(0.0, 1.0) * trim_len);

                        v.set_current_time(seek_time);
                        current_time.set(seek_time);
                    }
                }
            }
        });
    }

    // If trim window changes, clamp current time and stop if needed
    // If trim window changes, clamp current time and stop if needed
    {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let is_playing = is_playing.clone();
        let duration = duration.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();

        use_effect_with(((*left_value), (*right_value), (*duration)), move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let dur = *duration;

                // Just do nothing if duration isn't ready
                if dur > 0.0 {
                    let trim_start = (*left_value) * dur;
                    let trim_end = (*right_value) * dur;

                    let t = v.current_time();
                    if t < trim_start || t > trim_end {
                        let new_t = t.clamp(trim_start, trim_end);
                        v.set_current_time(new_t);
                        current_time.set(new_t);

                        if new_t >= trim_end {
                            v.pause().ok();
                            is_playing.set(false);
                        }
                    }
                }
            }

            // Always return exactly one cleanup closure type
            || ()
        });
    }

    // Dual range selector handlers
    let min_gap = 0.01;

    let on_left_range_input = {
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        Callback::from(move |e: InputEvent| {
            let val = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value_as_number()
                .clamp(0.0, 1.0);

            left_value.set(val.min(*right_value - min_gap));
        })
    };

    let on_right_range_input = {
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        Callback::from(move |e: InputEvent| {
            let val = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value_as_number()
                .clamp(0.0, 1.0);

            right_value.set(val.max(*left_value + min_gap));
        })
    };

    let is_converting = props.is_converting.unwrap_or(false);
    let is_cutting = props.is_cutting.unwrap_or(false);

    let on_cut_click = {
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let duration = duration.clone();
        let on_cut_video = props.on_cut_video.clone();

        Callback::from(move |_| {
            if let Some(on_cut) = &on_cut_video {
                let dur = *duration;
                if dur > 0.0 {
                    let start_time = (*left_value) * dur;
                    let end_time = (*right_value) * dur;
                    on_cut.emit((start_time, end_time));
                }
            }
        })
    };

    let on_luminance_input = {
        let cb = props.on_luminance_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(cb) = &cb {
                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                if let Ok(val) = input.value().parse::<u8>() {
                    cb.emit(val);
                }
            }
        })
    };

    let on_font_ratio_input = {
        let cb = props.on_font_ratio_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(cb) = &cb {
                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                if let Ok(val) = input.value().parse::<f32>() {
                    cb.emit(val);
                }
            }
        })
    };

    let on_columns_input = {
        let cb = props.on_columns_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(cb) = &cb {
                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                if let Ok(val) = input.value().parse::<u32>() {
                    cb.emit(val);
                }
            }
        })
    };

    let on_fps_input = {
        let cb = props.on_fps_change.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(cb) = &cb {
                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                if let Ok(val) = input.value().parse::<u32>() {
                    cb.emit(val);
                }
            }
        })
    };

    let on_convert_click = {
        let project_id = props.project_id.clone();
        let source_file_id = props.source_file_id.clone();
        let source_file_path = props.source_file_path.clone();
        let custom_name = props.custom_name.clone();

        let luminance = props.luminance;
        let font_ratio = props.font_ratio;
        let columns = props.columns;
        let fps = props.fps;
        let generate_colors = generate_colors.clone();
        let extract_audio = extract_audio.clone();

        let on_conversion_start = props.on_conversion_start.clone();
        let on_conversion_complete = props.on_conversion_complete.clone();
        let on_error_message_change = props.on_error_message_change.clone();

        Callback::from(move |_| {
            let color = *generate_colors;
            let extract_audio = *extract_audio;
            let (Some(project_id), Some(source_file_id), Some(file_path)) =
                (project_id.clone(), source_file_id.clone(), source_file_path.clone())
            else {
                return;
            };
            let custom_name = custom_name.clone();

            // Get display name for progress display
            let display_name = custom_name.clone().unwrap_or_else(|| {
                std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string()
            });

            // Notify that conversion is starting
            if let Some(cb) = &on_conversion_start {
                cb.emit((source_file_id.clone(), display_name));
            }
            if let Some(cb) = &on_error_message_change {
                cb.emit(None);
            }

            let on_conversion_complete = on_conversion_complete.clone();
            let on_error_message_change = on_error_message_change.clone();
            let source_file_id_for_complete = source_file_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                // Start conversion (returns immediately, progress/completion handled by global listeners)
                web_sys::console::log_1(&format!("🚀 Starting tauri_invoke for: {}", source_file_id_for_complete).into());
                let invoke_args = ConvertToAsciiInvokeArgs {request: ConvertToAsciiRequest {file_path, luminance, font_ratio, columns, fps: Some(fps), project_id, source_file_id, custom_name, color, extract_audio}};
                let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                let result = tauri_invoke("convert_to_ascii", args).await;

                // Backend returns immediately with "Conversion started for: {name}"
                // Actual completion is handled via conversion-complete event in project.rs
                match serde_wasm_bindgen::from_value::<String>(result.clone()) {
                    Ok(msg) => {
                        web_sys::console::log_1(&format!("✅ Conversion initiated: {}", msg).into());
                    }
                    Err(e) => {
                        // Error starting conversion
                        web_sys::console::log_1(&format!("❌ Failed to start conversion: {:?}, raw result: {:?}", e, result).into());
                        // Remove from active conversions since it failed to start
                        if let Some(cb) = &on_conversion_complete {
                            cb.emit(source_file_id_for_complete.clone());
                        }
                        if let Some(cb) = &on_error_message_change {
                            cb.emit(Some("Failed to start conversion. Please try again.".to_string()));
                        }
                    }
                }
            });
        })
    };

    html! {
        <div class={classes!("video-player", props.class.clone())}>
            <div class="video-wrap">
                <video ref={video_ref.clone()} class="video" src={props.src.clone()} preload="metadata" playsinline=true ontimeupdate={on_time_update} onloadedmetadata={on_loaded_metadata} onplay={on_play} onpause={on_pause} onerror={on_error} onclick={on_toggle.clone()} />
                if let Some(msg) = &*error_text {
                    <div class="error-overlay">{msg}</div>
                }
                <div class="timestamp-overlay">{timestamp}</div>
            </div>

            <div class="controls" id="video-controls">
                <div class="control-row" id="video-progress">
                    <input class="progress" type="range" min="0" max="1" step="0.0001" value={progress_in_trim.to_string()} oninput={on_seek_input_trim.clone()} title="Seek (within trim)" />
                    <button class="ctrl-btn" type="button" onclick={on_toggle.clone()} title="Play/Pause">
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="video-volume">
                    <input class="volume-bar" type="range" min="0" max="1" step="0.01" value={volume.to_string()} oninput={on_volume_input.clone()} title="Volume" />
                    <button class="ctrl-btn" type="button" onclick={on_toggle_mute.clone()} title="Mute/Unmute">
                        <Icon icon_id={vol_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="video-cut-controls">
                    <div class="range-selector">
                        <div class="range-selector-track"></div>
                        <input class="range-selector-input range-left" type="range" min="0" max="1" step="0.001" value={left_value.to_string()} oninput={on_left_range_input.clone()} title="Range start" />
                        <input class="range-selector-input range-right" type="range" min="0" max="1" step="0.001" value={right_value.to_string()} oninput={on_right_range_input.clone()} title="Range end" />
                    </div>
                    <button class="ctrl-btn" type="button" onclick={on_cut_click.clone()} disabled={is_cutting || props.on_cut_video.is_none()} title="Cut video segment">
                        <Icon icon_id={IconId::LucideScissors} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="controls-divider"></div>

                <div class="control-row" id="conversion-settings">
                    <div class="settings-info">
                        <div class="settings-row">
                            <span class="settings-label">{"FPS:"}</span>
                            <input type="number" class="setting-input" value={props.fps.to_string()} min="1" max="120" oninput={on_fps_input.clone()} />
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"FONT RATIO:"}</span>
                            <input type="number" class="setting-input" value={props.font_ratio.to_string()} min="0.1" max="2.0" step="0.1" oninput={on_font_ratio_input.clone()} />
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"LUMINANCE:"}</span>
                            <input type="number" class="setting-input" value={props.luminance.to_string()} min="0" max="255" oninput={on_luminance_input.clone()} />
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"COLUMNS:"}</span>
                            <input type="number" class="setting-input" value={props.columns.to_string()} min="1" max="2000" oninput={on_columns_input.clone()} />
                        </div>
                    </div>
                    <button class={classes!("ctrl-btn", "color-toggle-btn", (*generate_colors).then_some("active"))} type="button" onclick={{
                        let generate_colors = generate_colors.clone();
                        Callback::from(move |_| generate_colors.set(!*generate_colors))
                    }} title={if *generate_colors { "Color generation enabled" } else { "Color generation disabled" }}>
                        if *generate_colors {
                            <Icon icon_id={IconId::LucideBrush} width={"20"} height={"20"} />
                        } else {
                            <Icon icon_id={IconId::LucideXCircle} width={"20"} height={"20"} />
                        }
                    </button>
                    <button class={classes!("ctrl-btn", "audio-toggle-btn", (*extract_audio).then_some("active"))} type="button" onclick={{
                        let extract_audio = extract_audio.clone();
                        Callback::from(move |_| extract_audio.set(!*extract_audio))
                    }} title={if *extract_audio { "Audio extraction enabled" } else { "Audio extraction disabled" }}>
                        <Icon icon_id={IconId::LucideVolume2} width={"20"} height={"20"} />
                    </button>
                    <button class="ctrl-btn" type="button" onclick={on_convert_click.clone()} disabled={is_converting || props.project_id.is_none() || props.source_file_id.is_none() || props.source_file_path.is_none()} title="Convert to ASCII">
                        <Icon icon_id={IconId::LucideWand} width={"20"} height={"20"} />
                    </button>
                </div>

            </div>
        </div>
    }
}
