use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use serde::{Deserialize, Serialize};
use serde_json::json;
use yew_icons::{Icon, IconId};
use std::cell::RefCell;
use std::rc::Rc;
use gloo_timers::callback::Interval;

#[wasm_bindgen(inline_js = r#"
export async function tauriInvoke(cmd, args) {
  const g = globalThis.__TAURI__;
  if (g?.core?.invoke) return g.core.invoke(cmd, args);
  if (g?.tauri?.invoke) return g.tauri.invoke(cmd, args);
  throw new Error('Tauri invoke is not available');
}

export function observeResize(element, callback) {
  const observer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const { width, height } = entry.contentRect;
      callback(width, height);
    }
  });
  observer.observe(element);
  return observer;
}

export function disconnectObserver(observer) {
  observer.disconnect();
}

export function startInterval(callback, intervalMs) {
  return setInterval(callback, intervalMs);
}

export function clearIntervalById(id) {
  clearInterval(id);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_name = observeResize)]
    fn observe_resize(element: &web_sys::Element, callback: &Closure<dyn Fn(f64, f64)>) -> JsValue;

    #[wasm_bindgen(js_name = disconnectObserver)]
    fn disconnect_observer(observer: &JsValue);

    #[wasm_bindgen(js_name = startInterval)]
    fn start_interval(callback: &Closure<dyn Fn()>, interval_ms: u32) -> i32;

    #[wasm_bindgen(js_name = clearIntervalById)]
    fn clear_interval_by_id(id: i32);
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FrameFile {
    path: String,
    name: String,
    index: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AsciiConversion {
    id: String,
    folder_name: String,
    folder_path: String,
    frame_count: i32,
    source_file_id: String,
    project_id: String,
    settings: ConversionSettings,
    creation_date: String,
    total_size: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConversionSettings {
    pub fps: u32,
    pub font_ratio: f32,
    pub luminance: u8,
    pub columns: u32,
    pub frame_speed: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SpeedSelection {
    Custom,
    Base,
}

#[derive(Properties, PartialEq, Clone)]
pub struct AsciiFramesViewerProps {
    pub directory_path: String,
    #[prop_or(24)]
    pub fps: u32,
    #[prop_or(None)]
    pub settings: Option<ConversionSettings>,
    /// External control: when Some(true), play; when Some(false), pause; when None, no external control
    #[prop_or_default]
    pub should_play: Option<bool>,
    /// External control: when true, reset to beginning
    #[prop_or(false)]
    pub should_reset: bool,
    /// External control: seek to percentage (0.0-1.0)
    #[prop_or_default]
    pub seek_percentage: Option<f64>,
    /// Callback to report loading state to parent
    #[prop_or_default]
    pub on_loading_changed: Option<Callback<bool>>,
    /// Current frame speed override
    #[prop_or(None)]
    pub frame_speed: Option<u32>,
    /// Callback when frame speed changes
    #[prop_or_default]
    pub on_frame_speed_change: Option<Callback<u32>>,
    /// Which speed input is currently selected
    #[prop_or(SpeedSelection::Custom)]
    pub selected_speed: SpeedSelection,
    /// Callback when speed selection changes
    #[prop_or_default]
    pub on_speed_selection_change: Option<Callback<SpeedSelection>>,
    /// Whether to loop playback
    #[prop_or(true)]
    pub loop_enabled: bool,
    /// Callback when loop setting changes
    #[prop_or_default]
    pub on_loop_change: Option<Callback<bool>>,
    /// Callback to cut/export frame range: emits (start_frame, end_frame) indices
    #[prop_or_default]
    pub on_cut_frames: Option<Callback<(usize, usize)>>,
    /// Whether a cut operation is in progress
    #[prop_or(false)]
    pub is_cutting: bool,
}

#[function_component(AsciiFramesViewer)]
pub fn ascii_frames_viewer(props: &AsciiFramesViewerProps) -> Html {
    let frames = use_state(|| Vec::<String>::new());
    let current_index = use_state(|| 0usize);
    let current_index_ref = use_mut_ref(|| 0usize);
    let is_playing = use_state(|| false);
    let is_loading = use_state(|| true);
    let error_message = use_state(|| None::<String>);
    let loading_progress = use_state(|| (0, 0)); // (loaded, total)
    // Dual range selector state (0..1)
    let left_value = use_state(|| 0.0f64);
    let right_value = use_state(|| 1.0f64);
    // Store interval handle for animation
    let interval_handle: Rc<RefCell<Option<Interval>>> = use_mut_ref(|| None);
    let on_loading_changed = props.on_loading_changed.clone();

    // Auto-sizing state
    let container_ref = use_node_ref();
    let calculated_font_size = use_state(|| 10.0f64); // Default font size in px
    let container_size = use_state(|| (0.0f64, 0.0f64)); // (width, height) from ResizeObserver

    // Sync ref when current_index state changes
    {
        let current_index_ref = current_index_ref.clone();
        use_effect_with(*current_index, move |idx| {
            *current_index_ref.borrow_mut() = *idx;
            || ()
        });
    }

    // Load frames when directory_path changes
    {
        let directory_path = props.directory_path.clone();
        let frames = frames.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let current_index = current_index.clone();
        let interval_handle = interval_handle.clone();
        let is_playing = is_playing.clone();

        let loading_progress_clone = loading_progress.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        use_effect_with(directory_path.clone(), move |_| {
            loading_progress_clone.set((0, 0));
            is_loading.set(true);
            if let Some(callback) = &on_loading_changed {
                callback.emit(true);
            }
            error_message.set(None);
            frames.set(Vec::new());
            current_index.set(0);
            is_playing.set(false); // Stop playback when loading new frames
            left_value.set(0.0);
            right_value.set(1.0);

            // Cancel any running animation interval
            interval_handle.borrow_mut().take();

            let on_loading_changed_async = on_loading_changed.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // First, try to get conversion info to get the total frame count
                let conversion_args = serde_wasm_bindgen::to_value(&json!({ "folderPath": directory_path })).unwrap();
                let total_frames = match tauri_invoke("get_conversion_by_folder_path", conversion_args).await {
                    result => {
                        match serde_wasm_bindgen::from_value::<Option<AsciiConversion>>(result) {
                            Ok(Some(conversion)) => conversion.frame_count as usize,
                            _ => 0
                        }
                    }
                };

                // Get list of frame files
                let args = serde_wasm_bindgen::to_value(&json!({ "directoryPath": directory_path })).unwrap();
                match tauri_invoke("get_frame_files", args).await {
                    result => {
                        match serde_wasm_bindgen::from_value::<Vec<FrameFile>>(result) {
                            Ok(frame_files) => {
                                let total_count = if total_frames > 0 { total_frames } else { frame_files.len() };
                                loading_progress_clone.set((0, total_count));

                                // Load all frame contents
                                let mut loaded_frames = Vec::new();
                                for (i, frame_file) in frame_files.into_iter().enumerate() {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "filePath": frame_file.path })).unwrap();
                                    match tauri_invoke("read_frame_file", args).await {
                                        result => {
                                            match serde_wasm_bindgen::from_value::<String>(result) {
                                                Ok(content) => {
                                                    loaded_frames.push(content);
                                                    loading_progress_clone.set((i + 1, total_count));
                                                }
                                                Err(e) => {
                                                    error_message.set(Some(format!("Failed to read frame {}: {:?}", frame_file.name, e)));
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }

                                if loaded_frames.is_empty() {
                                    error_message.set(Some("No frames found in directory".to_string()));
                                } else {
                                    frames.set(loaded_frames);
                                }
                                is_loading.set(false);
                                if let Some(callback) = on_loading_changed_async {
                                    callback.emit(false);
                                }
                            }
                            Err(e) => {
                                error_message.set(Some(format!("Failed to list frames: {:?}", e)));
                                is_loading.set(false);
                                if let Some(callback) = on_loading_changed_async {
                                    callback.emit(false);
                                }
                            }
                        }
                    }
                }
            });

            || ()
        });
    }

    // Calculate current FPS from props (used for both animation and display)
    let current_fps = match props.selected_speed {
        SpeedSelection::Custom => props.frame_speed.unwrap_or(props.fps),
        SpeedSelection::Base => props.settings.as_ref().map(|s| s.fps).unwrap_or(props.fps),
    };

    // Effect to start/stop animation when is_playing or speed changes
    {
        let current_index = current_index.clone();
        let current_index_ref = current_index_ref.clone();
        let is_playing_state = is_playing.clone();
        let frames = frames.clone();
        let interval_handle = interval_handle.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let loop_enabled = props.loop_enabled;
        let playing = *is_playing;
        let frame_count = frames.len();

        use_effect_with(
            (playing, current_fps, frame_count),
            move |_| {
                // Always clear existing interval first
                interval_handle.borrow_mut().take();

                if playing && frame_count > 0 {
                    let interval_ms = (1000.0 / current_fps as f64).max(1.0) as u32;
                    let current_index_clone = current_index.clone();
                    let current_index_ref_clone = current_index_ref.clone();
                    let is_playing_clone = is_playing_state.clone();
                    let interval_handle_clone = interval_handle.clone();
                    let left_value_clone = left_value.clone();
                    let right_value_clone = right_value.clone();

                    let interval = Interval::new(interval_ms, move || {
                        let max_idx = frame_count.saturating_sub(1) as f64;
                        let effective_start = (*left_value_clone * max_idx).round() as usize;
                        let effective_end = (*right_value_clone * max_idx).round() as usize;
                        
                        // Use ref for latest value
                        let mut current = *current_index_ref_clone.borrow();
                        
                        if current < effective_start {
                             current = effective_start;
                        }

                        if current >= effective_end {
                            if loop_enabled {
                                current = effective_start;
                                *current_index_ref_clone.borrow_mut() = current;
                                current_index_clone.set(current);
                            } else {
                                interval_handle_clone.borrow_mut().take();
                                is_playing_clone.set(false);
                            }
                        } else {
                            current += 1;
                            *current_index_ref_clone.borrow_mut() = current;
                            current_index_clone.set(current);
                        }
                    });

                    // Store the interval to keep it alive
                    *interval_handle.borrow_mut() = Some(interval);
                }

                || ()
            }
        );
    }

    // External play/pause control
    {
        let is_playing = is_playing.clone();
        let current_index = current_index.clone();
        let should_play = props.should_play;
        let prev_should_play = use_mut_ref(|| None::<bool>);

        use_effect_with(should_play, move |should_play| {
            let current = *should_play;
            let prev = *prev_should_play.borrow();

            // Only act on changes
            if current != prev {
                match current {
                    Some(true) => {
                        // Check if this is resuming from pause or starting fresh
                        if prev == Some(false) {
                            // Resuming from pause - continue from current frame
                            is_playing.set(true);
                        } else {
                            // Fresh start - reset to first frame
                            current_index.set(0);
                            is_playing.set(true);
                        }
                    }
                    Some(false) => {
                        // Pause
                        is_playing.set(false);
                    }
                    None => {
                        // No external control - do nothing
                    }
                }
                *prev_should_play.borrow_mut() = current;
            }
            || ()
        });
    }

    // Handle reset
    {
        let current_index = current_index.clone();
        let should_reset = props.should_reset;

        use_effect_with(should_reset, move |should_reset| {
            if *should_reset {
                current_index.set(0);
            }
        });
    }

    // Handle seek percentage
    {
        let current_index = current_index.clone();
        let frames = frames.clone();
        let seek_percentage = props.seek_percentage;

        use_effect_with(seek_percentage, move |seek_percentage| {
            if let Some(percentage) = seek_percentage {
                let frame_count = frames.len();
                if frame_count > 0 {
                    let target_frame = ((frame_count - 1) as f64 * percentage).round() as usize;
                    let clamped_frame = target_frame.min(frame_count - 1);
                    current_index.set(clamped_frame);
                }
            }
        });
    }

    // Clamp current frame to range when range selection changes
    {
        let current_index = current_index.clone();
        let frames = frames.clone();
        let left_val = *left_value;
        let right_val = *right_value;

        use_effect_with((left_val, right_val, frames.len()), move |_| {
            let frame_count = frames.len();
            if frame_count > 0 {
                let max_idx = frame_count.saturating_sub(1) as f64;
                let range_start = (left_val * max_idx).round() as usize;
                let range_end = (right_val * max_idx).round() as usize;

                let current = *current_index;
                if current < range_start || current > range_end {
                    // Jump to range start if current frame is outside range
                    current_index.set(range_start);
                }
            }
            || ()
        });
    }

    // ResizeObserver to track container size changes
    {
        let container_ref = container_ref.clone();
        let container_size = container_size.clone();
        let observer_handle: Rc<RefCell<Option<JsValue>>> = use_mut_ref(|| None);

        use_effect_with(container_ref.clone(), move |container_ref| {
            let container_size = container_size.clone();
            let observer_handle = observer_handle.clone();

            if let Some(element) = container_ref.cast::<web_sys::Element>() {
                let container_size_clone = container_size.clone();
                let closure = Closure::wrap(Box::new(move |width: f64, height: f64| {
                    container_size_clone.set((width, height));
                }) as Box<dyn Fn(f64, f64)>);

                let observer = observe_resize(&element, &closure);
                *observer_handle.borrow_mut() = Some(observer);

                // Keep closure alive
                closure.forget();
            }

            // Return cleanup function
            move || {
                if let Some(obs) = observer_handle.borrow_mut().take() {
                    disconnect_observer(&obs);
                }
            }
        });
    }

    // Auto-size font to fit container when frames or container size changes
    {
        let frames = frames.clone();
        let calculated_font_size = calculated_font_size.clone();
        let is_loading = is_loading.clone();
        let container_width = container_size.0;
        let container_height = container_size.1;

        use_effect_with((frames.len(), (*is_loading).clone(), container_width as i32, container_height as i32), move |_| {
            if frames.is_empty() {
                return;
            }

            // Get the first frame to determine dimensions
            if let Some(first_frame) = frames.first() {
                let lines: Vec<&str> = first_frame.lines().collect();
                let row_count = lines.len();
                let col_count = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);

                if row_count == 0 || col_count == 0 {
                    return;
                }

                // Use container dimensions from ResizeObserver (subtract padding)
                let available_width = container_width - 20.0;
                let available_height = container_height - 20.0;

                if available_width <= 0.0 || available_height <= 0.0 {
                    return;
                }

                // For monospace fonts, character width is approximately 0.6 * font_size
                // Line height is 1.11 * font_size (as defined in CSS)
                let char_width_ratio = 0.6;
                let line_height_ratio = 1.11;

                // Calculate max font size that fits width
                let max_font_from_width = available_width / (col_count as f64 * char_width_ratio);

                // Calculate max font size that fits height
                let max_font_from_height = available_height / (row_count as f64 * line_height_ratio);

                // Use the smaller of the two to ensure both dimensions fit
                let optimal_font_size = max_font_from_width.min(max_font_from_height);

                // Clamp to reasonable range (1px to 50px)
                let clamped_font_size = optimal_font_size.max(1.0).min(50.0);

                calculated_font_size.set(clamped_font_size);
            }
        });
    }

    // Toggle play/pause - effect handles actual start/stop
    let on_toggle_play = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| {
            is_playing.set(!*is_playing);
        })
    };

    // Seek to specific frame
    // Seek within the selected range (slider value is 0-1 within range)
    let on_seek = {
        let current_index = current_index.clone();
        let is_playing = is_playing.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let frames = frames.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let slider_val = input.value_as_number();
                    if slider_val.is_finite() {
                        let frame_count = frames.len();
                        if frame_count > 0 {
                            let max_idx = frame_count.saturating_sub(1) as f64;
                            let range_start = (*left_value * max_idx).round() as usize;
                            let range_end = (*right_value * max_idx).round() as usize;

                            // Convert slider position (0-1) to frame index within range
                            let range_len = (range_end - range_start) as f64;
                            let target_frame = (range_start as f64 + slider_val.clamp(0.0, 1.0) * range_len).round() as usize;
                            let clamped_frame = target_frame.min(range_end).max(range_start);

                            // Pause when seeking
                            is_playing.set(false);
                            current_index.set(clamped_frame);
                        }
                    }
                }
            }
        })
    };

    // Speed change handler
    let on_speed_change = {
        let on_frame_speed_change = props.on_frame_speed_change.clone();
        let on_speed_selection_change = props.on_speed_selection_change.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    if let Ok(speed) = input.value().parse::<u32>() {
                        if let Some(callback) = &on_frame_speed_change {
                            callback.emit(speed);
                        }
                        // Auto-select custom speed when user types in it
                        if let Some(selection_callback) = &on_speed_selection_change {
                            selection_callback.emit(SpeedSelection::Custom);
                        }
                    }
                }
            }
        })
    };

    // Speed selection handlers
    let on_select_custom = {
        let on_speed_selection_change = props.on_speed_selection_change.clone();
        Callback::from(move |_| {
            if let Some(callback) = &on_speed_selection_change {
                callback.emit(SpeedSelection::Custom);
            }
        })
    };

    let on_select_base = {
        let on_speed_selection_change = props.on_speed_selection_change.clone();
        Callback::from(move |_| {
            if let Some(callback) = &on_speed_selection_change {
                callback.emit(SpeedSelection::Base);
            }
        })
    };

    // Dual range selector handlers
    let min_gap = 0.01;

    let on_left_range_input = {
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        Callback::from(move |e: web_sys::InputEvent| {
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
        Callback::from(move |e: web_sys::InputEvent| {
            let val = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value_as_number()
                .clamp(0.0, 1.0);
            right_value.set(val.max(*left_value + min_gap));
        })
    };

    let play_icon = if *is_playing { IconId::LucidePause } else { IconId::LucidePlay };
    let frame_count = frames.len();
    let current_frame = (*current_index).min(frame_count.saturating_sub(1));

    // Calculate range bounds in frame indices
    let max_idx = frame_count.saturating_sub(1) as f64;
    let range_start_frame = (*left_value * max_idx).round() as usize;
    let range_end_frame = (*right_value * max_idx).round() as usize;
    let range_frame_count = range_end_frame.saturating_sub(range_start_frame) + 1;

    // Progress within the range (0.0 to 1.0)
    let progress_in_range = if range_frame_count > 1 {
        ((current_frame as f64) - (range_start_frame as f64)) / ((range_end_frame - range_start_frame) as f64)
    } else {
        0.0
    }.clamp(0.0, 1.0);

    // Use current_fps for display (computed earlier at line 244)
    let display_fps = current_fps;

    // Position within the subset (1-based)
    let position_in_subset = if current_frame >= range_start_frame {
        current_frame - range_start_frame + 1
    } else {
        1
    };

    // Compute loading message
    let loading_message = {
        let (loaded, total) = *loading_progress;
        if total > 0 {
            let percentage = if total > 0 { (loaded as f32 / total as f32 * 100.0) as i32 } else { 0 };
            format!("Loading frames... {} / {} ({}%)", loaded, total, percentage)
        } else {
            "Loading frames...".to_string()
        }
    };

    let font_size_style = format!("font-size: {:.2}px;", *calculated_font_size);

    html! {
        <div class="ascii-frames-viewer">
            <div class="frames-display" ref={container_ref}>
                if *is_loading {
                    <div class="loading-frames">{loading_message.clone()}</div>
                } else if let Some(error) = &*error_message {
                    <div class="error-frames">{error}</div>
                } else if frames.is_empty() {
                    <div class="no-frames">{"No frames available"}</div>
                } else {
                    <pre class="ascii-frame-content" style={font_size_style.clone()}>{
                        frames.get(current_frame).cloned().unwrap_or_default()
                    }</pre>
                    <div class="frame-info-overlay">
                        <span class="info-left">{format!("Speed: {}", display_fps)}</span>
                        <span class="info-center">{format!("{}/{}", position_in_subset, range_frame_count)}</span>
                        <span class="info-right">{format!("({}-{})", range_start_frame + 1, range_end_frame + 1)}</span>
                    </div>
                }
            </div>

            <div class="controls" id="frames-controls">
                <div class="control-row" id="frames-progress">
                    <input id="frames-progress-bar" class="progress" type="range" min="0" max="1" step="0.001" value={progress_in_range.to_string()} oninput={on_seek} title="Seek frame" disabled={frame_count == 0} />
                    <button class="ctrl-btn" type="button" onclick={on_toggle_play} title="Play/Pause" disabled={frame_count == 0}>
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="cut-controls">
                    <div class="range-selector">
                        <div class="range-selector-track"></div>
                        <input class="range-selector-input range-left" type="range" min="0" max="1" step="0.001" value={left_value.to_string()} oninput={on_left_range_input.clone()} title="Range start" disabled={frame_count == 0} />
                        <input class="range-selector-input range-right" type="range" min="0" max="1" step="0.001" value={right_value.to_string()} oninput={on_right_range_input.clone()} title="Range end" disabled={frame_count == 0} />
                    </div>
                    <button id="cut-frames-button" class="ctrl-btn" type="button" disabled={props.is_cutting || props.on_cut_frames.is_none() || frame_count == 0} title="Cut frame segment" onclick={{
                            let left_value = left_value.clone();
                            let right_value = right_value.clone();
                            let on_cut_frames = props.on_cut_frames.clone();
                            let frame_count = frame_count;
                            Callback::from(move |_| {
                                if let Some(on_cut) = &on_cut_frames {
                                    if frame_count > 0 {
                                        let max_idx = frame_count.saturating_sub(1) as f64;
                                        let start_frame = (*left_value * max_idx).round() as usize;
                                        let end_frame = (*right_value * max_idx).round() as usize;
                                        on_cut.emit((start_frame, end_frame));
                                    }
                                }
                            })
                        }}>
                        <Icon icon_id={IconId::LucideScissors} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="frames-controls">
                    <label style="font-size: 0.875rem;">{"Speed:"}</label>
                    <input type="number" class={if props.selected_speed == SpeedSelection::Custom {"setting-input speed-input selected"} else {"setting-input speed-input"}} style="width: 68px;" value={props.frame_speed.unwrap_or(props.fps).to_string()} min="1" oninput={on_speed_change} onclick={on_select_custom} title="Frame playback speed (FPS)" />
                    <input type="number" class={if props.selected_speed == SpeedSelection::Base {"setting-input speed-input selected no-spinner"} else {"setting-input speed-input no-spinner"}} style="width: 68px;" value={props.settings.as_ref().map(|s| s.fps).unwrap_or(props.fps).to_string()} readonly=true onclick={on_select_base} title="Default Speed" />
                    <button type="button" class={if props.loop_enabled {"ctrl-btn loop-btn active"} else {"ctrl-btn loop-btn"}} title={if props.loop_enabled {"Loop enabled"} else {"Loop disabled"}}
                        onclick={{
                            let on_loop_change = props.on_loop_change.clone();
                            let loop_enabled = props.loop_enabled;
                            Callback::from(move |_| {
                                if let Some(cb) = &on_loop_change {
                                    cb.emit(!loop_enabled);
                                }
                            })
                        }}>
                        <Icon icon_id={IconId::LucideRepeat} width={"16"} height={"16"} />
                    </button>
                    <div style="flex: 1;"></div>
                    <button type="button" title="Step forward one frame" id="move-frame-forward" class="ctrl-btn" onclick={{
                            let current_index = current_index.clone();
                            let frames = frames.clone();
                            let is_playing = is_playing.clone();
                            let left_value = left_value.clone();
                            let right_value = right_value.clone();
                            Callback::from(move |_| {
                                // Pause if playing
                                if *is_playing {
                                    is_playing.set(false);
                                }
                                // Advance one frame
                                let frame_count = frames.len();
                                if frame_count > 0 {
                                    // Calculate range boundaries
                                    let max_idx = frame_count.saturating_sub(1) as f64;
                                    let range_start = (*left_value * max_idx).round() as usize;
                                    let range_end = (*right_value * max_idx).round() as usize;
                                    
                                    let current = *current_index;
                                    
                                    // If we are already at or past the end of the range, loop to start
                                    // Otherwise just go to next frame
                                    let next = if current >= range_end {
                                        range_start
                                    } else {
                                        current + 1
                                    };
                                    
                                    // Ensure we don't go out of bounds of the actual frames
                                    let valid_next = next.min(frame_count.saturating_sub(1));
                                    current_index.set(valid_next);
                                }
                            })
                        }}>
                        <Icon icon_id={IconId::LucideSkipForward} width={"20"} height={"20"} />
                    </button>
                </div>

                if let Some(settings) = &props.settings {
                    <div class="settings-info">
                        <div class="settings-row">
                            <span class="settings-label">{"FPS:"}</span>
                            <span class="settings-value">{settings.fps}</span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"Font Ratio:"}</span>
                            <span class="settings-value">{format!("{:.2}", settings.font_ratio)}</span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"Luminance:"}</span>
                            <span class="settings-value">{settings.luminance}</span>
                        </div>
                        <div class="settings-row">
                            <span class="settings-label">{"Columns:"}</span>
                            <span class="settings-value">{settings.columns}</span>
                        </div>
                    </div>
                }
            </div>
        </div>
    }
}
