use super::{
    cut_display_name, frame_display_name, get_project_content_entries_internal,
    preview_display_name, replace_project_content_entries, source_display_name, table_exists,
    ProjectContentDraft, ProjectContentType,
};
use rusqlite::{Connection, Result as SqlResult};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct LegacyExplorerLayout {
    #[serde(default)]
    root_items: Vec<LegacyExplorerItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) enum LegacyExplorerItem {
    Folder {
        name: String,
        #[serde(default)]
        children: Vec<LegacyExplorerItem>,
    },
    ResourceRef(LegacyResourceRef),
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) enum LegacyResourceRef {
    SourceFile { source_id: String },
    VideoCut { cut_id: String },
    FrameDirectory { directory_path: String },
    Preview { preview_id: String },
}

pub(crate) fn legacy_resource_to_project_content_draft(
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

pub(crate) fn flatten_legacy_explorer_items(
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

pub(crate) fn migrate_explorer_layout_to_project_content(conn: &Connection) -> SqlResult<()> {
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
