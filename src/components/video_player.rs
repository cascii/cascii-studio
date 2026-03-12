use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use web_sys::HtmlVideoElement;
use yew::prelude::*;
use yew_icons::{Icon, IconId};

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
    preprocess_enabled: bool,
    preprocess_preset: Option<String>,
    preprocess_custom: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ConvertToAsciiInvokeArgs {
    request: ConvertToAsciiRequest,
}

const PREPROCESS_PRESET_OPTIONS: &[(&str, &str)] = &[
    ("contours", "Contours"),
    ("contours-soft", "Contours (Soft)"),
    ("contours-strong", "Contours (Strong)"),
    ("bw-contrast", "B/W Contrast"),
    ("noir-detail", "Noir Detail"),
    ("vivid", "Vivid"),
    ("warm-pop", "Warm Pop"),
    ("cool-pop", "Cool Pop"),
    ("soft-glow", "Soft Glow"),
    ("other", "Other"),
];

#[derive(Serialize, Deserialize)]
struct CreatePreviewRequest {
    video_path: String,
    timestamp: f64,
    luminance: u8,
    font_ratio: f32,
    columns: u32,
    fps: u32,
    color: bool,
    project_id: String,
    source_file_id: String,
}

#[derive(Serialize, Deserialize)]
struct CreatePreviewInvokeArgs {
    request: CreatePreviewRequest,
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
    /// Whether playback should loop (applies to trim window)
    #[prop_or(true)]
    pub loop_enabled: bool,
    /// Playback volume (0.0-1.0)
    #[prop_or(1.0)]
    pub volume: f64,
    /// Whether audio is muted
    #[prop_or(false)]
    pub is_muted: bool,
    /// External control: seek to percentage (0.0-1.0) — interpreted RELATIVE TO TRIM WINDOW
    #[prop_or_default]
    pub seek_percentage: Option<f64>,
    /// Callback to report current progress (0.0-1.0) — emitted RELATIVE TO TRIM WINDOW
    #[prop_or_default]
    pub on_progress: Option<Callback<f64>>,
    /// Callback emitted once when playback reaches the end (trim end) and loop is disabled
    #[prop_or_default]
    pub on_ended: Option<Callback<()>>,
    /// Callback emitted when the video has decoded enough data to display a frame
    #[prop_or_default]
    pub on_ready: Option<Callback<()>>,
    /// Whether the player should seek to a visible frame while paused after metadata loads
    #[prop_or(true)]
    pub preview_seek_enabled: bool,
    /// Callback to report the intrinsic media aspect ratio (width / height)
    #[prop_or_default]
    pub on_aspect_ratio_change: Option<Callback<Option<f64>>>,

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
    /// Callback to crop video: emits (top, bottom, left, right) in pixels
    #[prop_or_default]
    pub on_crop_video: Option<Callback<(u32, u32, u32, u32)>>,
    #[prop_or_default]
    pub is_cropping: Option<bool>,

    // ---- Video preprocessing controls ----
    /// Callback to preprocess video: emits (preset, custom_filter)
    #[prop_or_default]
    pub on_preprocess_video: Option<Callback<(String, Option<String>)>>,
    #[prop_or_default]
    pub is_preprocessing: Option<bool>,

    // ---- Default settings from settings.json ----
    #[prop_or(true)]
    pub color_frames_default: bool,
    #[prop_or(false)]
    pub extract_audio_default: bool,

    // ---- Preview creation ----
    /// Callback when a preview is created (emits the created Preview)
    #[prop_or_default]
    pub on_preview_created: Option<Callback<serde_json::Value>>,
}

