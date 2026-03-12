use super::{init_database, Project, ProjectType};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};
use std::collections::HashMap;
use uuid::Uuid;

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

pub fn get_all_projects() -> SqlResult<Vec<Project>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, project_name, project_type, project_path, size, frames, creation_date, last_modified
         FROM projects
         ORDER BY last_modified DESC",
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
    conn.execute(
        "DELETE FROM ascii_conversions WHERE project_id = ?1",
        [project_id],
    )?;
    conn.execute("DELETE FROM audio WHERE project_id = ?1", [project_id])?;
    conn.execute("DELETE FROM cuts WHERE project_id = ?1", [project_id])?;
    conn.execute("DELETE FROM previews WHERE project_id = ?1", [project_id])?;
    conn.execute(
        "DELETE FROM source_content WHERE project_id = ?1",
        [project_id],
    )?;
    conn.execute("DELETE FROM projects WHERE id = ?1", [project_id])?;

    Ok(())
}

/// Duplicate all database records for a project, remapping IDs and file paths.
/// `old_dir` / `new_dir` are the absolute project directory paths used to rewrite
/// stored filesystem paths.
pub fn duplicate_project_records(
    old_project_id: &str,
    new_name: &str,
    new_project_path: &str,
    old_dir: &str,
    new_dir: &str,
) -> SqlResult<Project> {
    let mut conn = init_database()?;
    let tx = conn.transaction()?;

    let now = Utc::now();
    let new_project_id = Uuid::new_v4().to_string();

    // Read old project
    let (project_type_str, size, frames): (String, i64, i32) = tx.query_row(
        "SELECT project_type, size, frames FROM projects WHERE id = ?1",
        [old_project_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    // Insert new project
    tx.execute(
        "INSERT INTO projects (id, project_name, project_type, project_path, size, frames, creation_date, last_modified) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![new_project_id, new_name, project_type_str, new_project_path, size, frames, now.to_rfc3339(), now.to_rfc3339()],
    )?;

    // Unified map: old_id → new_id (all resource types share UUID space so no collisions)
    let mut id_map: HashMap<String, String> = HashMap::new();

    // Helper: query a list of id-only rows
    fn query_ids(
        tx: &rusqlite::Transaction,
        sql: &str,
        project_id: &str,
    ) -> SqlResult<Vec<String>> {
        let mut stmt = tx.prepare(sql)?;
        let rows = stmt.query_map([project_id], |row| row.get(0))?;
        rows.collect()
    }

    // Helper: query (id, fk) pairs
    fn query_id_fk(
        tx: &rusqlite::Transaction,
        sql: &str,
        project_id: &str,
    ) -> SqlResult<Vec<(String, String)>> {
        let mut stmt = tx.prepare(sql)?;
        let rows = stmt.query_map([project_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    // Helper: query (id, fk1, fk2) triples
    fn query_id_fk2(
        tx: &rusqlite::Transaction,
        sql: &str,
        project_id: &str,
    ) -> SqlResult<Vec<(String, String, String)>> {
        let mut stmt = tx.prepare(sql)?;
        let rows = stmt.query_map([project_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect()
    }

    fn remap(id_map: &HashMap<String, String>, old: &str) -> String {
        id_map.get(old).cloned().unwrap_or_else(|| old.to_string())
    }

    // --- source_content ---
    for old_id in query_ids(
        &tx,
        "SELECT id FROM source_content WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO source_content (id, content_type, project_id, date_added, size, file_path, custom_name) SELECT ?1, content_type, ?2, date_added, size, REPLACE(file_path, ?3, ?4), custom_name FROM source_content WHERE id = ?5",
            params![new_id, new_project_id, old_dir, new_dir, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- ascii_conversions ---
    for (old_id, old_source_id) in query_id_fk(
        &tx,
        "SELECT id, source_file_id FROM ascii_conversions WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        let new_source_id = remap(&id_map, &old_source_id);
        tx.execute(
            "INSERT INTO ascii_conversions (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color) SELECT ?1, folder_name, REPLACE(folder_path, ?3, ?4), frame_count, ?2, ?5, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name, color, output_mode, foreground_color, background_color FROM ascii_conversions WHERE id = ?6",
            params![new_id, new_source_id, old_dir, new_dir, new_project_id, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- cuts ---
    for (old_id, old_source_id) in query_id_fk(
        &tx,
        "SELECT id, source_file_id FROM cuts WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        let new_source_id = remap(&id_map, &old_source_id);
        tx.execute(
            "INSERT INTO cuts (id, project_id, source_file_id, file_path, date_added, size, custom_name, start_time, end_time, duration) SELECT ?1, ?2, ?3, REPLACE(file_path, ?4, ?5), date_added, size, custom_name, start_time, end_time, duration FROM cuts WHERE id = ?6",
            params![new_id, new_project_id, new_source_id, old_dir, new_dir, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- audio ---
    for (old_id, old_source_id) in query_id_fk(
        &tx,
        "SELECT id, source_file_id FROM audio WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        let new_source_id = remap(&id_map, &old_source_id);
        tx.execute(
            "INSERT INTO audio (id, folder_name, folder_path, source_file_id, project_id, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name) SELECT ?1, folder_name, REPLACE(folder_path, ?3, ?4), ?2, ?5, creation_date, total_size, audio_track_beginning, audio_track_end, custom_name FROM audio WHERE id = ?6",
            params![new_id, new_source_id, old_dir, new_dir, new_project_id, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- previews ---
    for (old_id, old_source_id) in query_id_fk(
        &tx,
        "SELECT id, source_file_id FROM previews WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        let new_source_id = remap(&id_map, &old_source_id);
        tx.execute(
            "INSERT INTO previews (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color) SELECT ?1, folder_name, REPLACE(folder_path, ?3, ?4), frame_count, ?2, ?5, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color FROM previews WHERE id = ?6",
            params![new_id, new_source_id, old_dir, new_dir, new_project_id, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- timelines ---
    for old_id in query_ids(
        &tx,
        "SELECT timeline_id FROM timelines WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO timelines (timeline_id, project_id, creation_date, last_updated) SELECT ?1, ?2, creation_date, last_updated FROM timelines WHERE timeline_id = ?3",
            params![new_id, new_project_id, old_id],
        )?;
        id_map.insert(old_id, new_id);
    }

    // --- clips ---
    for (old_id, old_timeline_id, old_resource_id) in query_id_fk2(
        &tx,
        "SELECT clip_id, timeline_id, actual_resource_id FROM clips WHERE project_id = ?1",
        old_project_id,
    )? {
        let new_id = Uuid::new_v4().to_string();
        let new_timeline_id = remap(&id_map, &old_timeline_id);
        let new_resource_id = remap(&id_map, &old_resource_id);
        tx.execute(
            "INSERT INTO clips (clip_id, project_id, timeline_id, order_index, media_type, resource_kind, actual_resource_id, frame_render_mode, length_seconds, creation_date, last_updated) SELECT ?1, ?2, ?3, order_index, media_type, resource_kind, ?4, frame_render_mode, length_seconds, creation_date, last_updated FROM clips WHERE clip_id = ?5",
            params![new_id, new_project_id, new_timeline_id, new_resource_id, old_id],
        )?;
    }

    // --- project_content ---
    let content_rows = query_id_fk(
        &tx,
        "SELECT id, item_id FROM project_content WHERE project_id = ?1",
        old_project_id,
    )?;
    for (old_id, old_item_id) in &content_rows {
        let new_id = Uuid::new_v4().to_string();
        let new_item_id = remap(&id_map, old_item_id);
        tx.execute(
            "INSERT INTO project_content (id, project_id, item_id, item_name, path, type, creation_date, last_modified) SELECT ?1, ?2, ?3, item_name, path, type, creation_date, last_modified FROM project_content WHERE id = ?4",
            params![new_id, new_project_id, new_item_id, old_id],
        )?;
    }

    tx.commit()?;

    Ok(Project {
        id: new_project_id,
        project_name: new_name.to_string(),
        project_type: ProjectType::from_string(&project_type_str),
        project_path: new_project_path.to_string(),
        size,
        frames,
        creation_date: now,
        last_modified: now,
    })
}
