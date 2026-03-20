use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Image,
    Animation,
}

impl ProjectType {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            ProjectType::Image => "image",
            ProjectType::Animation => "animation",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "animation" => ProjectType::Animation,
            _ => ProjectType::Image,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Image,
    Video,
}

impl SourceType {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            SourceType::Image => "image",
            SourceType::Video => "video",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "video" => SourceType::Video,
            _ => SourceType::Image,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub project_name: String,
    pub project_type: ProjectType,
    pub project_path: String,
    pub size: i64,
    pub frames: i32,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceContent {
    pub id: String,
    pub content_type: SourceType,
    pub project_id: String,
    pub date_added: DateTime<Utc>,
    pub size: i64,
    pub file_path: String,
    #[serde(default)]
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSettings {
    pub luminance: u8,
    pub font_ratio: f32,
    pub columns: u32,
    pub fps: u32,
    pub frame_speed: u32,
    #[serde(default)]
    pub color: bool,
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    #[serde(default)]
    pub foreground_color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiConversion {
    pub id: String,
    pub folder_name: String,
    pub folder_path: String,
    pub frame_count: i32,
    pub source_file_id: String,
    pub project_id: String,
    pub settings: ConversionSettings,
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCut {
    pub id: String,
    pub project_id: String,
    pub source_file_id: String,
    pub file_path: String,
    pub date_added: DateTime<Utc>,
    pub size: i64,
    pub custom_name: Option<String>,
    pub start_time: f64,
    pub end_time: f64,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioExtraction {
    pub id: String,
    pub folder_name: String,
    pub folder_path: String,
    pub source_file_id: String,
    pub project_id: String,
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,
    pub audio_track_beginning: f64,
    pub audio_track_end: f64,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewSettings {
    pub luminance: u8,
    pub font_ratio: f32,
    pub columns: u32,
    pub fps: u32,
    pub color: bool,
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    #[serde(default)]
    pub foreground_color: Option<String>,
    #[serde(default)]
    pub background_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    pub id: String,
    pub folder_name: String,
    pub folder_path: String,
    pub frame_count: i32,
    pub source_file_id: String,
    pub project_id: String,
    pub settings: PreviewSettings,
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineMediaType {
    Video,
    Frames,
    Frame,
}

impl TimelineMediaType {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            TimelineMediaType::Video => "video",
            TimelineMediaType::Frames => "frames",
            TimelineMediaType::Frame => "frame",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "frames" => TimelineMediaType::Frames,
            "frame" => TimelineMediaType::Frame,
            _ => TimelineMediaType::Video,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineResourceKind {
    Source,
    Cut,
    AsciiConversion,
    Preview,
}

impl TimelineResourceKind {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            TimelineResourceKind::Source => "source",
            TimelineResourceKind::Cut => "cut",
            TimelineResourceKind::AsciiConversion => "ascii_conversion",
            TimelineResourceKind::Preview => "preview",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "cut" => TimelineResourceKind::Cut,
            "ascii_conversion" => TimelineResourceKind::AsciiConversion,
            "preview" => TimelineResourceKind::Preview,
            _ => TimelineResourceKind::Source,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FrameRenderMode {
    BwText,
    StyledText,
    ColorFrames,
}

impl FrameRenderMode {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            FrameRenderMode::BwText => "bw_text",
            FrameRenderMode::StyledText => "styled_text",
            FrameRenderMode::ColorFrames => "color_frames",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "styled_text" => FrameRenderMode::StyledText,
            "color_frames" => FrameRenderMode::ColorFrames,
            _ => FrameRenderMode::BwText,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClipSpeedMode {
    Default,
    Sync,
}

impl ClipSpeedMode {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            ClipSpeedMode::Default => "default",
            ClipSpeedMode::Sync => "sync",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "sync" => ClipSpeedMode::Sync,
            _ => ClipSpeedMode::Default,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub timeline_id: String,
    pub project_id: String,
    pub creation_date: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineClip {
    pub clip_id: String,
    pub project_id: String,
    pub timeline_id: String,
    pub order_index: i32,
    pub media_type: TimelineMediaType,
    pub resource_kind: TimelineResourceKind,
    pub actual_resource_id: String,
    pub frame_render_mode: Option<FrameRenderMode>,
    pub clip_speed_mode: Option<ClipSpeedMode>,
    pub length_seconds: f64,
    pub creation_date: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineClipDraft {
    pub clip_id: String,
    pub media_type: TimelineMediaType,
    pub resource_kind: TimelineResourceKind,
    pub actual_resource_id: String,
    pub frame_render_mode: Option<FrameRenderMode>,
    pub clip_speed_mode: Option<ClipSpeedMode>,
    pub length_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTimeline {
    pub timeline: Option<Timeline>,
    pub clips: Vec<TimelineClip>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectContentType {
    Preview,
    Image,
    Frame,
    Cut,
    Source,
    Frames,
}

impl ProjectContentType {
    pub(crate) fn to_string(&self) -> &str {
        match self {
            ProjectContentType::Preview => "preview",
            ProjectContentType::Image => "image",
            ProjectContentType::Frame => "frame",
            ProjectContentType::Cut => "cut",
            ProjectContentType::Source => "source",
            ProjectContentType::Frames => "frames",
        }
    }

    pub(crate) fn from_string(s: &str) -> Self {
        match s {
            "preview" => ProjectContentType::Preview,
            "image" => ProjectContentType::Image,
            "frame" => ProjectContentType::Frame,
            "cut" => ProjectContentType::Cut,
            "frames" => ProjectContentType::Frames,
            _ => ProjectContentType::Source,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContentEntry {
    pub id: String,
    pub project_id: String,
    pub item_id: String,
    pub item_name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: ProjectContentType,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContentDraft {
    pub item_id: String,
    pub item_name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: ProjectContentType,
}

pub(crate) fn default_output_mode() -> String {
    "text-only".to_string()
}
