use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProjectType {
    Image,
    Animation,
}

impl ProjectType {
    fn to_string(&self) -> &str {
        match self {
            ProjectType::Image => "image",
            ProjectType::Animation => "animation",
        }
    }

    fn from_string(s: &str) -> Self {
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
    fn to_string(&self) -> &str {
        match self {
            SourceType::Image => "image",
            SourceType::Video => "video",
        }
    }

    fn from_string(s: &str) -> Self {
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
    pub size: i64, // in bytes
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
    pub size: i64, // in bytes
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
    pub folder_name: String,          // Name of the frames folder
    pub folder_path: String,          // Full path to the frames folder
    pub frame_count: i32,             // Number of frames
    pub source_file_id: String,       // Foreign key to source_content
    pub project_id: String,           // Foreign key to projects
    pub settings: ConversionSettings, // Conversion settings (luminance, font_ratio, columns, fps)
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,             // Total size of all frame files in bytes
    pub custom_name: Option<String>, // Custom display name for the frame directory
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoCut {
    pub id: String,
    pub project_id: String,
    pub source_file_id: String, // Foreign key to source_content (which video it was cut from)
    pub file_path: String,      // Full path to the cut video file
    pub date_added: DateTime<Utc>,
    pub size: i64, // File size in bytes
    pub custom_name: Option<String>,
    pub start_time: f64, // Cut start time in seconds
    pub end_time: f64,   // Cut end time in seconds
    pub duration: f64,   // Duration of the cut in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioExtraction {
    pub id: String,
    pub folder_name: String,    // Name of the audio folder
    pub folder_path: String,    // Full path to the audio folder
    pub source_file_id: String, // Foreign key to source_content
    pub project_id: String,     // Foreign key to projects
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,             // Total size of all audio files in bytes
    pub audio_track_beginning: f64,  // Start time in seconds
    pub audio_track_end: f64,        // End time in seconds
    pub custom_name: Option<String>, // Custom display name
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
    pub folder_name: String,       // Name of the preview folder
    pub folder_path: String,       // Full path to the preview folder
    pub frame_count: i32,          // Always 1 for previews
    pub source_file_id: String,    // Foreign key to source_content
    pub project_id: String,        // Foreign key to projects
    pub settings: PreviewSettings, // Conversion settings
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,             // Total size of all files in bytes
    pub custom_name: Option<String>, // Custom display name
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimelineMediaType {
    Video,
    Frames,
    Frame,
}

impl TimelineMediaType {
    fn to_string(&self) -> &str {
        match self {
            TimelineMediaType::Video => "video",
            TimelineMediaType::Frames => "frames",
            TimelineMediaType::Frame => "frame",
        }
    }

    fn from_string(s: &str) -> Self {
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
    fn to_string(&self) -> &str {
        match self {
            TimelineResourceKind::Source => "source",
            TimelineResourceKind::Cut => "cut",
            TimelineResourceKind::AsciiConversion => "ascii_conversion",
            TimelineResourceKind::Preview => "preview",
        }
    }

    fn from_string(s: &str) -> Self {
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
    fn to_string(&self) -> &str {
        match self {
            FrameRenderMode::BwText => "bw_text",
            FrameRenderMode::StyledText => "styled_text",
            FrameRenderMode::ColorFrames => "color_frames",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "styled_text" => FrameRenderMode::StyledText,
            "color_frames" => FrameRenderMode::ColorFrames,
            _ => FrameRenderMode::BwText,
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
    fn to_string(&self) -> &str {
        match self {
            ProjectContentType::Preview => "preview",
            ProjectContentType::Image => "image",
            ProjectContentType::Frame => "frame",
            ProjectContentType::Cut => "cut",
            ProjectContentType::Source => "source",
            ProjectContentType::Frames => "frames",
        }
    }

    fn from_string(s: &str) -> Self {
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

#[derive(Debug, Clone, Deserialize)]
struct LegacyExplorerLayout {
    #[serde(default)]
    root_items: Vec<LegacyExplorerItem>,
}

#[derive(Debug, Clone, Deserialize)]
enum LegacyExplorerItem {
    Folder {
        name: String,
        #[serde(default)]
        children: Vec<LegacyExplorerItem>,
    },
    ResourceRef(LegacyResourceRef),
}

#[derive(Debug, Clone, Deserialize)]
enum LegacyResourceRef {
    SourceFile { source_id: String },
    VideoCut { cut_id: String },
    FrameDirectory { directory_path: String },
    Preview { preview_id: String },
}

fn default_output_mode() -> String {
    "text-only".to_string()
}

fn file_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn cut_display_name(custom_name: Option<String>, start_time: f64, end_time: f64) -> String {
    custom_name.unwrap_or_else(|| {
        let sm = (start_time / 60.0) as u32;
        let ss = (start_time % 60.0) as u32;
        let em = (end_time / 60.0) as u32;
        let es = (end_time % 60.0) as u32;
        format!("Cut {:02}:{:02} - {:02}:{:02}", sm, ss, em, es)
    })
}

fn source_display_name(custom_name: Option<String>, file_path: &str) -> String {
    custom_name.unwrap_or_else(|| file_name_from_path(file_path))
}

fn preview_display_name(custom_name: Option<String>, folder_name: &str) -> String {
    custom_name.unwrap_or_else(|| folder_name.to_string())
}

fn frame_source_name_from_folder_name(folder_name: &str) -> String {
    if let Some(bracket_start) = folder_name.find("_ascii[") {
        folder_name[..bracket_start].to_string()
    } else if let Some(stripped) = folder_name.strip_suffix("_ascii") {
        stripped.to_string()
    } else {
        folder_name.to_string()
    }
}

fn frame_display_name(custom_name: Option<String>, folder_name: &str) -> String {
    custom_name.unwrap_or_else(|| {
        format!(
            "{} - Frames",
            frame_source_name_from_folder_name(folder_name)
        )
    })
}

fn project_content_row_id(
    project_id: &str,
    item_type: &ProjectContentType,
    item_id: &str,
    path: &str,
) -> String {
    format!(
        "project_content|{}|{}|{}|{}",
        project_id,
        item_type.to_string(),
        item_id,
        path
    )
}

fn table_exists(conn: &Connection, table_name: &str) -> SqlResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table_name],
        |row| row.get(0),
    )
}

fn get_project_content_entries_internal(
    conn: &Connection,
    project_id: &str,
) -> SqlResult<Vec<ProjectContentEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, item_id, item_name, path, type, creation_date, last_modified
         FROM project_content
         WHERE project_id = ?1
         ORDER BY path COLLATE NOCASE ASC, item_name COLLATE NOCASE ASC",
    )?;

    let rows = stmt
        .query_map([project_id], |row| {
            let creation_date: String = row.get(6)?;
            let last_modified: String = row.get(7)?;
            Ok(ProjectContentEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                item_id: row.get(2)?,
                item_name: row.get(3)?,
                path: row.get(4)?,
                item_type: ProjectContentType::from_string(&row.get::<_, String>(5)?),
                creation_date: DateTime::parse_from_rfc3339(&creation_date)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                last_modified: DateTime::parse_from_rfc3339(&last_modified)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(rows)
}

fn replace_project_content_entries(
    conn: &Connection,
    project_id: &str,
    entries: &[ProjectContentDraft],
    default_timestamp: Option<&str>,
) -> SqlResult<()> {
    let existing_entries = get_project_content_entries_internal(conn, project_id)?;
    let existing_by_id = existing_entries
        .into_iter()
        .map(|entry| (entry.id.clone(), entry))
        .collect::<HashMap<_, _>>();

    let now = Utc::now().to_rfc3339();
    let fallback_timestamp = default_timestamp.unwrap_or(now.as_str()).to_string();
    let mut retained_ids = HashSet::new();

    for entry in entries {
        if entry.item_id.trim().is_empty()
            || entry.item_name.trim().is_empty()
            || entry.path.trim().is_empty()
        {
            continue;
        }

        let row_id =
            project_content_row_id(project_id, &entry.item_type, &entry.item_id, &entry.path);
        if !retained_ids.insert(row_id.clone()) {
            continue;
        }

        let (creation_date, last_modified) = if let Some(existing) = existing_by_id.get(&row_id) {
            let unchanged = existing.item_id == entry.item_id
                && existing.item_name == entry.item_name
                && existing.path == entry.path
                && existing.item_type == entry.item_type;
            (
                existing.creation_date.to_rfc3339(),
                if unchanged {
                    existing.last_modified.to_rfc3339()
                } else {
                    now.clone()
                },
            )
        } else {
            (fallback_timestamp.clone(), fallback_timestamp.clone())
        };

        conn.execute(
            "INSERT INTO project_content (id, project_id, item_id, item_name, path, type, creation_date, last_modified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                project_id = excluded.project_id,
                item_id = excluded.item_id,
                item_name = excluded.item_name,
                path = excluded.path,
                type = excluded.type,
                creation_date = excluded.creation_date,
                last_modified = excluded.last_modified",
            params![
                row_id,
                project_id,
                entry.item_id,
                entry.item_name,
                entry.path,
                entry.item_type.to_string(),
                creation_date,
                last_modified,
            ],
        )?;
    }

    let obsolete_ids = existing_by_id
        .keys()
        .filter(|id| !retained_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();

    for row_id in obsolete_ids {
        conn.execute("DELETE FROM project_content WHERE id = ?1", [row_id])?;
    }

    Ok(())
}

fn legacy_resource_to_project_content_draft(
    conn: &Connection,
    resource: &LegacyResourceRef,
    folder_segments: &[String],
) -> SqlResult<Option<ProjectContentDraft>> {
    let resolved = match resource {
        LegacyResourceRef::SourceFile { source_id } => {
            let mut stmt = conn.prepare(
                "SELECT content_type, file_path, custom_name
                 FROM source_content
                 WHERE id = ?1
                 LIMIT 1",
            )?;
            let mut rows = stmt.query([source_id])?;
            rows.next()?.map(|row| {
                let content_type = row.get::<_, String>(0)?;
                let file_path = row.get::<_, String>(1)?;
                let custom_name = row.get::<_, Option<String>>(2)?;
                Ok::<ProjectContentDraft, rusqlite::Error>(ProjectContentDraft {
                    item_id: source_id.clone(),
                    item_name: source_display_name(custom_name, &file_path),
                    path: String::new(),
                    item_type: if content_type == "image" {
                        ProjectContentType::Image
                    } else {
                        ProjectContentType::Source
                    },
                })
            })
        }
        LegacyResourceRef::VideoCut { cut_id } => {
            let mut stmt = conn.prepare(
                "SELECT custom_name, start_time, end_time
                 FROM cuts
                 WHERE id = ?1
                 LIMIT 1",
            )?;
            let mut rows = stmt.query([cut_id])?;
            rows.next()?.map(|row| {
                let custom_name = row.get::<_, Option<String>>(0)?;
                let start_time = row.get::<_, f64>(1)?;
                let end_time = row.get::<_, f64>(2)?;
                Ok::<ProjectContentDraft, rusqlite::Error>(ProjectContentDraft {
                    item_id: cut_id.clone(),
                    item_name: cut_display_name(custom_name, start_time, end_time),
                    path: String::new(),
                    item_type: ProjectContentType::Cut,
                })
            })
        }
        LegacyResourceRef::FrameDirectory { directory_path } => {
            let mut stmt = conn.prepare(
                "SELECT id, folder_name, custom_name
                 FROM ascii_conversions
                 WHERE folder_path = ?1
                 LIMIT 1",
            )?;
            let mut rows = stmt.query([directory_path])?;
            rows.next()?.map(|row| {
                let conversion_id = row.get::<_, String>(0)?;
                let folder_name = row.get::<_, String>(1)?;
                let custom_name = row.get::<_, Option<String>>(2)?;
                Ok::<ProjectContentDraft, rusqlite::Error>(ProjectContentDraft {
                    item_id: conversion_id,
                    item_name: frame_display_name(custom_name, &folder_name),
                    path: String::new(),
                    item_type: ProjectContentType::Frames,
                })
            })
        }
        LegacyResourceRef::Preview { preview_id } => {
            let mut stmt = conn.prepare(
                "SELECT folder_name, custom_name
                 FROM previews
                 WHERE id = ?1
                 LIMIT 1",
            )?;
            let mut rows = stmt.query([preview_id])?;
            rows.next()?.map(|row| {
                let folder_name = row.get::<_, String>(0)?;
                let custom_name = row.get::<_, Option<String>>(1)?;
                Ok::<ProjectContentDraft, rusqlite::Error>(ProjectContentDraft {
                    item_id: preview_id.clone(),
                    item_name: preview_display_name(custom_name, &folder_name),
                    path: String::new(),
                    item_type: ProjectContentType::Preview,
                })
            })
        }
    };

    let Some(mut draft) = resolved.transpose()? else {
        return Ok(None);
    };

    draft.path = if folder_segments.is_empty() {
        draft.item_name.clone()
    } else {
        format!("{}/{}", folder_segments.join("/"), draft.item_name)
    };

    Ok(Some(draft))
}

fn flatten_legacy_explorer_items(
    conn: &Connection,
    items: &[LegacyExplorerItem],
    folder_segments: &[String],
    drafts: &mut Vec<ProjectContentDraft>,
) -> SqlResult<()> {
    for item in items {
        match item {
            LegacyExplorerItem::Folder { name, children } => {
                let mut next_segments = folder_segments.to_vec();
                next_segments.push(name.clone());
                flatten_legacy_explorer_items(conn, children, &next_segments, drafts)?;
            }
            LegacyExplorerItem::ResourceRef(resource) => {
                if let Some(draft) =
                    legacy_resource_to_project_content_draft(conn, resource, folder_segments)?
                {
                    drafts.push(draft);
                }
            }
        }
    }

    Ok(())
}

fn migrate_explorer_layout_to_project_content(conn: &Connection) -> SqlResult<()> {
    if !table_exists(conn, "explorer_layout")? {
        return Ok(());
    }

    let layouts = {
        let mut stmt = conn.prepare(
            "SELECT project_id, layout_json, last_modified
             FROM explorer_layout",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };

    for (project_id, layout_json, last_modified) in layouts {
        if project_id.trim().is_empty() {
            continue;
        }

        if !get_project_content_entries_internal(conn, &project_id)?.is_empty() {
            continue;
        }

        if let Ok(layout) = serde_json::from_str::<LegacyExplorerLayout>(&layout_json) {
            let mut drafts = Vec::new();
            flatten_legacy_explorer_items(conn, &layout.root_items, &[], &mut drafts)?;
            replace_project_content_entries(conn, &project_id, &drafts, Some(&last_modified))?;
        }
    }

    conn.execute("DROP TABLE IF EXISTS explorer_layout", [])?;
    conn.execute("VACUUM", [])?;
    Ok(())
}

fn delete_project_content_rows_for_item(
    conn: &Connection,
    item_id: &str,
    item_types: &[ProjectContentType],
) -> SqlResult<()> {
    for item_type in item_types {
        conn.execute(
            "DELETE FROM project_content WHERE item_id = ?1 AND type = ?2",
            params![item_id, item_type.to_string()],
        )?;
    }

    Ok(())
}

fn rename_project_content_rows_for_item(
    conn: &Connection,
    item_id: &str,
    item_types: &[ProjectContentType],
    next_item_name: &str,
) -> SqlResult<()> {
    let target_types = item_types
        .iter()
        .map(ProjectContentType::to_string)
        .collect::<HashSet<_>>();
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, path, item_name, type
             FROM project_content
             WHERE item_id = ?1",
        )?;
        let rows = stmt
            .query_map([item_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };

    let now = Utc::now().to_rfc3339();
    for (row_id, project_id, path, current_item_name, item_type) in rows {
        if !target_types.contains(item_type.as_str()) {
            continue;
        }

        let folder_prefix = if path == current_item_name {
            String::new()
        } else if let Some(stripped) = path.strip_suffix(&current_item_name) {
            stripped.trim_end_matches('/').to_string()
        } else if let Some((prefix, _)) = path.rsplit_once('/') {
            prefix.to_string()
        } else {
            String::new()
        };

        let next_path = if folder_prefix.is_empty() {
            next_item_name.to_string()
        } else {
            format!("{}/{}", folder_prefix, next_item_name)
        };
        let next_type = ProjectContentType::from_string(&item_type);
        let next_row_id = project_content_row_id(&project_id, &next_type, item_id, &next_path);

        conn.execute(
            "UPDATE project_content
             SET id = ?1, item_name = ?2, path = ?3, last_modified = ?4
             WHERE id = ?5",
            params![next_row_id, next_item_name, next_path, now, row_id],
        )?;
    }

    Ok(())
}

fn app_support_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("cascii_studio")
}

fn database_path() -> PathBuf {
    app_support_dir().join("projects.db")
}

pub fn init_database() -> SqlResult<Connection> {
    let db_path = database_path();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(db_path)?;

    // Create projects table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS projects (
            id TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            project_type TEXT NOT NULL,
            project_path TEXT NOT NULL,
            size INTEGER NOT NULL DEFAULT 0,
            frames INTEGER NOT NULL DEFAULT 0,
            creation_date TEXT NOT NULL,
            last_modified TEXT NOT NULL
        )",
        [],
    )?;

    // Create source_content table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS source_content (
            id TEXT PRIMARY KEY,
            content_type TEXT NOT NULL,
            project_id TEXT NOT NULL,
            date_added TEXT NOT NULL,
            size INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            custom_name TEXT,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Check if custom_name column exists, if not add it
    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('source_content') WHERE name='custom_name'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        // Add custom_name column for existing databases
        conn.execute("ALTER TABLE source_content ADD COLUMN custom_name TEXT", [])?;
    }

    // Create index on project_id for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_source_project_id ON source_content(project_id)",
        [],
    )?;

    // Create ascii_conversions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS ascii_conversions (
            id TEXT PRIMARY KEY,
            folder_name TEXT NOT NULL,
            folder_path TEXT NOT NULL,
            frame_count INTEGER NOT NULL,
            source_file_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            luminance INTEGER NOT NULL,
            font_ratio REAL NOT NULL,
            columns INTEGER NOT NULL,
            fps INTEGER NOT NULL,
            frame_speed INTEGER NOT NULL DEFAULT 0,
            creation_date TEXT NOT NULL,
            total_size INTEGER NOT NULL,
            custom_name TEXT,
            FOREIGN KEY (source_file_id) REFERENCES source_content(id) ON DELETE CASCADE,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Check if frame_speed column exists, if not add it
    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='frame_speed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        // Add frame_speed column for existing databases
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN frame_speed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;

        // Update existing records to set frame_speed = fps
        conn.execute("UPDATE ascii_conversions SET frame_speed = fps", [])?;
    }

    // Check if custom_name column exists, if not add it
    let column_exists = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='custom_name'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        // Add custom_name column for existing databases
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN custom_name TEXT",
            [],
        )?;
    }

    // Check if color column exists, if not add it (default false)
    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        // Add color column for existing databases (default 0 = false)
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN color INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='output_mode'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN output_mode TEXT NOT NULL DEFAULT 'text-only'",
            [],
        )?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='foreground_color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN foreground_color TEXT",
            [],
        )?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='background_color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN background_color TEXT",
            [],
        )?;
    }

    conn.execute(
        "UPDATE ascii_conversions
         SET output_mode = CASE
             WHEN output_mode IS NULL OR TRIM(output_mode) = '' THEN CASE WHEN color != 0 THEN 'text+color' ELSE 'text-only' END
             ELSE output_mode
         END",
        [],
    )?;
    conn.execute(
        "UPDATE ascii_conversions
         SET foreground_color = COALESCE(NULLIF(TRIM(foreground_color), ''), 'white'),
             background_color = COALESCE(NULLIF(TRIM(background_color), ''), 'black')",
        [],
    )?;

    // Create indexes for ascii_conversions
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_project_id ON ascii_conversions(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_source_id ON ascii_conversions(source_file_id)",
        [],
    )?;

    // Create cuts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cuts (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            source_file_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            date_added TEXT NOT NULL,
            size INTEGER NOT NULL,
            custom_name TEXT,
            start_time REAL NOT NULL,
            end_time REAL NOT NULL,
            duration REAL NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_file_id) REFERENCES source_content(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for cuts
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cuts_project_id ON cuts(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cuts_source_id ON cuts(source_file_id)",
        [],
    )?;

    // Create audio table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS audio (
            id TEXT PRIMARY KEY,
            folder_name TEXT NOT NULL,
            folder_path TEXT NOT NULL,
            source_file_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            creation_date TEXT NOT NULL,
            total_size INTEGER NOT NULL,
            audio_track_beginning REAL NOT NULL,
            audio_track_end REAL NOT NULL,
            custom_name TEXT,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_file_id) REFERENCES source_content(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for audio
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audio_project_id ON audio(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audio_source_id ON audio(source_file_id)",
        [],
    )?;

    // Create previews table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS previews (
            id TEXT PRIMARY KEY,
            folder_name TEXT NOT NULL,
            folder_path TEXT NOT NULL,
            frame_count INTEGER NOT NULL DEFAULT 1,
            source_file_id TEXT NOT NULL,
            project_id TEXT NOT NULL,
            luminance INTEGER NOT NULL,
            font_ratio REAL NOT NULL,
            columns INTEGER NOT NULL,
            fps INTEGER NOT NULL,
            color INTEGER NOT NULL DEFAULT 0,
            creation_date TEXT NOT NULL,
            total_size INTEGER NOT NULL DEFAULT 0,
            custom_name TEXT,
            output_mode TEXT NOT NULL DEFAULT 'text-only',
            foreground_color TEXT,
            background_color TEXT,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (source_file_id) REFERENCES source_content(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create indexes for previews
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_previews_project_id ON previews(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_previews_source_id ON previews(source_file_id)",
        [],
    )?;

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('previews') WHERE name='output_mode'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE previews ADD COLUMN output_mode TEXT NOT NULL DEFAULT 'text-only'",
            [],
        )?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('previews') WHERE name='foreground_color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute("ALTER TABLE previews ADD COLUMN foreground_color TEXT", [])?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('previews') WHERE name='background_color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute("ALTER TABLE previews ADD COLUMN background_color TEXT", [])?;
    }

    conn.execute(
        "UPDATE previews
         SET output_mode = CASE
             WHEN output_mode IS NULL OR TRIM(output_mode) = '' THEN CASE WHEN color != 0 THEN 'text+color' ELSE 'text-only' END
             ELSE output_mode
         END",
        [],
    )?;
    conn.execute(
        "UPDATE previews
         SET foreground_color = COALESCE(NULLIF(TRIM(foreground_color), ''), 'white'),
             background_color = COALESCE(NULLIF(TRIM(background_color), ''), 'black')",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS project_content (
            id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            item_id TEXT NOT NULL,
            item_name TEXT NOT NULL,
            path TEXT NOT NULL,
            type TEXT NOT NULL,
            creation_date TEXT NOT NULL,
            last_modified TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_project_content_project_id ON project_content(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_project_content_item_lookup ON project_content(project_id, type, item_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_project_content_path ON project_content(project_id, path)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS timelines (
            timeline_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            creation_date TEXT NOT NULL,
            last_updated TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timelines_project_id ON timelines(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timelines_project_updated ON timelines(project_id, last_updated DESC)",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS clips (
            clip_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            timeline_id TEXT NOT NULL,
            order_index INTEGER NOT NULL,
            media_type TEXT NOT NULL,
            resource_kind TEXT NOT NULL,
            actual_resource_id TEXT NOT NULL,
            frame_render_mode TEXT,
            length_seconds REAL NOT NULL,
            creation_date TEXT NOT NULL,
            last_updated TEXT NOT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
            FOREIGN KEY (timeline_id) REFERENCES timelines(timeline_id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clips_project_id ON clips(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clips_timeline_id ON clips(timeline_id)",
        [],
    )?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_clips_timeline_order ON clips(timeline_id, order_index)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_clips_resource ON clips(resource_kind, actual_resource_id)",
        [],
    )?;

    migrate_explorer_layout_to_project_content(&conn)?;

    Ok(conn)
}

pub fn create_project(project: &Project) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "INSERT INTO projects (id, project_name, project_type, project_path, size, frames, creation_date, last_modified)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            project.id,
            project.project_name,
            project.project_type.to_string(),
            project.project_path,
            project.size,
            project.frames,
            project.creation_date.to_rfc3339(),
            project.last_modified.to_rfc3339(),
        ],
    )?;

    Ok(())
}

pub fn add_source_content(source: &SourceContent) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "INSERT INTO source_content (id, content_type, project_id, date_added, size, file_path, custom_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            source.id,
            source.content_type.to_string(),
            source.project_id,
            source.date_added.to_rfc3339(),
            source.size,
            source.file_path,
            source.custom_name,
        ],
    )?;

    Ok(())
}

