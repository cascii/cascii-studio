use cascii_core_view::{
    load_color_frames, load_text_frames, yield_to_event_loop, Frame, FrameColors,
    FrameDataProvider, FrameFile,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

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

pub struct TauriFrameProvider;

impl FrameDataProvider for TauriFrameProvider {
    fn get_frame_files(
        &self,
        directory: &str,
    ) -> impl std::future::Future<Output = Result<Vec<FrameFile>, String>> {
        let directory = directory.to_string();
        async move {
            let args =
                serde_wasm_bindgen::to_value(&json!({ "directoryPath": directory })).unwrap();
            serde_wasm_bindgen::from_value::<Vec<FrameFile>>(
                tauri_invoke("get_frame_files", args).await,
            )
            .map_err(|error| format!("Failed to list frames: {:?}", error))
        }
    }

    fn read_frame_text(
        &self,
        path: &str,
    ) -> impl std::future::Future<Output = Result<String, String>> {
        let path = path.to_string();
        async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "filePath": path })).unwrap();
            serde_wasm_bindgen::from_value::<String>(tauri_invoke("read_frame_file", args).await)
                .map_err(|error| format!("Failed to read frame: {:?}", error))
        }
    }

    fn read_cframe_bytes(
        &self,
        path: &str,
    ) -> impl std::future::Future<Output = Result<Option<Vec<u8>>, String>> {
        let path = path.to_string();
        async move {
            let args = serde_wasm_bindgen::to_value(&json!({ "txtFilePath": path })).unwrap();
            serde_wasm_bindgen::from_value::<Option<Vec<u8>>>(
                tauri_invoke("read_cframe_file", args).await,
            )
            .map_err(|error| format!("Failed to read cframe file: {:?}", error))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameRenderMode {
    BwText,
    StyledText,
    ColorFrames,
}

impl FrameRenderMode {
    pub fn label(&self) -> &'static str {
        match self {
            FrameRenderMode::BwText => "BW",
            FrameRenderMode::StyledText => "TXT",
            FrameRenderMode::ColorFrames => "RGB",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            FrameRenderMode::BwText => "Black and white text",
            FrameRenderMode::StyledText => "Text with saved colors",
            FrameRenderMode::ColorFrames => "Colored frames",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameAssetMetadata {
    pub directory_path: String,
    #[serde(default)]
    pub fps: u32,
    #[serde(default)]
    pub frame_speed: u32,
    #[serde(default)]
    pub frame_count: i32,
    #[serde(default)]
    pub color_enabled: bool,
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    #[serde(default)]
    pub foreground_color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub has_text_frames: bool,
    #[serde(default)]
    pub has_color_frames: bool,
}

#[derive(Clone, Debug)]
pub struct PreloadedFrameBundle {
    pub directory_path: String,
    pub render_mode: FrameRenderMode,
    pub frames: Vec<Frame>,
    pub frame_files: Vec<FrameFile>,
    pub frame_colors: FrameColors,
    pub has_any_color: bool,
}

impl PartialEq for PreloadedFrameBundle {
    fn eq(&self, other: &Self) -> bool {
        self.directory_path == other.directory_path
            && self.render_mode == other.render_mode
            && self.frames.len() == other.frames.len()
            && self.frame_files.len() == other.frame_files.len()
            && self.frame_colors == other.frame_colors
            && self.has_any_color == other.has_any_color
    }
}

fn default_output_mode() -> String {
    "text-only".to_string()
}

pub fn resolve_frame_colors(metadata: &FrameAssetMetadata) -> FrameColors {
    FrameColors::from_strings(
        metadata.foreground_color.as_deref().unwrap_or("white"),
        metadata.background_color.as_deref().unwrap_or("black"),
    )
}

pub fn supported_frame_render_modes(metadata: &FrameAssetMetadata) -> Vec<FrameRenderMode> {
    let color_frames_available = metadata.color_enabled && metadata.has_color_frames;

    match metadata.output_mode.as_str() {
        "color-only" => {
            if color_frames_available {
                vec![FrameRenderMode::ColorFrames]
            } else {
                Vec::new()
            }
        }
        "text+color" => {
            let mut modes = vec![FrameRenderMode::BwText];
            if color_frames_available {
                modes.push(FrameRenderMode::ColorFrames);
            } else {
                modes.push(FrameRenderMode::StyledText);
            }
            modes
        }
        _ => {
            if color_frames_available {
                vec![FrameRenderMode::BwText, FrameRenderMode::ColorFrames]
            } else {
                vec![FrameRenderMode::BwText]
            }
        }
    }
}

pub fn default_frame_render_mode(metadata: &FrameAssetMetadata) -> Option<FrameRenderMode> {
    let supported_modes = supported_frame_render_modes(metadata);
    if supported_modes.contains(&FrameRenderMode::BwText) {
        Some(FrameRenderMode::BwText)
    } else if supported_modes.contains(&FrameRenderMode::StyledText) {
        Some(FrameRenderMode::StyledText)
    } else {
        supported_modes.into_iter().next()
    }
}

pub fn next_frame_render_mode(
    metadata: &FrameAssetMetadata,
    current_mode: &FrameRenderMode,
) -> Option<FrameRenderMode> {
    let supported_modes = supported_frame_render_modes(metadata);
    if supported_modes.is_empty() {
        return None;
    }

    let current_index = supported_modes
        .iter()
        .position(|mode| mode == current_mode)
        .unwrap_or(0);
    let next_index = (current_index + 1) % supported_modes.len();
    supported_modes.get(next_index).cloned()
}

pub async fn preload_frame_bundle(
    metadata: &FrameAssetMetadata,
    render_mode: FrameRenderMode,
) -> Result<PreloadedFrameBundle, String> {
    let provider = TauriFrameProvider;
    let (loaded_frames, frame_files) =
        load_text_frames(&provider, &metadata.directory_path).await?;
    let frames_ref = Rc::new(RefCell::new(loaded_frames));
    let has_any_color = Rc::new(RefCell::new(false));

    if matches!(render_mode, FrameRenderMode::ColorFrames) {
        let frames_ref_for_color = frames_ref.clone();
        let has_any_color_for_color = has_any_color.clone();

        load_color_frames(
            &provider,
            &frame_files,
            move |index, _total, cframe| {
                if let Some(cframe) = cframe {
                    let mut frames = frames_ref_for_color.borrow_mut();
                    if let Some(frame) = frames.get_mut(index) {
                        frame.cframe = Some(cframe);
                        *has_any_color_for_color.borrow_mut() = true;
                    }
                }
            },
            || async {
                yield_to_event_loop().await;
            },
        )
        .await?;

        if !*has_any_color.borrow() {
            return Err("No color frame data available for this clip.".to_string());
        }
    }

    let frames = frames_ref.borrow().clone();
    let has_any_color = *has_any_color.borrow();

    Ok(PreloadedFrameBundle {
        directory_path: metadata.directory_path.clone(),
        render_mode,
        frames,
        frame_files,
        frame_colors: resolve_frame_colors(metadata),
        has_any_color,
    })
}
