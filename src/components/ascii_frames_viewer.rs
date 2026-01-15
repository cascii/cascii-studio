use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use serde::{Deserialize, Serialize};
use serde_json::json;
use yew_icons::{Icon, IconId};
use gloo_timers::callback::Timeout;
use std::cell::RefCell;
use std::rc::Rc;

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
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_name = observeResize)]
    fn observe_resize(element: &web_sys::Element, callback: &Closure<dyn Fn(f64, f64)>) -> JsValue;

    #[wasm_bindgen(js_name = disconnectObserver)]
    fn disconnect_observer(observer: &JsValue);
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
}

#[function_component(AsciiFramesViewer)]
pub fn ascii_frames_viewer(props: &AsciiFramesViewerProps) -> Html {
    let frames = use_state(|| Vec::<String>::new());
    let current_index = use_state(|| 0usize);
    let is_playing = use_state(|| false);
    let is_loading = use_state(|| true);
    let error_message = use_state(|| None::<String>);
    let loading_progress = use_state(|| (0, 0)); // (loaded, total)
    // Store timeout handle to allow cancellation
    let timeout_handle: Rc<RefCell<Option<Timeout>>> = use_mut_ref(|| None);
    let on_loading_changed = props.on_loading_changed.clone();

    // Auto-sizing state
    let container_ref = use_node_ref();
    let calculated_font_size = use_state(|| 10.0f64); // Default font size in px
    let container_size = use_state(|| (0.0f64, 0.0f64)); // (width, height) from ResizeObserver

    // Load frames when directory_path changes
    {
        let directory_path = props.directory_path.clone();
        let frames = frames.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let current_index = current_index.clone();
        let timeout_handle_clone = timeout_handle.clone();
        let is_playing = is_playing.clone();

        let loading_progress_clone = loading_progress.clone();
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

            // Cancel any pending timeout
            timeout_handle_clone.borrow_mut().take();

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

    // Animation loop - schedule next frame when playing
    {
        let current_index = current_index.clone();
        let is_playing = is_playing.clone();
        let frames = frames.clone();
        let timeout_handle = timeout_handle.clone();
        // Clone the values we need to avoid lifetime issues
        let frame_speed = props.frame_speed;
        let fps = props.fps;
        let selected_speed = props.selected_speed.clone();
        let settings = props.settings.clone();
        let loop_enabled = props.loop_enabled;

        // Use a simple effect that runs on every render when playing state changes
        // This is simpler than trying to track all the complex dependencies
        use_effect(move || {
            let playing = *is_playing;
            let frame_count = frames.len();

            // Cancel any pending timeout first
            timeout_handle.borrow_mut().take();

            // Only schedule next frame if playing and we have frames
            if playing && frame_count > 0 {
                let current_fps = match selected_speed {
                    SpeedSelection::Custom => frame_speed.unwrap_or(fps),
                    SpeedSelection::Base => settings.as_ref().map(|s| s.fps).unwrap_or(fps),
                };
                    let interval_ms = (1000.0 / current_fps as f64).max(1.0) as u32;
                    let current_index_clone = current_index.clone();
                    let is_playing_clone = is_playing.clone();
                    let frame_count_clone = frame_count;

                    // Schedule the next frame advance
                    let handle = Timeout::new(interval_ms, move || {
                        let current = *current_index_clone;
                        if current + 1 >= frame_count_clone {
                            if loop_enabled {
                                current_index_clone.set(0); // Loop back to start
                            } else {
                                // Stop at the end
                                is_playing_clone.set(false);
                            }
                        } else {
                            current_index_clone.set(current + 1);
                        }
                    });

                    *timeout_handle.borrow_mut() = Some(handle);
                }

                // Cleanup function
                let timeout_handle_cleanup = timeout_handle.clone();
                move || {
                    timeout_handle_cleanup.borrow_mut().take();
                }
            });
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

    // Toggle play/pause
    let on_toggle_play = {
        let is_playing = is_playing.clone();
        Callback::from(move |_| {
            is_playing.set(!*is_playing);
        })
    };

    // Seek to specific frame
    let on_seek = {
        let current_index = current_index.clone();
        let is_playing = is_playing.clone();
        let frames_len = frames.len();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let val = input.value_as_number();
                    if val.is_finite() {
                        let idx = val as usize;
                        if idx < frames_len {
                            // Pause when seeking
                            is_playing.set(false);
                            current_index.set(idx);
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

    let play_icon = if *is_playing { IconId::LucidePause } else { IconId::LucidePlay };
    let frame_count = frames.len();
    let current_frame = (*current_index).min(frame_count.saturating_sub(1));

    let format_frame_info = |idx: usize, total: usize| -> String {
        format!("Frame {} / {}", idx + 1, total)
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
                        {format_frame_info(current_frame, frame_count)}
                    </div>
                }
            </div>

            <div class="controls">
                <div class="control-row">
                    <input
                        class="progress"
                        type="range"
                        min="0"
                        max={(frame_count.saturating_sub(1)).to_string()}
                        value={current_frame.to_string()}
                        oninput={on_seek}
                        title="Seek frame"
                        disabled={frame_count == 0}
                    />
                    <button
                        class="ctrl-btn"
                        type="button"
                        onclick={on_toggle_play}
                        title="Play/Pause"
                        disabled={frame_count == 0}
                    >
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row">
                    <label style="font-size: 0.875rem;">{"Speed:"}</label>
                    <input
                        type="number"
                        class={if props.selected_speed == SpeedSelection::Custom {"setting-input speed-input selected"} else {"setting-input speed-input"}}
                        style="width: 68px;"
                        value={props.frame_speed.unwrap_or(props.fps).to_string()}
                        min="1"
                        oninput={on_speed_change}
                        onclick={on_select_custom}
                        title="Frame playback speed (FPS)"
                    />
                    <input
                        type="number"
                        class={if props.selected_speed == SpeedSelection::Base {"setting-input speed-input selected no-spinner"} else {"setting-input speed-input no-spinner"}}
                        style="width: 68px;"
                        value={props.settings.as_ref().map(|s| s.fps).unwrap_or(props.fps).to_string()}
                        readonly=true
                        onclick={on_select_base}
                        title="Default Speed"
                    />
                    <button
                        type="button"
                        class={if props.loop_enabled {"ctrl-btn loop-btn active"} else {"ctrl-btn loop-btn"}}
                        onclick={{
                            let on_loop_change = props.on_loop_change.clone();
                            let loop_enabled = props.loop_enabled;
                            Callback::from(move |_| {
                                if let Some(cb) = &on_loop_change {
                                    cb.emit(!loop_enabled);
                                }
                            })
                        }}
                        title={if props.loop_enabled {"Loop enabled"} else {"Loop disabled"}}
                    >
                        <Icon icon_id={IconId::LucideRepeat} width={"16"} height={"16"} />
                    </button>
                    <div style="flex: 1;"></div>
                    <button
                        type="button"
                        class="ctrl-btn"
                        onclick={{
                            let current_index = current_index.clone();
                            let frames = frames.clone();
                            let is_playing = is_playing.clone();
                            Callback::from(move |_| {
                                // Pause if playing
                                if *is_playing {
                                    is_playing.set(false);
                                }
                                // Advance one frame
                                let frame_count = frames.len();
                                if frame_count > 0 {
                                    let current = *current_index;
                                    let next = if current + 1 >= frame_count {
                                        0
                                    } else {
                                        current + 1
                                    };
                                    current_index.set(next);
                                }
                            })
                        }}
                        title="Step forward one frame"
                    >
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