pub fn get_all_projects() -> SqlResult<Vec<Project>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_name, project_type, project_path, size, frames, creation_date, last_modified 
         FROM projects 
         ORDER BY last_modified DESC"
    )?;

    let projects = stmt
        .query_map([], |row| {
            let creation_str: String = row.get(6)?;
            let modified_str: String = row.get(7)?;

            Ok(Project {
                id: row.get(0)?,
                project_name: row.get(1)?,
                project_type: ProjectType::from_string(&row.get::<_, String>(2)?),
                project_path: row.get(3)?,
                size: row.get(4)?,
                frames: row.get(5)?,
                creation_date: DateTime::parse_from_rfc3339(&creation_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                last_modified: DateTime::parse_from_rfc3339(&modified_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(projects)
}

pub fn get_project(project_id: &str) -> SqlResult<Project> {
    let conn = init_database()?;
    conn.query_row(
        "SELECT id, project_name, project_type, project_path, size, frames, creation_date, last_modified 
         FROM projects 
         WHERE id = ?1",
        [project_id],
        |row| {
            let creation_str: String = row.get(6)?;
            let modified_str: String = row.get(7)?;

            Ok(Project {
                id: row.get(0)?,
                project_name: row.get(1)?,
                project_type: ProjectType::from_string(&row.get::<_, String>(2)?),
                project_path: row.get(3)?,
                size: row.get(4)?,
                frames: row.get(5)?,
                creation_date: DateTime::parse_from_rfc3339(&creation_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                last_modified: DateTime::parse_from_rfc3339(&modified_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        },
    )
}

pub fn get_project_sources(project_id: &str) -> SqlResult<Vec<SourceContent>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, content_type, project_id, date_added, size, file_path, custom_name 
         FROM source_content 
         WHERE project_id = ?1 
         ORDER BY date_added ASC",
    )?;

    let sources = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(3)?;

            Ok(SourceContent {
                id: row.get(0)?,
                content_type: SourceType::from_string(&row.get::<_, String>(1)?),
                project_id: row.get(2)?,
                date_added: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                size: row.get(4)?,
                file_path: row.get(5)?,
                custom_name: row.get(6)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(sources)
}

pub fn update_source_custom_name(source_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "UPDATE source_content SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, source_id],
    )?;

    let mut stmt = conn.prepare(
        "SELECT content_type, file_path, custom_name
         FROM source_content
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([source_id])?;
    if let Some(row) = rows.next()? {
        let content_type = row.get::<_, String>(0)?;
        let file_path = row.get::<_, String>(1)?;
        let custom_name = row.get::<_, Option<String>>(2)?;
        let item_type = if content_type == "image" {
            ProjectContentType::Image
        } else {
            ProjectContentType::Source
        };
        rename_project_content_rows_for_item(
            &conn,
            source_id,
            &[item_type],
            &source_display_name(custom_name, &file_path),
        )?;
    }

    Ok(())
}

pub fn delete_source_content(source_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting source content: {}", source_id);
    let conn = init_database()?;
    let source_type = conn
        .query_row(
            "SELECT content_type FROM source_content WHERE id = ?1 LIMIT 1",
            [source_id],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let source_item_type = if source_type.as_deref() == Some("image") {
        ProjectContentType::Image
    } else {
        ProjectContentType::Source
    };
    let conversion_ids = {
        let mut stmt =
            conn.prepare("SELECT id FROM ascii_conversions WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };
    let cut_ids = {
        let mut stmt = conn.prepare("SELECT id FROM cuts WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };
    let preview_ids = {
        let mut stmt = conn.prepare("SELECT id FROM previews WHERE source_file_id = ?1")?;
        let rows = stmt
            .query_map([source_id], |row| row.get::<_, String>(0))?
            .collect::<SqlResult<Vec<_>>>()?;
        rows
    };

    delete_project_content_rows_for_item(&conn, source_id, &[source_item_type])?;
    for conversion_id in conversion_ids {
        delete_project_content_rows_for_item(&conn, &conversion_id, &[ProjectContentType::Frames])?;
    }
    for cut_id in cut_ids {
        delete_project_content_rows_for_item(&conn, &cut_id, &[ProjectContentType::Cut])?;
    }
    for preview_id in preview_ids {
        delete_project_content_rows_for_item(
            &conn,
            &preview_id,
            &[ProjectContentType::Preview, ProjectContentType::Frame],
        )?;
    }

    // Delete associated ascii conversions first (foreign key constraint)
    let result = conn.execute(
        "DELETE FROM ascii_conversions WHERE source_file_id = ?1",
        [source_id],
    );
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated conversions", rows),
        Err(e) => println!("🗑️ DB: Error deleting conversions: {}", e),
    }
    result?;

    // Delete associated cuts (foreign key constraint)
    let result = conn.execute("DELETE FROM cuts WHERE source_file_id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated cuts", rows),
        Err(e) => println!("🗑️ DB: Error deleting cuts: {}", e),
    }
    result?;

    // Delete associated audio extractions (foreign key constraint)
    let result = conn.execute("DELETE FROM audio WHERE source_file_id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated audio extractions", rows),
        Err(e) => println!("🗑️ DB: Error deleting audio: {}", e),
    }
    result?;

    // Delete associated previews (foreign key constraint)
    let result = conn.execute(
        "DELETE FROM previews WHERE source_file_id = ?1",
        [source_id],
    );
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} associated previews", rows),
        Err(e) => println!("🗑️ DB: Error deleting previews: {}", e),
    }
    result?;

    // Delete the source content
    let result = conn.execute("DELETE FROM source_content WHERE id = ?1", [source_id]);
    match &result {
        Ok(rows) => println!("🗑️ DB: Deleted {} source content rows", rows),
        Err(e) => println!("🗑️ DB: Error deleting source content: {}", e),
    }
    result?;

    Ok(())
}

pub fn update_project_size_and_frames(project_id: &str, size: i64, frames: i32) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "UPDATE projects 
         SET size = ?1, frames = ?2, last_modified = ?3 
         WHERE id = ?4",
        params![size, frames, Utc::now().to_rfc3339(), project_id],
    )?;

    Ok(())
}

pub fn update_project_name(project_id: &str, project_name: &str) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "UPDATE projects
         SET project_name = ?1, last_modified = ?2
         WHERE id = ?3",
        params![project_name, Utc::now().to_rfc3339(), project_id],
    )?;

    Ok(())
}

pub fn delete_project(project_id: &str) -> SqlResult<()> {
    let conn = init_database()?;

    conn.execute(
        "DELETE FROM project_content WHERE project_id = ?1",
        [project_id],
    )?;
    conn.execute("DELETE FROM clips WHERE project_id = ?1", [project_id])?;
    conn.execute("DELETE FROM timelines WHERE project_id = ?1", [project_id])?;

    // Delete all ascii conversions first
    conn.execute(
        "DELETE FROM ascii_conversions WHERE project_id = ?1",
        [project_id],
    )?;

    // Delete all audio extractions
    conn.execute("DELETE FROM audio WHERE project_id = ?1", [project_id])?;

    // Delete all cuts
    conn.execute("DELETE FROM cuts WHERE project_id = ?1", [project_id])?;

    // Delete all previews
    conn.execute("DELETE FROM previews WHERE project_id = ?1", [project_id])?;

    // Delete all source content (should be handled by CASCADE, but being explicit)
    conn.execute(
        "DELETE FROM source_content WHERE project_id = ?1",
        [project_id],
    )?;

    // Delete the project
    conn.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;

    Ok(())
}

pub fn add_ascii_conversion(conversion: &AsciiConversion) -> SqlResult<()> {
    let conn = init_database()?;

    // Round font_ratio to 2 decimal places using f64 for precision
    // f32 cannot precisely represent values like 0.7, but f64 can after rounding
    let font_ratio_rounded = (conversion.settings.font_ratio as f64 * 100.0).round() / 100.0;

    conn.execute(
        "INSERT INTO ascii_conversions (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            conversion.id,
            conversion.folder_name,
            conversion.folder_path,
            conversion.frame_count,
            conversion.source_file_id,
            conversion.project_id,
            conversion.settings.luminance,
            font_ratio_rounded,
            conversion.settings.columns,
            conversion.settings.fps,
            conversion.settings.frame_speed,
            conversion.creation_date.to_rfc3339(),
            conversion.total_size,
            conversion.custom_name,
            conversion.settings.color,
            conversion.settings.output_mode,
            conversion.settings.foreground_color,
            conversion.settings.background_color,
        ],
    )?;

    Ok(())
}

pub fn get_project_conversions(project_id: &str) -> SqlResult<Vec<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color
         FROM ascii_conversions
         WHERE project_id = ?1
         ORDER BY creation_date DESC"
    )?;

    let conversions = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(11)?;

            Ok(AsciiConversion {
                id: row.get(0)?,
                folder_name: row.get(1)?,
                folder_path: row.get(2)?,
                frame_count: row.get(3)?,
                source_file_id: row.get(4)?,
                project_id: row.get(5)?,
                settings: ConversionSettings {
                    luminance: row.get(6)?,
                    font_ratio: row.get(7)?,
                    columns: row.get(8)?,
                    fps: row.get(9)?,
                    frame_speed: row.get(10)?,
                    color: row.get::<_, i32>(14).unwrap_or(0) != 0,
                    output_mode: row
                        .get::<_, Option<String>>(15)?
                        .unwrap_or_else(default_output_mode),
                    foreground_color: row.get(16)?,
                    background_color: row.get(17)?,
                },
                creation_date: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                total_size: row.get(12)?,
                custom_name: row.get(13)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(conversions)
}

pub fn update_conversion_frame_speed(conversion_id: &str, frame_speed: u32) -> SqlResult<()> {
    println!(
        "📝 DB: Updating frame_speed for conversion {} to {}",
        conversion_id, frame_speed
    );
    let conn = init_database()?;

    let result = conn.execute(
        "UPDATE ascii_conversions SET frame_speed = ?1 WHERE id = ?2",
        params![frame_speed, conversion_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn get_conversion_by_folder_path(folder_path: &str) -> SqlResult<Option<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color
         FROM ascii_conversions
         WHERE folder_path = ?1
         LIMIT 1"
    )?;

    let mut rows = stmt.query([folder_path])?;

    if let Some(row) = rows.next()? {
        let date_str: String = row.get(11)?;

        Ok(Some(AsciiConversion {
            id: row.get(0)?,
            folder_name: row.get(1)?,
            folder_path: row.get(2)?,
            frame_count: row.get(3)?,
            source_file_id: row.get(4)?,
            project_id: row.get(5)?,
            settings: ConversionSettings {
                luminance: row.get(6)?,
                font_ratio: row.get(7)?,
                columns: row.get(8)?,
                fps: row.get(9)?,
                frame_speed: row.get(10)?,
                color: row.get::<_, i32>(14).unwrap_or(0) != 0,
                output_mode: row
                    .get::<_, Option<String>>(15)?
                    .unwrap_or_else(default_output_mode),
                foreground_color: row.get(16)?,
                background_color: row.get(17)?,
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_conversion_by_folder_path(folder_path: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting conversion by folder path: {}", folder_path);
    let conn = init_database()?;
    if let Some(conversion_id) = conn
        .query_row(
            "SELECT id FROM ascii_conversions WHERE folder_path = ?1 LIMIT 1",
            [folder_path],
            |row| row.get::<_, String>(0),
        )
        .ok()
    {
        delete_project_content_rows_for_item(&conn, &conversion_id, &[ProjectContentType::Frames])?;
    }
    let result = conn.execute(
        "DELETE FROM ascii_conversions WHERE folder_path = ?1",
        [folder_path],
    );

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_conversion_custom_name(
    conversion_id: &str,
    custom_name: Option<String>,
) -> SqlResult<()> {
    println!(
        "📝 DB: Updating conversion custom_name for {} to {:?}",
        conversion_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE ascii_conversions SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, conversion_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    let mut stmt = conn.prepare(
        "SELECT folder_name, custom_name
         FROM ascii_conversions
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([conversion_id])?;
    if let Some(row) = rows.next()? {
        let folder_name = row.get::<_, String>(0)?;
        let custom_name = row.get::<_, Option<String>>(1)?;
        rename_project_content_rows_for_item(
            &conn,
            conversion_id,
            &[ProjectContentType::Frames],
            &frame_display_name(custom_name, &folder_name),
        )?;
    }

    Ok(())
}

pub fn update_conversion_dimensions(
    conversion_id: &str,
    frame_count: i32,
    total_size: i64,
) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "UPDATE ascii_conversions SET frame_count = ?1, total_size = ?2 WHERE id = ?3",
        params![frame_count, total_size, conversion_id],
    )?;
    Ok(())
}

// ============== Video Cuts CRUD ==============

pub fn add_video_cut(cut: &VideoCut) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "INSERT INTO cuts (id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            cut.id,
            cut.project_id,
            cut.source_file_id,
            cut.file_path,
            cut.date_added.to_rfc3339(),
            cut.size,
            cut.custom_name,
            cut.start_time,
            cut.end_time,
            cut.duration,
        ],
    )?;
    Ok(())
}

pub fn get_project_cuts(project_id: &str) -> SqlResult<Vec<VideoCut>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration
         FROM cuts
         WHERE project_id = ?1
         ORDER BY date_added DESC"
    )?;

    let cuts = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(4)?;
            Ok(VideoCut {
                id: row.get(0)?,
                project_id: row.get(1)?,
                source_file_id: row.get(2)?,
                file_path: row.get(3)?,
                date_added: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                size: row.get(5)?,
                custom_name: row.get(6)?,
                start_time: row.get(7)?,
                end_time: row.get(8)?,
                duration: row.get(9)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(cuts)
}

pub fn delete_cut(cut_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting cut: {}", cut_id);
    let conn = init_database()?;
    delete_project_content_rows_for_item(&conn, cut_id, &[ProjectContentType::Cut])?;
    let result = conn.execute("DELETE FROM cuts WHERE id = ?1", [cut_id]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_cut_custom_name(cut_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!(
        "📝 DB: Updating cut custom_name for {} to {:?}",
        cut_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE cuts SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, cut_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    let mut stmt = conn.prepare(
        "SELECT custom_name, start_time, end_time
         FROM cuts
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([cut_id])?;
    if let Some(row) = rows.next()? {
        let custom_name = row.get::<_, Option<String>>(0)?;
        let start_time = row.get::<_, f64>(1)?;
        let end_time = row.get::<_, f64>(2)?;
        rename_project_content_rows_for_item(
            &conn,
            cut_id,
            &[ProjectContentType::Cut],
            &cut_display_name(custom_name, start_time, end_time),
        )?;
    }

    Ok(())
}

// ============== Audio Extractions CRUD ==============

pub fn add_audio_extraction(audio: &AudioExtraction) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute(
        "INSERT INTO audio (id, folder_name, folder_path, source_file_id, project_id, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            audio.id,
            audio.folder_name,
            audio.folder_path,
            audio.source_file_id,
            audio.project_id,
            audio.creation_date.to_rfc3339(),
            audio.total_size,
            audio.audio_track_beginning,
            audio.audio_track_end,
            audio.custom_name,
        ],
    )?;
    Ok(())
}

pub fn get_project_audio(project_id: &str) -> SqlResult<Vec<AudioExtraction>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, source_file_id, project_id, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name
         FROM audio
         WHERE project_id = ?1
         ORDER BY creation_date DESC"
    )?;

    let audio_list = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(5)?;
            Ok(AudioExtraction {
                id: row.get(0)?,
                folder_name: row.get(1)?,
                folder_path: row.get(2)?,
                source_file_id: row.get(3)?,
                project_id: row.get(4)?,
                creation_date: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                total_size: row.get(6)?,
                audio_track_beginning: row.get(7)?,
                audio_track_end: row.get(8)?,
                custom_name: row.get(9)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(audio_list)
}

pub fn delete_audio(audio_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting audio: {}", audio_id);
    let conn = init_database()?;
    let result = conn.execute("DELETE FROM audio WHERE id = ?1", [audio_id]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn delete_audio_by_folder_path(folder_path: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting audio by folder path: {}", folder_path);
    let conn = init_database()?;
    let result = conn.execute("DELETE FROM audio WHERE folder_path = ?1", [folder_path]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_audio_custom_name(audio_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!(
        "📝 DB: Updating audio custom_name for {} to {:?}",
        audio_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE audio SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, audio_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    Ok(())
}

// ============== Previews CRUD ==============

pub fn add_preview(preview: &Preview) -> SqlResult<()> {
    let conn = init_database()?;

    // Round font_ratio to 2 decimal places
    let font_ratio_rounded = (preview.settings.font_ratio as f64 * 100.0).round() / 100.0;

    conn.execute(
        "INSERT INTO previews (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            preview.id,
            preview.folder_name,
            preview.folder_path,
            preview.frame_count,
            preview.source_file_id,
            preview.project_id,
            preview.settings.luminance,
            font_ratio_rounded,
            preview.settings.columns,
            preview.settings.fps,
            preview.settings.color,
            preview.creation_date.to_rfc3339(),
            preview.total_size,
            preview.custom_name,
            preview.settings.output_mode,
            preview.settings.foreground_color,
            preview.settings.background_color,
        ],
    )?;

    Ok(())
}

pub fn get_project_previews(project_id: &str) -> SqlResult<Vec<Preview>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color
         FROM previews
         WHERE project_id = ?1
         ORDER BY creation_date DESC"
    )?;

    let previews = stmt
        .query_map([project_id], |row| {
            let date_str: String = row.get(11)?;
            Ok(Preview {
                id: row.get(0)?,
                folder_name: row.get(1)?,
                folder_path: row.get(2)?,
                frame_count: row.get(3)?,
                source_file_id: row.get(4)?,
                project_id: row.get(5)?,
                settings: PreviewSettings {
                    luminance: row.get(6)?,
                    font_ratio: row.get(7)?,
                    columns: row.get(8)?,
                    fps: row.get(9)?,
                    color: row.get::<_, i32>(10)? != 0,
                    output_mode: row
                        .get::<_, Option<String>>(14)?
                        .unwrap_or_else(default_output_mode),
                    foreground_color: row.get(15)?,
                    background_color: row.get(16)?,
                },
                creation_date: DateTime::parse_from_rfc3339(&date_str)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                total_size: row.get(12)?,
                custom_name: row.get(13)?,
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;
    Ok(previews)
}

pub fn get_preview(preview_id: &str) -> SqlResult<Option<Preview>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color
         FROM previews
         WHERE id = ?1
         LIMIT 1"
    )?;

    let mut rows = stmt.query([preview_id])?;

    if let Some(row) = rows.next()? {
        let date_str: String = row.get(11)?;
        Ok(Some(Preview {
            id: row.get(0)?,
            folder_name: row.get(1)?,
            folder_path: row.get(2)?,
            frame_count: row.get(3)?,
            source_file_id: row.get(4)?,
            project_id: row.get(5)?,
            settings: PreviewSettings {
                luminance: row.get(6)?,
                font_ratio: row.get(7)?,
                columns: row.get(8)?,
                fps: row.get(9)?,
                color: row.get::<_, i32>(10)? != 0,
                output_mode: row
                    .get::<_, Option<String>>(14)?
                    .unwrap_or_else(default_output_mode),
                foreground_color: row.get(15)?,
                background_color: row.get(16)?,
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn delete_preview(preview_id: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting preview: {}", preview_id);
    let conn = init_database()?;
    delete_project_content_rows_for_item(
        &conn,
        preview_id,
        &[ProjectContentType::Preview, ProjectContentType::Frame],
    )?;
    let result = conn.execute("DELETE FROM previews WHERE id = ?1", [preview_id]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn delete_preview_by_folder_path(folder_path: &str) -> SqlResult<()> {
    println!("🗑️ DB: Deleting preview by folder path: {}", folder_path);
    let conn = init_database()?;
    if let Some(preview_id) = conn
        .query_row(
            "SELECT id FROM previews WHERE folder_path = ?1 LIMIT 1",
            [folder_path],
            |row| row.get::<_, String>(0),
        )
        .ok()
    {
        delete_project_content_rows_for_item(
            &conn,
            &preview_id,
            &[ProjectContentType::Preview, ProjectContentType::Frame],
        )?;
    }
    let result = conn.execute("DELETE FROM previews WHERE folder_path = ?1", [folder_path]);

    match &result {
        Ok(rows_affected) => println!("🗑️ DB: Delete successful, {} rows affected", rows_affected),
        Err(e) => println!("🗑️ DB: Delete failed: {}", e),
    }

    result?;
    Ok(())
}

pub fn update_preview_custom_name(preview_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!(
        "📝 DB: Updating preview custom_name for {} to {:?}",
        preview_id, custom_name
    );
    let conn = init_database()?;
    let result = conn.execute(
        "UPDATE previews SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, preview_id],
    );

    match &result {
        Ok(rows_affected) => println!("📝 DB: Update successful, {} rows affected", rows_affected),
        Err(e) => println!("📝 DB: Update failed: {}", e),
    }

    result?;
    let mut stmt = conn.prepare(
        "SELECT folder_name, custom_name
         FROM previews
         WHERE id = ?1
         LIMIT 1",
    )?;
    let mut rows = stmt.query([preview_id])?;
    if let Some(row) = rows.next()? {
        let folder_name = row.get::<_, String>(0)?;
        let custom_name = row.get::<_, Option<String>>(1)?;
        rename_project_content_rows_for_item(
            &conn,
            preview_id,
            &[ProjectContentType::Preview, ProjectContentType::Frame],
            &preview_display_name(custom_name, &folder_name),
        )?;
    }

    Ok(())
}

// ============== Project Content CRUD ==============

pub fn get_project_content(project_id: &str) -> SqlResult<Vec<ProjectContentEntry>> {
    let conn = init_database()?;
    get_project_content_entries_internal(&conn, project_id)
}

pub fn save_project_content(project_id: &str, entries: &[ProjectContentDraft]) -> SqlResult<()> {
    let conn = init_database()?;
    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])?;

    let result = replace_project_content_entries(&conn, project_id, entries, None);
    if result.is_ok() {
        conn.execute("COMMIT", [])?;
    } else {
        let _ = conn.execute("ROLLBACK", []);
    }

    result
}

pub fn get_preview_by_folder_path(folder_path: &str) -> SqlResult<Option<Preview>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color
         FROM previews
         WHERE folder_path = ?1
         LIMIT 1"
    )?;

    let mut rows = stmt.query([folder_path])?;

    if let Some(row) = rows.next()? {
        let date_str: String = row.get(11)?;
        Ok(Some(Preview {
            id: row.get(0)?,
            folder_name: row.get(1)?,
            folder_path: row.get(2)?,
            frame_count: row.get(3)?,
            source_file_id: row.get(4)?,
            project_id: row.get(5)?,
            settings: PreviewSettings {
                luminance: row.get(6)?,
                font_ratio: row.get(7)?,
                columns: row.get(8)?,
                fps: row.get(9)?,
                color: row.get::<_, i32>(10)? != 0,
                output_mode: row
                    .get::<_, Option<String>>(14)?
                    .unwrap_or_else(default_output_mode),
                foreground_color: row.get(15)?,
                background_color: row.get(16)?,
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

fn parse_rfc3339_or_now(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap_or_else(|_| Utc::now().into())
        .with_timezone(&Utc)
}

fn get_timeline_by_id_with_conn(
    conn: &Connection,
    timeline_id: &str,
) -> SqlResult<Option<Timeline>> {
    let mut stmt = conn.prepare(
        "SELECT timeline_id, project_id, creation_date, last_updated
         FROM timelines
         WHERE timeline_id = ?1
         LIMIT 1",
    )?;

    let mut rows = stmt.query([timeline_id])?;
    if let Some(row) = rows.next()? {
        let creation_date: String = row.get(2)?;
        let last_updated: String = row.get(3)?;
        Ok(Some(Timeline {
            timeline_id: row.get(0)?,
            project_id: row.get(1)?,
            creation_date: parse_rfc3339_or_now(&creation_date),
            last_updated: parse_rfc3339_or_now(&last_updated),
        }))
    } else {
        Ok(None)
    }
}

fn get_timeline_clips_with_conn(
    conn: &Connection,
    timeline_id: &str,
) -> SqlResult<Vec<TimelineClip>> {
    let mut stmt = conn.prepare(
        "SELECT clip_id, project_id, timeline_id, order_index, media_type, resource_kind, actual_resource_id, frame_render_mode, length_seconds, creation_date, last_updated
         FROM clips
         WHERE timeline_id = ?1
         ORDER BY order_index ASC, creation_date ASC",
    )?;

    let clips = stmt
        .query_map([timeline_id], |row| {
            let creation_date: String = row.get(9)?;
            let last_updated: String = row.get(10)?;
            let frame_render_mode = row
                .get::<_, Option<String>>(7)?
                .map(|value| FrameRenderMode::from_string(&value));

            Ok(TimelineClip {
                clip_id: row.get(0)?,
                project_id: row.get(1)?,
                timeline_id: row.get(2)?,
                order_index: row.get(3)?,
                media_type: TimelineMediaType::from_string(&row.get::<_, String>(4)?),
                resource_kind: TimelineResourceKind::from_string(&row.get::<_, String>(5)?),
                actual_resource_id: row.get(6)?,
                frame_render_mode,
                length_seconds: row.get(8)?,
                creation_date: parse_rfc3339_or_now(&creation_date),
                last_updated: parse_rfc3339_or_now(&last_updated),
            })
        })?
        .collect::<SqlResult<Vec<_>>>()?;

    Ok(clips)
}

pub fn get_active_project_timeline(project_id: &str) -> SqlResult<ProjectTimeline> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT timeline_id, project_id, creation_date, last_updated
         FROM timelines
         WHERE project_id = ?1
         ORDER BY last_updated DESC, creation_date DESC
         LIMIT 1",
    )?;

    let mut rows = stmt.query([project_id])?;
    let timeline = if let Some(row) = rows.next()? {
        let creation_date: String = row.get(2)?;
        let last_updated: String = row.get(3)?;
        Some(Timeline {
            timeline_id: row.get(0)?,
            project_id: row.get(1)?,
            creation_date: parse_rfc3339_or_now(&creation_date),
            last_updated: parse_rfc3339_or_now(&last_updated),
        })
    } else {
        None
    };

    let clips = if let Some(timeline) = &timeline {
        get_timeline_clips_with_conn(&conn, &timeline.timeline_id)?
    } else {
        Vec::new()
    };

    Ok(ProjectTimeline { timeline, clips })
}

pub fn save_project_timeline(
    project_id: &str,
    timeline_id: Option<&str>,
    clip_drafts: &[TimelineClipDraft],
) -> SqlResult<ProjectTimeline> {
    let mut conn = init_database()?;
    let tx = conn.transaction()?;
    let now = Utc::now();
    let timeline_id = timeline_id
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let existing_timeline = get_timeline_by_id_with_conn(&tx, &timeline_id)?;
    let creation_date = existing_timeline
        .as_ref()
        .map(|timeline| timeline.creation_date.clone())
        .unwrap_or(now);

    tx.execute(
        "INSERT INTO timelines (timeline_id, project_id, creation_date, last_updated)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(timeline_id) DO UPDATE
         SET project_id = excluded.project_id,
             last_updated = excluded.last_updated",
        params![
            timeline_id,
            project_id,
            creation_date.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    let mut existing_dates = std::collections::HashMap::<String, String>::new();
    {
        let mut stmt =
            tx.prepare("SELECT clip_id, creation_date FROM clips WHERE timeline_id = ?1")?;
        let mut rows = stmt.query([timeline_id.as_str()])?;
        while let Some(row) = rows.next()? {
            existing_dates.insert(row.get(0)?, row.get(1)?);
        }
    }

    tx.execute(
        "DELETE FROM clips WHERE timeline_id = ?1",
        [timeline_id.as_str()],
    )?;

    for (index, clip) in clip_drafts.iter().enumerate() {
        let clip_creation_date = existing_dates
            .get(&clip.clip_id)
            .cloned()
            .unwrap_or_else(|| now.to_rfc3339());

        tx.execute(
            "INSERT INTO clips (clip_id, project_id, timeline_id, order_index, media_type, resource_kind, actual_resource_id, frame_render_mode, length_seconds, creation_date, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                clip.clip_id,
                project_id,
                timeline_id,
                index as i32,
                clip.media_type.to_string(),
                clip.resource_kind.to_string(),
                clip.actual_resource_id,
                clip.frame_render_mode.as_ref().map(FrameRenderMode::to_string),
                clip.length_seconds,
                clip_creation_date,
                now.to_rfc3339(),
            ],
        )?;
    }

    tx.commit()?;
    get_active_project_timeline(project_id)
}
