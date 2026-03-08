use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use super::explorer_types::{ExplorerItem, ExplorerLayout, ResourceRef};
use crate::components::settings::available_cuts::VideoCut;
use crate::pages::project::{ContentType, FrameDirectory, Preview, SourceContent};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectContentType {
    Preview,
    Image,
    Frame,
    Cut,
    Source,
    Frames,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectContentEntry {
    pub id: String,
    pub project_id: String,
    pub item_id: String,
    pub item_name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: ProjectContentType,
    pub creation_date: String,
    pub last_modified: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectContentDraft {
    pub item_id: String,
    pub item_name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub item_type: ProjectContentType,
}

fn file_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn source_item_name(source: &SourceContent) -> String {
    source
        .custom_name
        .clone()
        .unwrap_or_else(|| file_name_from_path(&source.file_path))
}

fn cut_item_name(cut: &VideoCut) -> String {
    cut.custom_name.clone().unwrap_or_else(|| {
        let sm = (cut.start_time / 60.0) as u32;
        let ss = (cut.start_time % 60.0) as u32;
        let em = (cut.end_time / 60.0) as u32;
        let es = (cut.end_time % 60.0) as u32;
        format!("Cut {:02}:{:02} - {:02}:{:02}", sm, ss, em, es)
    })
}

fn frame_item_name(frame_dir: &FrameDirectory) -> String {
    frame_dir.name.clone()
}

fn preview_item_name(preview: &Preview) -> String {
    preview
        .custom_name
        .clone()
        .unwrap_or_else(|| preview.folder_name.clone())
}

fn draft_for_resource(
    resource: &ResourceRef,
    folder_segments: &[String],
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
) -> Option<ProjectContentDraft> {
    let (item_id, item_name, item_type) = match resource {
        ResourceRef::SourceFile { source_id } => {
            let source = source_files.iter().find(|source| source.id == *source_id)?;
            let item_type = if source.content_type == ContentType::Image {
                ProjectContentType::Image
            } else {
                ProjectContentType::Source
            };
            (source.id.clone(), source_item_name(source), item_type)
        }
        ResourceRef::VideoCut { cut_id } => {
            let cut = video_cuts.iter().find(|cut| cut.id == *cut_id)?;
            (cut.id.clone(), cut_item_name(cut), ProjectContentType::Cut)
        }
        ResourceRef::FrameDirectory { directory_path } => {
            let frame_dir = frame_directories
                .iter()
                .find(|frame_dir| frame_dir.directory_path == *directory_path)?;
            (
                frame_dir.conversion_id.clone(),
                frame_item_name(frame_dir),
                ProjectContentType::Frames,
            )
        }
        ResourceRef::Preview { preview_id } => {
            let preview = previews.iter().find(|preview| preview.id == *preview_id)?;
            (
                preview.id.clone(),
                preview_item_name(preview),
                ProjectContentType::Preview,
            )
        }
    };

    let path = if folder_segments.is_empty() {
        item_name.clone()
    } else {
        format!("{}/{}", folder_segments.join("/"), item_name)
    };

    Some(ProjectContentDraft {
        item_id,
        item_name,
        path,
        item_type,
    })
}

fn collect_project_content_drafts(
    items: &[ExplorerItem],
    folder_segments: &[String],
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
    drafts: &mut Vec<ProjectContentDraft>,
) {
    for item in items {
        match item {
            ExplorerItem::Folder { name, children, .. } => {
                let mut next_segments = folder_segments.to_vec();
                next_segments.push(name.clone());
                collect_project_content_drafts(
                    children,
                    &next_segments,
                    source_files,
                    video_cuts,
                    frame_directories,
                    previews,
                    drafts,
                );
            }
            ExplorerItem::ResourceRef(resource) => {
                if let Some(draft) = draft_for_resource(
                    resource,
                    folder_segments,
                    source_files,
                    video_cuts,
                    frame_directories,
                    previews,
                ) {
                    drafts.push(draft);
                }
            }
        }
    }
}

pub fn project_content_from_layout(
    layout: &ExplorerLayout,
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
) -> Vec<ProjectContentDraft> {
    let mut drafts = Vec::new();
    collect_project_content_drafts(
        &layout.root_items,
        &[],
        source_files,
        video_cuts,
        frame_directories,
        previews,
        &mut drafts,
    );

    let mut seen = HashSet::new();
    drafts.retain(|draft| {
        seen.insert(format!(
            "{}|{}|{}",
            draft.item_type_string(),
            draft.item_id,
            draft.path
        ))
    });
    drafts
}

fn ensure_folder<'a>(
    items: &'a mut Vec<ExplorerItem>,
    folder_name: &str,
    folder_path: &str,
) -> &'a mut Vec<ExplorerItem> {
    let existing_index = items.iter().position(|item| match item {
        ExplorerItem::Folder { id, name, .. } => id == folder_path && name == folder_name,
        ExplorerItem::ResourceRef(_) => false,
    });

    if let Some(index) = existing_index {
        match &mut items[index] {
            ExplorerItem::Folder { children, .. } => return children,
            ExplorerItem::ResourceRef(_) => unreachable!(),
        }
    }

    items.push(ExplorerItem::Folder {
        id: folder_path.to_string(),
        name: folder_name.to_string(),
        children: Vec::new(),
        is_expanded: true,
    });

    match items.last_mut() {
        Some(ExplorerItem::Folder { children, .. }) => children,
        _ => unreachable!(),
    }
}

