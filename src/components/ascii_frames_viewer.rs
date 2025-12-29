use yew::prelude::*;
use wasm_bindgen::prelude::*;
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
"#)]
extern "C" {
    #[wasm_bindgen(js_name = tauriInvoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FrameFile {
    path: String,
    name: String,
    index: u32,
}

#[derive(Properties, PartialEq, Clone)]
pub struct AsciiFramesViewerProps {
    pub directory_path: String,
    #[prop_or(24)]
    pub fps: u32,
}

#[function_component(AsciiFramesViewer)]
pub fn ascii_frames_viewer(props: &AsciiFramesViewerProps) -> Html {
    let frames = use_state(|| Vec::<String>::new());
    let current_index = use_state(|| 0usize);
    let is_playing = use_state(|| false);
    let is_loading = use_state(|| true);
    let error_message = use_state(|| None::<String>);
    // Store timeout handle to allow cancellation
    let timeout_handle: Rc<RefCell<Option<Timeout>>> = use_mut_ref(|| None);

    // Load frames when directory_path changes
    {
        let directory_path = props.directory_path.clone();
        let frames = frames.clone();
        let is_loading = is_loading.clone();
        let error_message = error_message.clone();
        let current_index = current_index.clone();
        let timeout_handle_clone = timeout_handle.clone();
        let is_playing = is_playing.clone();

        use_effect_with(directory_path.clone(), move |_| {
            is_loading.set(true);
            error_message.set(None);
            frames.set(Vec::new());
            current_index.set(0);
            is_playing.set(false); // Stop playback when loading new frames
            
            // Cancel any pending timeout
            timeout_handle_clone.borrow_mut().take();

            wasm_bindgen_futures::spawn_local(async move {
                // Get list of frame files
                let args = serde_wasm_bindgen::to_value(&json!({ "directoryPath": directory_path })).unwrap();
                match tauri_invoke("get_frame_files", args).await {
                    result => {
                        match serde_wasm_bindgen::from_value::<Vec<FrameFile>>(result) {
                            Ok(frame_files) => {
                                // Load all frame contents
                                let mut loaded_frames = Vec::new();
                                for frame_file in frame_files {
                                    let args = serde_wasm_bindgen::to_value(&json!({ "filePath": frame_file.path })).unwrap();
                                    match tauri_invoke("read_frame_file", args).await {
                                        result => {
                                            match serde_wasm_bindgen::from_value::<String>(result) {
                                                Ok(content) => {
                                                    loaded_frames.push(content);
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
                            }
                            Err(e) => {
                                error_message.set(Some(format!("Failed to list frames: {:?}", e)));
                                is_loading.set(false);
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
        let fps = props.fps;

        use_effect_with((*is_playing, *current_index, frames.len(), fps), move |(playing, _current, frame_count, fps)| {
            let playing = *playing;
            let frame_count = *frame_count;
            let fps = *fps;
            
            // Cancel any pending timeout first
            timeout_handle.borrow_mut().take();

            // Only schedule next frame if playing and we have frames
            if playing && frame_count > 0 {
                let interval_ms = (1000.0 / fps as f64).max(1.0) as u32;
                let current_index_clone = current_index.clone();
                let frame_count_clone = frame_count;
                
                // Schedule the next frame advance
                let handle = Timeout::new(interval_ms, move || {
                    let current = *current_index_clone;
                    let next = if current + 1 >= frame_count_clone {
                        0 // Loop back to start
                    } else {
                        current + 1
                    };
                    current_index_clone.set(next);
                    // After setting, Yew will re-render, which will trigger this effect again
                    // to schedule the next frame (because current_index is in dependencies)
                });
                
                *timeout_handle.borrow_mut() = Some(handle);
            }

            let timeout_handle_cleanup = timeout_handle.clone();
            move || {
                // Cleanup: cancel pending timeout on unmount or dependency change
                timeout_handle_cleanup.borrow_mut().take();
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

    let play_icon = if *is_playing { IconId::LucidePause } else { IconId::LucidePlay };
    let frame_count = frames.len();
    let current_frame = (*current_index).min(frame_count.saturating_sub(1));

    let format_frame_info = |idx: usize, total: usize| -> String {
        format!("Frame {} / {}", idx + 1, total)
    };

    html! {
        <div class="ascii-frames-viewer">
            <div class="frames-display">
                if *is_loading {
                    <div class="loading-frames">{"Loading frames..."}</div>
                } else if let Some(error) = &*error_message {
                    <div class="error-frames">{error}</div>
                } else if frames.is_empty() {
                    <div class="no-frames">{"No frames available"}</div>
                } else {
                    <pre class="ascii-frame-content">{
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
            </div>
        </div>
    }
}

