use super::{ProjectContentDraft, ProjectContentEntry, ProjectContentType};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub(crate) fn file_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn cut_display_name(
    custom_name: Option<String>,
    start_time: f64,
    end_time: f64,
) -> String {
    custom_name.unwrap_or_else(|| {
        let sm = (start_time / 60.0) as u32;
        let ss = (start_time % 60.0) as u32;
        let em = (end_time / 60.0) as u32;
        let es = (end_time % 60.0) as u32;
        format!("Cut {:02}:{:02} - {:02}:{:02}", sm, ss, em, es)
    })
}

pub(crate) fn source_display_name(custom_name: Option<String>, file_path: &str) -> String {
    custom_name.unwrap_or_else(|| file_name_from_path(file_path))
}

pub(crate) fn preview_display_name(custom_name: Option<String>, folder_name: &str) -> String {
    custom_name.unwrap_or_else(|| folder_name.to_string())
}

pub(crate) fn frame_source_name_from_folder_name(folder_name: &str) -> String {
    if let Some(bracket_start) = folder_name.find("_ascii[") {
        folder_name[..bracket_start].to_string()
    } else if let Some(stripped) = folder_name.strip_suffix("_ascii") {
        stripped.to_string()
    } else {
        folder_name.to_string()
    }
}

pub(crate) fn frame_display_name(custom_name: Option<String>, folder_name: &str) -> String {
    custom_name.unwrap_or_else(|| {
        format!(
            "{} - Frames",
            frame_source_name_from_folder_name(folder_name)
        )
    })
}

pub(crate) fn project_content_row_id(
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

pub(crate) fn table_exists(conn: &Connection, table_name: &str) -> SqlResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table_name],
        |row| row.get(0),
    )
}

pub(crate) fn get_project_content_entries_internal(
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

pub(crate) fn replace_project_content_entries(
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

pub(crate) fn delete_project_content_rows_for_item(
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

pub(crate) fn rename_project_content_rows_for_item(
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

pub fn get_project_content(project_id: &str) -> SqlResult<Vec<ProjectContentEntry>> {
    let conn = super::init_database()?;
    get_project_content_entries_internal(&conn, project_id)
}

pub fn save_project_content(project_id: &str, entries: &[ProjectContentDraft]) -> SqlResult<()> {
    let conn = super::init_database()?;
    conn.execute("BEGIN IMMEDIATE TRANSACTION", [])?;

    let result = replace_project_content_entries(&conn, project_id, entries, None);
    if result.is_ok() {
        conn.execute("COMMIT", [])?;
    } else {
        let _ = conn.execute("ROLLBACK", []);
    }

    result
}