fn insert_resource(
    items: &mut Vec<ExplorerItem>,
    folder_segments: &[String],
    current_path: &str,
    resource: ResourceRef,
) {
    if folder_segments.is_empty() {
        if !items.iter().any(
            |item| matches!(item, ExplorerItem::ResourceRef(existing) if *existing == resource),
        ) {
            items.push(ExplorerItem::ResourceRef(resource));
        }
        return;
    }

    let folder_name = &folder_segments[0];
    let folder_path = if current_path.is_empty() {
        folder_name.clone()
    } else {
        format!("{}/{}", current_path, folder_name)
    };
    let children = ensure_folder(items, folder_name, &folder_path);
    insert_resource(children, &folder_segments[1..], &folder_path, resource);
}

fn resource_from_entry(
    entry: &ProjectContentEntry,
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
) -> Option<ResourceRef> {
    match entry.item_type {
        ProjectContentType::Source | ProjectContentType::Image => source_files
            .iter()
            .find(|source| source.id == entry.item_id)
            .map(|source| ResourceRef::SourceFile {
                source_id: source.id.clone(),
            }),
        ProjectContentType::Cut => {
            video_cuts
                .iter()
                .find(|cut| cut.id == entry.item_id)
                .map(|cut| ResourceRef::VideoCut {
                    cut_id: cut.id.clone(),
                })
        }
        ProjectContentType::Frames => frame_directories
            .iter()
            .find(|frame_dir| {
                frame_dir.conversion_id == entry.item_id
                    || frame_dir.directory_path == entry.item_id
            })
            .map(|frame_dir| ResourceRef::FrameDirectory {
                directory_path: frame_dir.directory_path.clone(),
            }),
        ProjectContentType::Preview | ProjectContentType::Frame => previews
            .iter()
            .find(|preview| preview.id == entry.item_id)
            .map(|preview| ResourceRef::Preview {
                preview_id: preview.id.clone(),
            }),
    }
}

pub fn hydrate_layout_from_project_content(
    project_id: &str,
    entries: &[ProjectContentEntry],
    source_files: &[SourceContent],
    video_cuts: &[VideoCut],
    frame_directories: &[FrameDirectory],
    previews: &[Preview],
) -> ExplorerLayout {
    let mut root_items = Vec::new();
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by(|left, right| left.path.to_lowercase().cmp(&right.path.to_lowercase()));

    for entry in sorted_entries {
        let Some(resource) = resource_from_entry(
            &entry,
            source_files,
            video_cuts,
            frame_directories,
            previews,
        ) else {
            continue;
        };

        let folder_segments = entry
            .path
            .rsplit_once('/')
            .map(|(folder_path, _)| {
                folder_path
                    .split('/')
                    .filter(|segment| !segment.is_empty())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        insert_resource(&mut root_items, &folder_segments, "", resource);
    }

    ExplorerLayout {
        project_id: project_id.to_string(),
        root_items,
    }
}

impl ProjectContentDraft {
    fn item_type_string(&self) -> &'static str {
        match self.item_type {
            ProjectContentType::Preview => "preview",
            ProjectContentType::Image => "image",
            ProjectContentType::Frame => "frame",
            ProjectContentType::Cut => "cut",
            ProjectContentType::Source => "source",
            ProjectContentType::Frames => "frames",
        }
    }
}