#[function_component(VideoPlayer)]
pub fn video_player(props: &VideoPlayerProps) -> Html {
    let video_ref = use_node_ref();

    let is_playing = use_state(|| false);
    let duration = use_state(|| 0.0f64);
    let current_time = use_state(|| 0.0f64);
    let error_text = use_state(|| None::<String>);

    // Dual range selector state (0..1)
    let left_value = use_state(|| 0.0f64);
    let right_value = use_state(|| 1.0f64);

    // Color generation toggle state (initialized from settings)
    let color_default = props.color_frames_default;
    let generate_colors = use_state(move || color_default);

    // Audio extraction toggle state (initialized from settings)
    let audio_default = props.extract_audio_default;
    let extract_audio = use_state(move || audio_default);

    // Advanced settings visibility toggle (shows preprocessing controls)
    let show_advanced_settings = use_state(|| false);
    let show_crop_settings = use_state(|| false);

    // Preprocessing UI state (applied during video frame generation)
    let preprocess_enabled = use_state(|| false);
    let preprocess_preset = use_state(|| "contours".to_string());
    let preprocess_custom = use_state(String::new);

    // Video crop UI state (pixel-based)
    let crop_top = use_state(|| 0u32);
    let crop_bottom = use_state(|| 0u32);
    let crop_left = use_state(|| 0u32);
    let crop_right = use_state(|| 0u32);
    let crop_side = use_state(|| "top".to_string());

    // Preview creation state
    let is_creating_preview = use_state(|| false);

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

    {
        let on_aspect_ratio_change = props.on_aspect_ratio_change.clone();
        let src = props.src.clone();

        use_effect_with(src, move |_| {
            if let Some(callback) = &on_aspect_ratio_change {
                callback.emit(None);
            }
            || ()
        });
    }

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
    let ended_fired = use_mut_ref(|| false);
    let on_time_update = {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let duration = duration.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let is_playing = is_playing.clone();
        let loop_enabled = props.loop_enabled;
        let on_progress = props.on_progress.clone();
        let on_ended = props.on_ended.clone();
        let ended_fired = ended_fired.clone();

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

                // Stop or loop at trim end
                if t >= trim_end {
                    if loop_enabled {
                        t = trim_start;
                        v.set_current_time(t);
                        if v.paused() {
                            let _ = v.play();
                        }
                        is_playing.set(true);
                        *ended_fired.borrow_mut() = false;
                    } else {
                        t = trim_end;
                        v.set_current_time(t);
                        v.pause().ok();
                        is_playing.set(false);
                        if !*ended_fired.borrow() {
                            *ended_fired.borrow_mut() = true;
                            if let Some(cb) = &on_ended {
                                cb.emit(());
                            }
                        }
                    }
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

    // Metadata (duration)
    let on_loaded_metadata = {
        let video_ref = video_ref.clone();
        let duration = duration.clone();
        let current_time = current_time.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let should_play = props.should_play;
        let preview_seek_enabled = props.preview_seek_enabled;
        let on_aspect_ratio_change = props.on_aspect_ratio_change.clone();

        Callback::from(move |_| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                let dur = v.duration();
                duration.set(dur);
                let video_width = v.video_width();
                let video_height = v.video_height();
                if let Some(callback) = &on_aspect_ratio_change {
                    if video_width > 0 && video_height > 0 {
                        callback.emit(Some(video_width as f64 / video_height as f64));
                    }
                }

                if dur.is_finite() && dur > 0.0 {
                    let trim_start = (*left_value) * dur;
                    let trim_end = (*right_value) * dur;
                    if preview_seek_enabled && should_play != Some(true) && v.current_time() == 0.0
                    {
                        let max_target = (trim_end - 0.000_1).max(0.0);
                        let target = if trim_start > 0.0 {
                            trim_start.min(max_target)
                        } else {
                            0.001_f64.min(max_target)
                        };

                        if target > 0.0 {
                            v.set_current_time(target);
                            current_time.set(target);
                        }
                    } else if v.current_time() < trim_start {
                        v.set_current_time(trim_start);
                        current_time.set(trim_start);
                    } else if v.current_time() >= trim_end {
                        v.set_current_time(trim_start);
                        current_time.set(trim_start);
                    }
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

    let ready_emitted = use_mut_ref(|| false);
    let pending_play = use_mut_ref(|| false);
    {
        let ready_emitted = ready_emitted.clone();
        let pending_play = pending_play.clone();
        let src = props.src.clone();
        use_effect_with(src, move |_| {
            *ready_emitted.borrow_mut() = false;
            *pending_play.borrow_mut() = false;
            || ()
        });
    }

    let on_loaded_data = {
        let on_ready = props.on_ready.clone();
        let ready_emitted = ready_emitted.clone();
        let video_ref = video_ref.clone();
        let is_playing = is_playing.clone();
        let pending_play = pending_play.clone();
        let should_play = props.should_play;
        Callback::from(move |_| {
            web_sys::console::log_1(&"[video_player] on_loaded_data fired".into());
            if !*ready_emitted.borrow() {
                *ready_emitted.borrow_mut() = true;
                web_sys::console::log_1(&"[video_player] emitting on_ready".into());
                if let Some(callback) = &on_ready {
                    callback.emit(());
                }
            }

            if should_play == Some(true) {
                if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                    if v.paused() || *pending_play.borrow() {
                        web_sys::console::log_1(
                            &"[video_player] on_loaded_data: calling v.play()".into(),
                        );
                        let _ = v.play();
                        is_playing.set(true);
                        *pending_play.borrow_mut() = false;
                    }
                }
            }
        })
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
                if dur <= 0.0 {
                    return;
                }

                let trim_start = (*left_value) * dur;
                let trim_end = (*right_value) * dur;
                let trim_len = (trim_end - trim_start).max(0.000_1);

                // slider gives 0..1 within trim
                let p = e
                    .target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value_as_number()
                    .clamp(0.0, 1.0);

                let t = trim_start + p * trim_len;
                v.set_current_time(t);
                current_time.set(t);
            }
        })
    };

    // Icon choices
    let play_icon = if *is_playing {
        IconId::LucidePause
    } else {
        IconId::LucidePlay
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
    let has_crop = *crop_top > 0 || *crop_bottom > 0 || *crop_left > 0 || *crop_right > 0;

    let crop_preview_overlay = if *show_crop_settings && has_crop {
        if let Some(video) = video_ref.cast::<HtmlVideoElement>() {
            let intrinsic_width = video.video_width() as f64;
            let intrinsic_height = video.video_height() as f64;
            let bounds = video.get_bounding_client_rect();
            let container_width = bounds.width();
            let container_height = bounds.height();

            if intrinsic_width > 0.0
                && intrinsic_height > 0.0
                && container_width > 0.0
                && container_height > 0.0
            {
                let video_aspect = intrinsic_width / intrinsic_height;
                let container_aspect = container_width / container_height;
                let (content_width, content_height, offset_x, offset_y) =
                    if container_aspect > video_aspect {
                        let content_height = container_height;
                        let content_width = content_height * video_aspect;
                        let offset_x = (container_width - content_width) / 2.0;
                        (content_width, content_height, offset_x, 0.0)
                    } else {
                        let content_width = container_width;
                        let content_height = content_width / video_aspect;
                        let offset_y = (container_height - content_height) / 2.0;
                        (content_width, content_height, 0.0, offset_y)
                    };

                let clamped_top = (*crop_top).min(video.video_height()) as f64;
                let clamped_bottom = (*crop_bottom).min(video.video_height()) as f64;
                let clamped_left = (*crop_left).min(video.video_width()) as f64;
                let clamped_right = (*crop_right).min(video.video_width()) as f64;

                let top_pct = (clamped_top / intrinsic_height * 100.0).clamp(0.0, 100.0);
                let bottom_pct = (clamped_bottom / intrinsic_height * 100.0).clamp(0.0, 100.0);
                let left_pct = (clamped_left / intrinsic_width * 100.0).clamp(0.0, 100.0);
                let right_pct = (clamped_right / intrinsic_width * 100.0).clamp(0.0, 100.0);

                let remaining_width_pct = (100.0 - left_pct - right_pct).max(0.0);
                let remaining_height_pct = (100.0 - top_pct - bottom_pct).max(0.0);

                html! {
                    <div
                        class="video-crop-halo-layer"
                        style={format!(
                            "left: {:.3}px; top: {:.3}px; width: {:.3}px; height: {:.3}px;",
                            offset_x, offset_y, content_width, content_height
                        )}
                    >
                        <div class="video-crop-halo-inner">
                            if top_pct > 0.0 {
                                <div class="video-crop-halo video-crop-halo-top" style={format!("height: {:.4}%;", top_pct)}></div>
                            }
                            if bottom_pct > 0.0 {
                                <div class="video-crop-halo video-crop-halo-bottom" style={format!("height: {:.4}%;", bottom_pct)}></div>
                            }
                            if left_pct > 0.0 {
                                <div
                                    class="video-crop-halo video-crop-halo-left"
                                    style={format!(
                                        "width: {:.4}%; top: {:.4}%; bottom: {:.4}%;",
                                        left_pct, top_pct, bottom_pct
                                    )}
                                ></div>
                            }
                            if right_pct > 0.0 {
                                <div
                                    class="video-crop-halo video-crop-halo-right"
                                    style={format!(
                                        "width: {:.4}%; top: {:.4}%; bottom: {:.4}%;",
                                        right_pct, top_pct, bottom_pct
                                    )}
                                ></div>
                            }
                            if remaining_width_pct > 0.0 && remaining_height_pct > 0.0 {
                                <div
                                    class="video-crop-focus-ring"
                                    style={format!(
                                        "left: {:.4}%; top: {:.4}%; width: {:.4}%; height: {:.4}%;",
                                        left_pct, top_pct, remaining_width_pct, remaining_height_pct
                                    )}
                                ></div>
                            }
                        </div>
                    </div>
                }
            } else {
                html! {}
            }
        } else {
            html! {}
        }
    } else {
        html! {}
    };

    // External play/pause control (trim-aware)
    {
        let video_ref = video_ref.clone();
        let is_playing = is_playing.clone();
        let current_time = current_time.clone();
        let should_play = props.should_play;
        let prev_should_play = use_mut_ref(|| None::<bool>);
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let ended_fired = ended_fired.clone();
        let pending_play = pending_play.clone();
        let ready_emitted = ready_emitted.clone();
        let on_ready = props.on_ready.clone();

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
                            *ended_fired.borrow_mut() = false;
                            web_sys::console::log_1(&format!(
                                "[video_player] should_play=true, dur={}, readyState={}, networkState={}",
                                dur,
                                v.ready_state(),
                                v.network_state()
                            ).into());
                            if dur.is_finite() && dur > 0.0 {
                                let t = v.current_time();
                                if t < trim_start || t >= trim_end {
                                    v.set_current_time(trim_start);
                                    current_time.set(trim_start);
                                }
                            }

                            *pending_play.borrow_mut() = true;
                            let _ = v.play();
                            is_playing.set(true);

                            // If video data is already cached (from overview's <video preload="auto">),
                            // loadeddata won't fire again so on_ready would never be emitted.
                            if v.ready_state() >= 2 && !*ready_emitted.borrow() {
                                *ready_emitted.borrow_mut() = true;
                                web_sys::console::log_1(&format!("[video_player] already loaded (readyState={}), emitting on_ready from should_play", v.ready_state()).into());
                                if let Some(callback) = &on_ready {
                                    callback.emit(());
                                }
                            }
                        }
                        Some(false) => {
                            *pending_play.borrow_mut() = false;
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

    // Handle reset (go to trim start). Reset is intentionally one-way: the
    // parent transport owns whether playback resumes.
    {
        let video_ref = video_ref.clone();
        let current_time = current_time.clone();
        let is_playing = is_playing.clone();
        let should_reset = props.should_reset;
        let left_value = left_value.clone();
        let pending_play = pending_play.clone();

        use_effect_with(should_reset, move |should_reset| {
            if *should_reset {
                if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                    let dur = v.duration();
                    let target = if dur.is_finite() && dur > 0.0 {
                        (*left_value) * dur
                    } else {
                        0.0
                    };
                    *pending_play.borrow_mut() = false;
                    v.pause().ok();
                    v.set_current_time(target);
                    current_time.set(target);
                    is_playing.set(false);
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

                        let seek_time = trim_start + (percentage.clamp(0.0, 1.0) * trim_len);

                        v.set_current_time(seek_time);
                        current_time.set(seek_time);
                    }
                }
            }
        });
    }

    // Apply external volume/mute controls.
    {
        let video_ref = video_ref.clone();
        let volume = props.volume;
        let is_muted = props.is_muted;

        use_effect_with((volume, is_muted), move |(volume, is_muted)| {
            if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                v.set_volume(volume.clamp(0.0, 1.0));
                v.set_muted(*is_muted);
            }
            || ()
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
    let is_cropping = props.is_cropping.unwrap_or(false);

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

    let on_crop_click = {
        let video_ref = video_ref.clone();
        let crop_top = crop_top.clone();
        let crop_bottom = crop_bottom.clone();
        let crop_left = crop_left.clone();
        let crop_right = crop_right.clone();
        let on_crop_video = props.on_crop_video.clone();
        let on_error_message_change = props.on_error_message_change.clone();

        Callback::from(move |_| {
            let top = *crop_top;
            let bottom = *crop_bottom;
            let left = *crop_left;
            let right = *crop_right;

            if top == 0 && bottom == 0 && left == 0 && right == 0 {
                return;
            }

            let Some(video) = video_ref.cast::<HtmlVideoElement>() else {
                if let Some(cb) = &on_error_message_change {
                    cb.emit(Some(
                        "Crop settings could not be applied because the video is not ready."
                            .to_string(),
                    ));
                }
                return;
            };

            let video_width = video.video_width();
            let video_height = video.video_height();

            if video_width == 0 || video_height == 0 {
                if let Some(cb) = &on_error_message_change {
                    cb.emit(Some(
                        "Crop settings could not be applied because the video dimensions are not available yet."
                            .to_string(),
                    ));
                }
                return;
            }

            if top + bottom >= video_height {
                if let Some(cb) = &on_error_message_change {
                    cb.emit(Some(format!(
                        "Top + bottom crop must be less than the video height ({} px).",
                        video_height
                    )));
                }
                return;
            }

            if left + right >= video_width {
                if let Some(cb) = &on_error_message_change {
                    cb.emit(Some(format!(
                        "Left + right crop must be less than the video width ({} px).",
                        video_width
                    )));
                }
                return;
            }

            if let Some(cb) = &on_error_message_change {
                cb.emit(None);
            }

            if let Some(on_crop) = &on_crop_video {
                on_crop.emit((top, bottom, left, right));
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

    let on_preprocess_enabled_change = {
        let preprocess_enabled = preprocess_enabled.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            preprocess_enabled.set(input.checked());
        })
    };

    let on_preprocess_preset_change = {
        let preprocess_preset = preprocess_preset.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            preprocess_preset.set(select.value());
        })
    };

    let on_preprocess_custom_input = {
        let preprocess_custom = preprocess_custom.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            preprocess_custom.set(input.value());
        })
    };

    let on_preprocess_apply = {
        let on_preprocess_video = props.on_preprocess_video.clone();
        let preprocess_preset = preprocess_preset.clone();
        let preprocess_custom = preprocess_custom.clone();
        let preprocess_enabled = preprocess_enabled.clone();

        Callback::from(move |_| {
            if !*preprocess_enabled {
                return;
            }
            if let Some(cb) = &on_preprocess_video {
                let preset = (*preprocess_preset).clone();
                let custom = if preset == "other" {
                    Some((*preprocess_custom).clone())
                } else {
                    None
                };
                cb.emit((preset, custom));
            }
        })
    };

    let on_create_preview = {
        let video_ref = video_ref.clone();
        let project_id = props.project_id.clone();
        let source_file_id = props.source_file_id.clone();
        let source_file_path = props.source_file_path.clone();
        let luminance = props.luminance;
        let font_ratio = props.font_ratio;
        let columns = props.columns;
        let fps = props.fps;
        let generate_colors = generate_colors.clone();
        let is_creating_preview = is_creating_preview.clone();
        let on_preview_created = props.on_preview_created.clone();
        let on_error_message_change = props.on_error_message_change.clone();

        Callback::from(move |_| {
            let color = *generate_colors;
            let (Some(project_id), Some(source_file_id), Some(file_path)) = (
                project_id.clone(),
                source_file_id.clone(),
                source_file_path.clone(),
            ) else {
                return;
            };

            // Get current timestamp from video element
            let timestamp = if let Some(v) = video_ref.cast::<HtmlVideoElement>() {
                v.current_time()
            } else {
                0.0
            };

            is_creating_preview.set(true);
            if let Some(cb) = &on_error_message_change {
                cb.emit(None);
            }

            let is_creating_preview = is_creating_preview.clone();
            let on_preview_created = on_preview_created.clone();
            let on_error_message_change = on_error_message_change.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let request = CreatePreviewRequest {
                    video_path: file_path,
                    timestamp,
                    luminance,
                    font_ratio,
                    columns,
                    fps,
                    color,
                    project_id,
                    source_file_id,
                };
                let invoke_args = CreatePreviewInvokeArgs { request };
                let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                let result = tauri_invoke("create_preview", args).await;

                is_creating_preview.set(false);

                match serde_wasm_bindgen::from_value::<serde_json::Value>(result.clone()) {
                    Ok(preview) => {
                        web_sys::console::log_1(&format!("✅ Preview created successfully").into());
                        if let Some(cb) = &on_preview_created {
                            cb.emit(preview);
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("❌ Failed to create preview: {:?}", e).into(),
                        );
                        if let Some(cb) = &on_error_message_change {
                            cb.emit(Some(
                                "Failed to create preview. Please try again.".to_string(),
                            ));
                        }
                    }
                }
            });
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
        let preprocess_enabled = preprocess_enabled.clone();
        let preprocess_preset = preprocess_preset.clone();
        let preprocess_custom = preprocess_custom.clone();

        let on_conversion_start = props.on_conversion_start.clone();
        let on_conversion_complete = props.on_conversion_complete.clone();
        let on_error_message_change = props.on_error_message_change.clone();

        Callback::from(move |_| {
            let color = *generate_colors;
            let extract_audio = *extract_audio;
            let preprocess_enabled_value = *preprocess_enabled;
            let preprocess_preset_value = if preprocess_enabled_value {
                Some((*preprocess_preset).clone())
            } else {
                None
            };
            let preprocess_custom_value = if preprocess_enabled_value
                && preprocess_preset_value.as_deref() == Some("other")
            {
                Some((*preprocess_custom).clone())
            } else {
                None
            };
            let (Some(project_id), Some(source_file_id), Some(file_path)) = (
                project_id.clone(),
                source_file_id.clone(),
                source_file_path.clone(),
            ) else {
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

            if preprocess_enabled_value
                && preprocess_preset_value.as_deref() == Some("other")
                && preprocess_custom_value
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
            {
                if let Some(cb) = &on_conversion_complete {
                    cb.emit(source_file_id.clone());
                }
                if let Some(cb) = &on_error_message_change {
                    cb.emit(Some("Preprocessing is enabled with 'Other', but no ffmpeg filter string was provided.".to_string()));
                }
                return;
            }

            let on_conversion_complete = on_conversion_complete.clone();
            let on_error_message_change = on_error_message_change.clone();
            let source_file_id_for_complete = source_file_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                // Start conversion (returns immediately, progress/completion handled by global listeners)
                web_sys::console::log_1(
                    &format!(
                        "🚀 Starting tauri_invoke for: {}",
                        source_file_id_for_complete
                    )
                    .into(),
                );
                let invoke_args = ConvertToAsciiInvokeArgs {
                    request: ConvertToAsciiRequest {
                        file_path,
                        luminance,
                        font_ratio,
                        columns,
                        fps: Some(fps),
                        project_id,
                        source_file_id,
                        custom_name,
                        color,
                        extract_audio,
                        preprocess_enabled: preprocess_enabled_value,
                        preprocess_preset: preprocess_preset_value,
                        preprocess_custom: preprocess_custom_value,
                    },
                };
                let args = serde_wasm_bindgen::to_value(&invoke_args).unwrap();
                let result = tauri_invoke("convert_to_ascii", args).await;

                // Backend returns immediately with "Conversion started for: {name}"
                // Actual completion is handled via conversion-complete event in project.rs
                match serde_wasm_bindgen::from_value::<String>(result.clone()) {
                    Ok(msg) => {
                        web_sys::console::log_1(
                            &format!("✅ Conversion initiated: {}", msg).into(),
                        );
                    }
                    Err(e) => {
                        // Error starting conversion
                        web_sys::console::log_1(
                            &format!(
                                "❌ Failed to start conversion: {:?}, raw result: {:?}",
                                e, result
                            )
                            .into(),
                        );
                        // Remove from active conversions since it failed to start
                        if let Some(cb) = &on_conversion_complete {
                            cb.emit(source_file_id_for_complete.clone());
                        }
                        if let Some(cb) = &on_error_message_change {
                            cb.emit(Some(
                                "Failed to start conversion. Please try again.".to_string(),
                            ));
                        }
                    }
                }
            });
        })
    };

    html! {
        <div id="video-player" class={classes!("video-player", props.class.clone())}>
            <div id="video-wrap" class="video-wrap">
                <video id="video-element" ref={video_ref.clone()} class="video" src={props.src.clone()} preload="auto" playsinline=true ontimeupdate={on_time_update} onloadedmetadata={on_loaded_metadata} onloadeddata={on_loaded_data} onplay={on_play} onpause={on_pause} onerror={on_error} onclick={on_toggle.clone()} />
                {crop_preview_overlay}
                if let Some(msg) = &*error_text {
                    <div id="video-error-overlay" class="error-overlay">{msg}</div>
                }
            </div>

            <div class="controls" id="video-controls">
                <div class="control-row" id="video-progress">
                    <input id="video-progress-bar" class="progress" type="range" min="0" max="1" step="0.0001" value={progress_in_trim.to_string()} oninput={on_seek_input_trim.clone()} title="Seek (within trim)" />
                    <button id="video-play-btn" class="ctrl-btn" type="button" onclick={on_toggle.clone()} title="Play/Pause">
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="video-cut-controls">
                    <div id="video-range-selector" class="range-selector">
                        <div id="video-range-track" class="range-selector-track"></div>
                        <input id="video-range-left" class="range-selector-input range-left" type="range" min="0" max="1" step="0.001" value={left_value.to_string()} oninput={on_left_range_input.clone()} title="Range start" />
                        <input id="video-range-right" class="range-selector-input range-right" type="range" min="0" max="1" step="0.001" value={right_value.to_string()} oninput={on_right_range_input.clone()} title="Range end" />
                    </div>
                    <button id="video-cut-btn" class="ctrl-btn" type="button" onclick={on_cut_click.clone()} disabled={is_cutting || props.on_cut_video.is_none()} title="Cut video segment">
                        <Icon icon_id={IconId::LucideScissors} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row timestamp-row" id="video-timestamp-row">
                    <div id="video-timestamp-actions" class="timestamp-actions">
                        <button id="video-color-toggle-btn" class={classes!("ctrl-btn", "color-toggle-btn", (*generate_colors).then_some("active"))} type="button" onclick={{
                            let generate_colors = generate_colors.clone();
                            Callback::from(move |_| generate_colors.set(!*generate_colors))
                        }} title={if *generate_colors { "Color generation enabled" } else { "Color generation disabled" }}>
                            if *generate_colors {
                                <Icon icon_id={IconId::LucideBrush} width={"20"} height={"20"} />
                            } else {
                                <Icon icon_id={IconId::LucideXCircle} width={"20"} height={"20"} />
                            }
                        </button>
                        <button id="video-audio-toggle-btn" class={classes!("ctrl-btn", "audio-toggle-btn", (*extract_audio).then_some("active"))} type="button" onclick={{
                            let extract_audio = extract_audio.clone();
                            Callback::from(move |_| extract_audio.set(!*extract_audio))
                        }} title={if *extract_audio { "Audio extraction enabled" } else { "Audio extraction disabled" }}>
                            <Icon icon_id={IconId::LucideVolume2} width={"20"} height={"20"} />
                        </button>
                        <button id="video-advanced-settings-btn" class={classes!("ctrl-btn", "advanced-settings-btn", (*show_advanced_settings).then_some("active"))} type="button" onclick={{
                            let show_advanced_settings = show_advanced_settings.clone();
                            Callback::from(move |_| show_advanced_settings.set(!*show_advanced_settings))
                        }} title={if *show_advanced_settings { "Hide advanced settings" } else { "Show advanced settings" }}>
                            <Icon icon_id={IconId::LucideSettings} width={"20"} height={"20"} />
                        </button>
                        <button id="frames-crop-settings-btn" class={classes!("ctrl-btn", "advanced-settings-btn", (*show_crop_settings).then_some("active"))} type="button" onclick={{
                            let show_crop_settings = show_crop_settings.clone();
                            Callback::from(move |_| show_crop_settings.set(!*show_crop_settings))
                        }} title={if *show_crop_settings { "Hide crop settings" } else { "Show crop settings" }}>
                            <Icon icon_id={IconId::LucideCrop} width={"20"} height={"20"} />
                        </button>
                    </div>
                    <span id="video-timestamp-overlay" class="timestamp-text">{timestamp}</span>
                </div>

                if *show_crop_settings {
                    <div class="control-row expanded" id="video-crop-settings">
                        <select id="video-crop-side-select" class="setting-input crop-preset-select" onchange={{
                            let crop_side = crop_side.clone();
                            Callback::from(move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                crop_side.set(select.value());
                            })
                        }} value={(*crop_side).clone()}>
                            <option value="top" selected={*crop_side == "top"}>{"Top"}</option>
                            <option value="bottom" selected={*crop_side == "bottom"}>{"Bottom"}</option>
                            <option value="left" selected={*crop_side == "left"}>{"Left"}</option>
                            <option value="right" selected={*crop_side == "right"}>{"Right"}</option>
                        </select>
                        <input
                            id="video-crop-value-input"
                            type="number"
                            class="setting-input"
                            style="width: 68px;"
                            min="0"
                            value={match (*crop_side).as_str() {
                                "top" => (*crop_top).to_string(),
                                "bottom" => (*crop_bottom).to_string(),
                                "left" => (*crop_left).to_string(),
                                "right" => (*crop_right).to_string(),
                                _ => "0".to_string(),
                            }}
                            oninput={{
                                let crop_side = crop_side.clone();
                                let crop_top = crop_top.clone();
                                let crop_bottom = crop_bottom.clone();
                                let crop_left = crop_left.clone();
                                let crop_right = crop_right.clone();
                                Callback::from(move |e: InputEvent| {
                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    let value = input.value().parse::<u32>().unwrap_or(0);
                                    match (*crop_side).as_str() {
                                        "top" => crop_top.set(value),
                                        "bottom" => crop_bottom.set(value),
                                        "left" => crop_left.set(value),
                                        "right" => crop_right.set(value),
                                        _ => {}
                                    }
                                })
                            }}
                            title="Number of pixels to crop"
                        />
                        <span class="crop-label">{"px"}</span>
                        <div style="flex: 1;"></div>
                        <button
                            id="crop-video-button"
                            class="ctrl-btn"
                            type="button"
                            disabled={is_cropping || props.on_crop_video.is_none() || (*crop_top == 0 && *crop_bottom == 0 && *crop_left == 0 && *crop_right == 0)}
                            title="Apply crop to video"
                            onclick={on_crop_click.clone()}
                        >
                            <Icon icon_id={IconId::LucideCrop} width={"20"} height={"20"} />
                        </button>
                    </div>
                }

                if *show_advanced_settings {
                    <div class={classes!("control-row", (*preprocess_enabled).then_some("expanded"))} id="video-preprocess-settings">
                        <label id="video-settings-preprocess-checkbox-label" class="preprocess-toggle-label" title="Apply an ffmpeg preprocessing filter before generating ASCII frames">
                            <input id="video-settings-preprocess-checkbox" type="checkbox" checked={*preprocess_enabled} onchange={on_preprocess_enabled_change.clone()} />
                            <span>{"Preprocessing"}</span>
                        </label>
                        if *preprocess_enabled {
                            <select id="video-settings-preprocess-preset-select" class="setting-input preprocess-preset-select" onchange={on_preprocess_preset_change.clone()} value={(*preprocess_preset).clone()}>
                                {for PREPROCESS_PRESET_OPTIONS.iter().map(|(value, label)| {
                                    html! { <option value={(*value).to_string()}>{*label}</option> }
                                })}
                            </select>
                            if (*preprocess_preset).as_str() == "other" {
                                <input
                                    id="video-settings-preprocess-custom-input"
                                    type="text"
                                    class="setting-input preprocess-custom-input"
                                    value={(*preprocess_custom).clone()}
                                    oninput={on_preprocess_custom_input.clone()}
                                    placeholder={"format=gray,edgedetect=...,eq=..."}
                                    title="Custom ffmpeg -vf filtergraph (without the ffmpeg command)"
                                />
                            }
                            <div style="flex: 1;"></div>
                            <button id="video-preprocess-apply-btn" class="ctrl-btn" type="button"
                                disabled={props.is_preprocessing.unwrap_or(false) || props.on_preprocess_video.is_none()}
                                title="Apply preprocessing filter to video"
                                onclick={on_preprocess_apply.clone()}>
                                <Icon icon_id={IconId::LucideWand} width={"20"} height={"20"} />
                            </button>
                        }
                    </div>
                }

                <div id="video-controls-divider" class="controls-divider"></div>

                <div class="control-row" id="conversion-settings">
                    <div id="video-settings-info" class="settings-info">
                        <div id="video-settings-fps-row" class="settings-row">
                            <span id="video-settings-fps-label" class="settings-label">{"FPS:"}</span>
                            <input id="video-settings-fps-input" type="number" class="setting-input" value={props.fps.to_string()} min="1" max="120" oninput={on_fps_input.clone()} />
                        </div>
                        <div id="video-settings-font-ratio-row" class="settings-row">
                            <span id="video-settings-font-ratio-label" class="settings-label">{"FONT RATIO:"}</span>
                            <input id="video-settings-font-ratio-input" type="number" class="setting-input" value={props.font_ratio.to_string()} min="0.1" max="2.0" step="0.1" oninput={on_font_ratio_input.clone()} />
                        </div>
                        <div id="video-settings-luminance-row" class="settings-row">
                            <span id="video-settings-luminance-label" class="settings-label">{"LUMINANCE:"}</span>
                            <input id="video-settings-luminance-input" type="number" class="setting-input" value={props.luminance.to_string()} min="0" max="255" oninput={on_luminance_input.clone()} />
                        </div>
                        <div id="video-settings-columns-row" class="settings-row">
                            <span id="video-settings-columns-label" class="settings-label">{"COLUMNS:"}</span>
                            <input id="video-settings-columns-input" type="number" class="setting-input" value={props.columns.to_string()} min="1" max="2000" oninput={on_columns_input.clone()} />
                        </div>
                    </div>
                    <button id="video-preview-btn" class="ctrl-btn" type="button" onclick={on_create_preview.clone()} disabled={*is_creating_preview || props.project_id.is_none() || props.source_file_id.is_none() || props.source_file_path.is_none()} title="Create preview of current frame">
                        <Icon icon_id={IconId::LucideCamera} width={"20"} height={"20"} />
                    </button>
                    <button id="video-convert-btn" class="ctrl-btn" type="button" onclick={on_convert_click.clone()} disabled={is_converting || props.project_id.is_none() || props.source_file_id.is_none() || props.source_file_path.is_none()} title="Convert to ASCII">
                        <Icon icon_id={IconId::LucideWand} width={"20"} height={"20"} />
                    </button>
                </div>

            </div>
        </div>
    }
}
