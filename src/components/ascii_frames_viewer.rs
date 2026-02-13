use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use serde::{Deserialize, Serialize};
use serde_json::json;
use yew_icons::{Icon, IconId};
use std::collections::VecDeque;
use std::cell::RefCell;
use std::rc::Rc;
use gloo_timers::callback::Interval;

use cascii_core_view::{
    draw_frame_from_cache, load_color_frames, load_text_frames, render_to_offscreen_canvas,
    yield_to_event_loop, FontSizing, Frame, FrameCanvasCache, FrameDataProvider, FrameFile,
    LoadResult, LoadingPhase, RenderConfig,
};

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

async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        } else {
            let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

const BW_PLAYBACK_BACKGROUND_SLEEP_MS: i32 = 12;
const COLOR_LOADING_PROGRESS_TICK_MS: u32 = 500;

struct TauriFrameProvider;

impl FrameDataProvider for TauriFrameProvider {
    fn get_frame_files(&self, directory: &str) -> impl std::future::Future<Output = LoadResult<Vec<FrameFile>>> {
        let dir = directory.to_string();
        async move {
            let args =
                serde_wasm_bindgen::to_value(&json!({ "directoryPath": dir })).unwrap();
            serde_wasm_bindgen::from_value::<Vec<FrameFile>>(
                tauri_invoke("get_frame_files", args).await,
            )
            .map_err(|e| format!("Failed to list frames: {:?}", e))
        }
    }

    fn read_frame_text(&self, path: &str) -> impl std::future::Future<Output = LoadResult<String>> {
        let path = path.to_string();
        async move {
            let args =
                serde_wasm_bindgen::to_value(&json!({ "filePath": path })).unwrap();
            serde_wasm_bindgen::from_value::<String>(
                tauri_invoke("read_frame_file", args).await,
            )
            .map_err(|e| format!("Failed to read frame: {:?}", e))
        }
    }

    fn read_cframe_bytes(&self, txt_path: &str) -> impl std::future::Future<Output = LoadResult<Option<Vec<u8>>>> {
        let path = txt_path.to_string();
        async move {
            let args =
                serde_wasm_bindgen::to_value(&json!({ "txtFilePath": path })).unwrap();
            serde_wasm_bindgen::from_value::<Option<Vec<u8>>>(
                tauri_invoke("read_cframe_file", args).await,
            )
            .map_err(|e| format!("Failed to read cframe file: {:?}", e))
        }
    }
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
    // Frame storage - use RefCell to avoid re-renders during color loading
    let frames_ref: Rc<RefCell<Vec<Frame>>> = use_mut_ref(Vec::new);
    let frame_count = use_state(|| 0usize);
    let loading_phase = use_state(|| LoadingPhase::Idle);
    let error_message = use_state(|| None::<String>);
    // Color progress: RefCell so color loading doesn't trigger re-renders.
    // Progress display piggybacks on animation re-renders instead.
    let color_progress: Rc<RefCell<(usize, usize)>> = use_mut_ref(|| (0usize, 0usize));

    // Offscreen canvas cache: pre-rendered colored frames for fast drawImage playback
    let frame_canvas_cache: Rc<RefCell<FrameCanvasCache>> = use_mut_ref(FrameCanvasCache::default);
    let color_cache_queue: Rc<RefCell<VecDeque<usize>>> = use_mut_ref(VecDeque::new);
    let color_loaded_flags: Rc<RefCell<Vec<bool>>> = use_mut_ref(Vec::new);
    let has_any_color = use_state(|| false);
    let has_any_color_flag: Rc<RefCell<bool>> = use_mut_ref(|| false);
    let color_cache_refresh = use_state(|| 0u64);
    let color_cache_worker_id: Rc<RefCell<u64>> = use_mut_ref(|| 0u64);
    let is_playing_ref = use_mut_ref(|| false);
    let color_enabled_ref = use_mut_ref(|| false);
    let loading_phase_ref: Rc<RefCell<LoadingPhase>> = use_mut_ref(|| LoadingPhase::Idle);

    let current_index = use_state(|| 0usize);
    let current_index_ref = use_mut_ref(|| 0usize);
    let is_playing = use_state(|| false);
    // Dual range selector state (0..1)
    let left_value = use_state(|| 0.0f64);
    let right_value = use_state(|| 1.0f64);
    // Store interval handle for animation
    let interval_handle: Rc<RefCell<Option<Interval>>> = use_mut_ref(|| None);
    let on_loading_changed = props.on_loading_changed.clone();

    // Color display toggle
    let color_enabled = use_state(|| false);

    // Auto-sizing state
    let container_ref = use_node_ref();
    let content_ref = use_node_ref();
    let canvas_ref = use_node_ref();
    let calculated_font_size = use_state(|| 10.0f64);
    let container_size = use_state(|| (0.0f64, 0.0f64));

    // Sync ref when current_index state changes
    {
        let current_index_ref = current_index_ref.clone();
        use_effect_with(*current_index, move |idx| {
            *current_index_ref.borrow_mut() = *idx;
            || ()
        });
    }

    // Keep playback state in a ref for background workers.
    {
        let is_playing_ref = is_playing_ref.clone();
        use_effect_with(*is_playing, move |playing| {
            *is_playing_ref.borrow_mut() = *playing;
            || ()
        });
    }

    // Keep color toggle state in a ref for background workers.
    {
        let color_enabled_ref = color_enabled_ref.clone();
        use_effect_with(*color_enabled, move |enabled| {
            *color_enabled_ref.borrow_mut() = *enabled;
            || ()
        });
    }

    // Keep loading phase in a ref for background workers.
    {
        let loading_phase_ref = loading_phase_ref.clone();
        use_effect_with(*loading_phase, move |phase| {
            *loading_phase_ref.borrow_mut() = *phase;
            || ()
        });
    }

    // Periodic re-render tick during LoadingColors phase so the progress
    // display (which reads from a RefCell) stays up-to-date when paused.
    // Stop this while playing to reduce main-thread contention.
    let _color_loading_tick = use_state(|| 0u32);
    {
        let phase = *loading_phase;
        let playing = *is_playing;
        let tick = _color_loading_tick.clone();
        use_effect_with((phase, playing), move |(phase, playing)| {
            let handle: Rc<RefCell<Option<Interval>>> = Rc::new(RefCell::new(None));
            if *phase == LoadingPhase::LoadingColors && !*playing {
                let interval = Interval::new(COLOR_LOADING_PROGRESS_TICK_MS, move || {
                    tick.set((*tick).wrapping_add(1));
                });
                *handle.borrow_mut() = Some(interval);
            }
            let cleanup_handle = handle.clone();
            move || { cleanup_handle.borrow_mut().take(); }
        });
    }

    // Two-phase loading: text frames first (fast), then color data in background
    {
        let directory_path = props.directory_path.clone();
        let frames_ref = frames_ref.clone();
        let frame_count = frame_count.clone();
        let loading_phase = loading_phase.clone();
        let error_message = error_message.clone();
        let color_progress = color_progress.clone();
        let current_index = current_index.clone();
        let interval_handle = interval_handle.clone();
        let is_playing = is_playing.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let on_loading_changed = on_loading_changed.clone();
        let frame_canvas_cache = frame_canvas_cache.clone();
        let color_cache_queue = color_cache_queue.clone();
        let color_loaded_flags = color_loaded_flags.clone();
        let has_any_color = has_any_color.clone();
        let has_any_color_flag = has_any_color_flag.clone();
        let color_enabled_for_color = color_enabled.clone();
        let current_index_ref_for_color = current_index_ref.clone();
        let color_cache_refresh_for_color = color_cache_refresh.clone();
        let color_cache_worker_id = color_cache_worker_id.clone();
        let is_playing_ref = is_playing_ref.clone();
        let color_enabled_ref = color_enabled_ref.clone();

        use_effect_with(directory_path.clone(), move |_| {
            // Reset state
            frames_ref.borrow_mut().clear();
            frame_count.set(0);
            loading_phase.set(LoadingPhase::Idle);
            error_message.set(None);
            *color_progress.borrow_mut() = (0, 0);
            frame_canvas_cache.borrow_mut().clear();
            color_cache_queue.borrow_mut().clear();
            color_loaded_flags.borrow_mut().clear();
            has_any_color.set(false);
            *has_any_color_flag.borrow_mut() = false;
            let next_worker_id = color_cache_worker_id.borrow().wrapping_add(1);
            *color_cache_worker_id.borrow_mut() = next_worker_id;
            current_index.set(0);
            is_playing.set(false);
            left_value.set(0.0);
            right_value.set(1.0);
            interval_handle.borrow_mut().take();

            if let Some(callback) = &on_loading_changed {
                callback.emit(true);
            }

            if !directory_path.is_empty() {
                loading_phase.set(LoadingPhase::LoadingText);

                let on_loading_changed_async = on_loading_changed.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let provider = TauriFrameProvider;

                    // Phase 1: Load text frames (fast)
                    match load_text_frames(&provider, &directory_path).await {
                        Ok((loaded_frames, frame_files)) => {
                            let total = loaded_frames.len();
                            *frames_ref.borrow_mut() = loaded_frames;
                            *color_loaded_flags.borrow_mut() = vec![false; total];
                            frame_count.set(total);
                            *color_progress.borrow_mut() = (0, total);
                            frame_canvas_cache.borrow_mut().resize(total);
                            loading_phase.set(LoadingPhase::LoadingColors);

                            // Notify parent that loading is done (playback can start)
                            if let Some(callback) = &on_loading_changed_async {
                                callback.emit(false);
                            }

                            // Phase 2: Load color data in background (no re-renders)
                            let frames_for_color = frames_ref.clone();
                            let progress_for_color = color_progress.clone();
                            let _ = load_color_frames(
                                &provider,
                                &frame_files,
                                |i, _total, cf| {
                                    if let Some(cframe) = cf {
                                        let mut frames = frames_for_color.borrow_mut();
                                        if i < frames.len() {
                                            frames[i].cframe = Some(cframe);
                                        }
                                        {
                                            let mut loaded_flags = color_loaded_flags.borrow_mut();
                                            if i < loaded_flags.len() {
                                                loaded_flags[i] = true;
                                            }
                                        }
                                        color_cache_queue.borrow_mut().push_back(i);
                                        if !*has_any_color_flag.borrow() {
                                            *has_any_color_flag.borrow_mut() = true;
                                            has_any_color.set(true);
                                        }
                                        if *color_enabled_for_color
                                            && i == *current_index_ref_for_color.borrow()
                                        {
                                            color_cache_refresh_for_color
                                                .set((*color_cache_refresh_for_color).wrapping_add(1));
                                        }
                                    }
                                    *progress_for_color.borrow_mut() = (i + 1, total);
                                },
                                || {
                                    let is_playing_ref = is_playing_ref.clone();
                                    let color_enabled_ref = color_enabled_ref.clone();
                                    async move {
                                        // While B/W is actively playing, back off color loading cadence
                                        // to preserve animation smoothness.
                                        if *is_playing_ref.borrow() && !*color_enabled_ref.borrow() {
                                            sleep_ms(BW_PLAYBACK_BACKGROUND_SLEEP_MS).await;
                                        } else {
                                            yield_to_event_loop().await;
                                        }
                                    }
                                },
                            )
                            .await;
                            loading_phase.set(LoadingPhase::Complete);
                        }
                        Err(e) => {
                            error_message.set(Some(e));
                            loading_phase.set(LoadingPhase::Idle);
                            if let Some(callback) = &on_loading_changed_async {
                                callback.emit(false);
                            }
                        }
                    }
                });
            }

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
        let interval_handle = interval_handle.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let loop_enabled = props.loop_enabled;
        let playing = *is_playing;
        let total_frames = *frame_count;

        use_effect_with(
            (playing, current_fps, total_frames),
            move |_| {
                // Always clear existing interval first
                interval_handle.borrow_mut().take();

                if playing && total_frames > 0 {
                    let interval_ms = (1000.0 / current_fps as f64).max(1.0) as u32;
                    let current_index_clone = current_index.clone();
                    let current_index_ref_clone = current_index_ref.clone();
                    let is_playing_clone = is_playing_state.clone();
                    let interval_handle_clone = interval_handle.clone();
                    let left_value_clone = left_value.clone();
                    let right_value_clone = right_value.clone();

                    let interval = Interval::new(interval_ms, move || {
                        let max_idx = total_frames.saturating_sub(1) as f64;
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
        let frame_count = frame_count.clone();
        let seek_percentage = props.seek_percentage;

        use_effect_with(seek_percentage, move |seek_percentage| {
            if let Some(percentage) = seek_percentage {
                let total = *frame_count;
                if total > 0 {
                    let target_frame = ((total - 1) as f64 * percentage).round() as usize;
                    let clamped_frame = target_frame.min(total - 1);
                    current_index.set(clamped_frame);
                }
            }
        });
    }

    // Clamp current frame to range when range selection changes
    {
        let current_index = current_index.clone();
        let total_frames = *frame_count;
        let left_val = *left_value;
        let right_val = *right_value;

        use_effect_with((left_val, right_val, total_frames), move |_| {
            if total_frames > 0 {
                let max_idx = total_frames.saturating_sub(1) as f64;
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

    // Auto-size font to fit container using FontSizing::calculate()
    {
        let frames_ref = frames_ref.clone();
        let calculated_font_size = calculated_font_size.clone();
        let container_width = container_size.0;
        let container_height = container_size.1;
        let total_frames = *frame_count;
        let phase = *loading_phase;

        use_effect_with((total_frames, phase, container_width as i32, container_height as i32), move |_| {
            let frames = frames_ref.borrow();
            if frames.is_empty() {
                return;
            }

            if let Some(first_frame) = frames.first() {
                let (cols, rows) = first_frame.dimensions();

                if rows == 0 || cols == 0 {
                    return;
                }

                let optimal_font_size =
                    FontSizing::calculate(cols, rows, container_width, container_height);
                calculated_font_size.set(optimal_font_size);
            }
        });
    }

    // Keep the color cache warm in background without hurting B/W playback.
    {
        let frames_ref = frames_ref.clone();
        let frame_canvas_cache = frame_canvas_cache.clone();
        let color_cache_queue = color_cache_queue.clone();
        let color_loaded_flags = color_loaded_flags.clone();
        let color_cache_refresh = color_cache_refresh.clone();
        let color_cache_worker_id = color_cache_worker_id.clone();
        let current_index_ref = current_index_ref.clone();
        let is_playing_ref = is_playing_ref.clone();
        let color_enabled_ref = color_enabled_ref.clone();
        let loading_phase_ref = loading_phase_ref.clone();
        let total_frames = *frame_count;
        let has_any_color_val = *has_any_color;
        let font_size = *calculated_font_size;
        let font_size_key = (font_size * 100.0) as i32;

        use_effect_with((total_frames, has_any_color_val, font_size_key), move |_| {
            if total_frames == 0 || !has_any_color_val {
                return;
            }

            let next_worker_id = color_cache_worker_id.borrow().wrapping_add(1);
            *color_cache_worker_id.borrow_mut() = next_worker_id;

            {
                let mut cache = frame_canvas_cache.borrow_mut();
                cache.resize(total_frames);
                cache.invalidate_for_font_size_key(font_size_key);
            }

            // Rebuild queue from already-loaded color frames.
            {
                let loaded_flags = color_loaded_flags.borrow();
                let mut queue = color_cache_queue.borrow_mut();
                queue.clear();
                for (idx, loaded) in loaded_flags.iter().enumerate() {
                    if *loaded {
                        queue.push_back(idx);
                    }
                }
            }

            let frames_for_cache = frames_ref.clone();
            let cache_for_cache = frame_canvas_cache.clone();
            let queue_for_cache = color_cache_queue.clone();
            let refresh_for_cache = color_cache_refresh.clone();
            let worker_id_ref = color_cache_worker_id.clone();
            let current_index_ref = current_index_ref.clone();
            let is_playing_ref = is_playing_ref.clone();
            let color_enabled_ref = color_enabled_ref.clone();
            let loading_phase_ref = loading_phase_ref.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let config = RenderConfig::new(font_size);

                loop {
                    if *worker_id_ref.borrow() != next_worker_id {
                        return;
                    }

                    // While B/W playback is active, prioritize smooth animation and
                    // defer expensive canvas prerendering.
                    if *is_playing_ref.borrow() && !*color_enabled_ref.borrow() {
                        sleep_ms(BW_PLAYBACK_BACKGROUND_SLEEP_MS).await;
                        continue;
                    }

                    let next_frame = { queue_for_cache.borrow_mut().pop_front() };
                    let Some(i) = next_frame else {
                        if *loading_phase_ref.borrow() == LoadingPhase::Complete {
                            refresh_for_cache.set((*refresh_for_cache).wrapping_add(1));
                            return;
                        }
                        sleep_ms(BW_PLAYBACK_BACKGROUND_SLEEP_MS).await;
                        continue;
                    };

                    if cache_for_cache.borrow().has(i) {
                        continue;
                    }

                    let offscreen = {
                        let frames = frames_for_cache.borrow();
                        frames
                            .get(i)
                            .and_then(|f| f.cframe.as_ref())
                            .and_then(|cframe| render_to_offscreen_canvas(cframe, &config).ok())
                    };

                    if let Some(canvas) = offscreen {
                        cache_for_cache.borrow_mut().store(i, canvas);
                        if i == *current_index_ref.borrow() {
                            refresh_for_cache.set((*refresh_for_cache).wrapping_add(1));
                        }
                    }

                    yield_to_event_loop().await;
                }
            });
        });
    }

    // Update frame content: draw pre-rendered color canvas when available,
    // otherwise fall back to plain text to keep playback smooth.
    {
        let content_ref = content_ref.clone();
        let canvas_ref = canvas_ref.clone();
        let frames_ref = frames_ref.clone();
        let frame_canvas_cache = frame_canvas_cache.clone();
        let color_enabled_val = *color_enabled;
        let total_frames = *frame_count;
        let current_frame_idx = (*current_index).min(total_frames.saturating_sub(1));
        let font_size_key = (*calculated_font_size * 100.0) as i32;
        let cache_refresh_tick = *color_cache_refresh;

        use_effect_with((current_frame_idx, color_enabled_val, total_frames, font_size_key, cache_refresh_tick), move |_| {
            let frames = frames_ref.borrow();
            if let Some(frame) = frames.get(current_frame_idx) {
                if color_enabled_val {
                    if frame.cframe.is_some() {
                        if let Some(canvas) = canvas_ref.cast::<web_sys::HtmlCanvasElement>() {
                            {
                                let mut cache = frame_canvas_cache.borrow_mut();
                                cache.resize(total_frames);
                                cache.invalidate_for_font_size_key(font_size_key);
                            }

                            let drawn = {
                                let cache = frame_canvas_cache.borrow();
                                draw_frame_from_cache(&canvas, &cache, current_frame_idx)
                                    .unwrap_or(false)
                            };
                            if drawn {
                                return;
                            }
                        }
                    }
                }

                if let Some(element) = content_ref.cast::<web_sys::HtmlElement>() {
                    element.set_text_content(Some(&frame.content));
                }
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

    // Seek within the selected range (slider value is 0-1 within range)
    let on_seek = {
        let current_index = current_index.clone();
        let is_playing = is_playing.clone();
        let left_value = left_value.clone();
        let right_value = right_value.clone();
        let frame_count = frame_count.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                    let slider_val = input.value_as_number();
                    if slider_val.is_finite() {
                        let total_frames = *frame_count;
                        if total_frames > 0 {
                            let max_idx = total_frames.saturating_sub(1) as f64;
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
    let total_frames = *frame_count;
    let current_frame = (*current_index).min(total_frames.saturating_sub(1));

    // Calculate range bounds in frame indices
    let max_idx = total_frames.saturating_sub(1) as f64;
    let range_start_frame = (*left_value * max_idx).round() as usize;
    let range_end_frame = (*right_value * max_idx).round() as usize;
    let range_frame_count = range_end_frame.saturating_sub(range_start_frame) + 1;

    // Progress within the range (0.0 to 1.0)
    let progress_in_range = if range_frame_count > 1 {
        ((current_frame as f64) - (range_start_frame as f64)) / ((range_end_frame - range_start_frame) as f64)
    } else {
        0.0
    }.clamp(0.0, 1.0);

    // Position within the subset (1-based)
    let position_in_subset = if current_frame >= range_start_frame {
        current_frame - range_start_frame + 1
    } else {
        1
    };

    // Loading message
    let loading_message = "Loading frames...".to_string();

    // Color loading progress (read from RefCell - piggybacks on periodic tick re-renders)
    let (color_loaded, color_total) = *color_progress.borrow();
    let color_loading_pct = if *loading_phase == LoadingPhase::LoadingColors && color_total > 0 {
        let pct = (color_loaded as f32 / color_total as f32 * 100.0) as u8;
        Some(format!("{}%", pct))
    } else {
        None
    };

    let colors_loading = *loading_phase == LoadingPhase::LoadingColors;

    let font_size_style = format!("font-size: {:.2}px;", *calculated_font_size);
    let _color_cache_refresh_tick = *color_cache_refresh;

    // Track whether any color data has been loaded.
    let color_available = *has_any_color;

    let has_colors = {
        if !*color_enabled || !color_available {
            false
        } else {
            let frame_has_color = {
                let frames = frames_ref.borrow();
                frames
                    .get(current_frame)
                    .map(|f| f.has_color())
                    .unwrap_or(false)
            };
            let frame_cached = {
                let cache = frame_canvas_cache.borrow();
                cache.has(current_frame)
            };
            frame_has_color && frame_cached
        }
    };

    html! {
        <div id="ascii-frames-viewer" class="ascii-frames-viewer">
            <div id="frames-display" class="frames-display" ref={container_ref}>
                if *loading_phase == LoadingPhase::LoadingText {
                    <div id="frames-loading-msg" class="loading-frames">{loading_message.clone()}</div>
                } else if let Some(error) = &*error_message {
                    <div id="frames-error-msg" class="error-frames">{error}</div>
                } else if total_frames == 0 {
                    <div id="frames-empty-msg" class="no-frames">{"No frames available"}</div>
                } else {
                    if has_colors {
                        <canvas id="frames-canvas" ref={canvas_ref.clone()} class="ascii-frame-canvas"></canvas>
                    } else {
                        <pre id="frames-text-content" class="ascii-frame-content" style={font_size_style.clone()} ref={content_ref.clone()}></pre>
                    }
                }
            </div>

            <div class="controls" id="frames-controls">
                <div class="control-row" id="frames-progress">
                    <input id="frames-progress-bar" class="progress" type="range" min="0" max="1" step="0.001" value={progress_in_range.to_string()} oninput={on_seek} title="Seek frame" disabled={total_frames == 0} />
                    <button id="frames-play-btn" class="ctrl-btn" type="button" onclick={on_toggle_play} title="Play/Pause" disabled={total_frames == 0}>
                        <Icon icon_id={play_icon} width={"20"} height={"20"} />
                    </button>
                </div>

                <div class="control-row" id="cut-controls">
                    <div id="frames-range-selector" class="range-selector">
                        <div id="frames-range-track" class="range-selector-track"></div>
                        <input id="frames-range-left" class="range-selector-input range-left" type="range" min="0" max="1" step="0.001" value={left_value.to_string()} oninput={on_left_range_input.clone()} title="Range start" disabled={total_frames == 0} />
                        <input id="frames-range-right" class="range-selector-input range-right" type="range" min="0" max="1" step="0.001" value={right_value.to_string()} oninput={on_right_range_input.clone()} title="Range end" disabled={total_frames == 0} />
                    </div>
                    <button id="cut-frames-button" class="ctrl-btn" type="button" disabled={props.is_cutting || props.on_cut_frames.is_none() || total_frames == 0} title="Cut frame segment" onclick={{
                            let left_value = left_value.clone();
                            let right_value = right_value.clone();
                            let on_cut_frames = props.on_cut_frames.clone();
                            let total_frames = total_frames;
                            Callback::from(move |_| {
                                if let Some(on_cut) = &on_cut_frames {
                                    if total_frames > 0 {
                                        let max_idx = total_frames.saturating_sub(1) as f64;
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

                <div class="control-row" id="frames-speed-controls">
                    <label style="font-size: 0.875rem;">{"Speed:"}</label>
                    <input id="frames-speed-custom-input" type="number" class={if props.selected_speed == SpeedSelection::Custom {"setting-input speed-input selected"} else {"setting-input speed-input"}} style="width: 68px;" value={props.frame_speed.unwrap_or(props.fps).to_string()} min="1" oninput={on_speed_change} onclick={on_select_custom} title="Frame playback speed (FPS)" />
                    <input id="frames-speed-base-input" type="number" class={if props.selected_speed == SpeedSelection::Base {"setting-input speed-input selected no-spinner"} else {"setting-input speed-input no-spinner"}} style="width: 68px;" value={props.settings.as_ref().map(|s| s.fps).unwrap_or(props.fps).to_string()} readonly=true onclick={on_select_base} title="Default Speed" />
                    <button id="frames-loop-btn" type="button" class={if props.loop_enabled {"ctrl-btn loop-btn active"} else {"ctrl-btn loop-btn"}} title={if props.loop_enabled {"Loop enabled"} else {"Loop disabled"}}
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
                    <button id="frames-color-btn" type="button" class={if *color_enabled && color_available {"ctrl-btn color-btn active"} else if !color_available {"ctrl-btn color-btn disabled"} else {"ctrl-btn color-btn"}}
                        title={if colors_loading {"Loading colors..."} else if !color_available {"No color data available"} else if *color_enabled {"Color enabled"} else {"Color disabled"}}
                        disabled={!color_available}
                        onclick={{
                            let color_enabled = color_enabled.clone();
                            Callback::from(move |_| {
                                color_enabled.set(!*color_enabled);
                            })
                        }}>
                        if colors_loading {
                            if let Some(ref msg) = color_loading_pct {
                                <span id="frames-color-loading-pct" class="color-loading-pct">{msg.clone()}</span>
                            } else {
                                <Icon icon_id={IconId::LucideMoreHorizontal} width={"16"} height={"16"} />
                            }
                        } else {
                            <Icon icon_id={IconId::LucideBrush} width={"16"} height={"16"} />
                        }
                    </button>
                    <span id="frames-progress-label" class="frame-progress-label">{format!("{}/{}", position_in_subset, range_frame_count)}</span>
                    <div style="flex: 1;"></div>
                    <button type="button" title="Step backward one frame" id="move-frame-backward" class="ctrl-btn" onclick={{
                            let current_index = current_index.clone();
                            let frame_count = frame_count.clone();
                            let is_playing = is_playing.clone();
                            let left_value = left_value.clone();
                            let right_value = right_value.clone();
                            Callback::from(move |_| {
                                // Pause if playing
                                if *is_playing {
                                    is_playing.set(false);
                                }
                                // Go back one frame
                                let total_frames = *frame_count;
                                if total_frames > 0 {
                                    // Calculate range boundaries
                                    let max_idx = total_frames.saturating_sub(1) as f64;
                                    let range_start = (*left_value * max_idx).round() as usize;
                                    let range_end = (*right_value * max_idx).round() as usize;

                                    let current = *current_index;

                                    // If we are at or before the start of the range, loop to end
                                    // Otherwise just go to previous frame
                                    let prev = if current <= range_start {
                                        range_end
                                    } else {
                                        current - 1
                                    };

                                    // Ensure we don't go out of bounds of the actual frames
                                    let valid_prev = prev.min(total_frames.saturating_sub(1));
                                    current_index.set(valid_prev);
                                }
                            })
                        }}>
                        <span style="display: inline-flex; transform: scaleX(-1);">
                            <Icon icon_id={IconId::LucideSkipForward} width={"20"} height={"20"} />
                        </span>
                    </button>
                    <button type="button" title="Step forward one frame" id="move-frame-forward" class="ctrl-btn" onclick={{
                            let current_index = current_index.clone();
                            let frame_count = frame_count.clone();
                            let is_playing = is_playing.clone();
                            let left_value = left_value.clone();
                            let right_value = right_value.clone();
                            Callback::from(move |_| {
                                // Pause if playing
                                if *is_playing {
                                    is_playing.set(false);
                                }
                                // Advance one frame
                                let total_frames = *frame_count;
                                if total_frames > 0 {
                                    // Calculate range boundaries
                                    let max_idx = total_frames.saturating_sub(1) as f64;
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
                                    let valid_next = next.min(total_frames.saturating_sub(1));
                                    current_index.set(valid_next);
                                }
                            })
                        }}>
                        <Icon icon_id={IconId::LucideSkipForward} width={"20"} height={"20"} />
                    </button>
                </div>

                if let Some(settings) = &props.settings {
                    <div id="frames-settings-info" class="settings-info">
                        <div id="frames-settings-fps" class="settings-row">
                            <span id="frames-settings-fps-label" class="settings-label">{"FPS:"}</span>
                            <span id="frames-settings-fps-value" class="settings-value">{settings.fps}</span>
                        </div>
                        <div id="frames-settings-font-ratio" class="settings-row">
                            <span id="frames-settings-font-ratio-label" class="settings-label">{"Font Ratio:"}</span>
                            <span id="frames-settings-font-ratio-value" class="settings-value">{format!("{:.2}", settings.font_ratio)}</span>
                        </div>
                        <div id="frames-settings-luminance" class="settings-row">
                            <span id="frames-settings-luminance-label" class="settings-label">{"Luminance:"}</span>
                            <span id="frames-settings-luminance-value" class="settings-value">{settings.luminance}</span>
                        </div>
                        <div id="frames-settings-columns" class="settings-row">
                            <span id="frames-settings-columns-label" class="settings-label">{"Columns:"}</span>
                            <span id="frames-settings-columns-value" class="settings-value">{settings.columns}</span>
                        </div>
                    </div>
                }
            </div>
        </div>
    }
}
