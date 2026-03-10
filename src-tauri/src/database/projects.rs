use super::{init_database, Project, ProjectType};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

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
