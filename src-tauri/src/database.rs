use rusqlite::{Connection, Result as SqlResult, params};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

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
            FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
        )",
        [],
    )?;

    // Create index on project_id for faster queries
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_source_project_id ON source_content(project_id)",
        [],
    )?;

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
        "INSERT INTO source_content (id, content_type, project_id, date_added, size, file_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source.id,
            source.content_type.to_string(),
            source.project_id,
            source.date_added.to_rfc3339(),
            source.size,
            source.file_path,
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

    let projects = stmt.query_map([], |row| {
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
    })?.collect::<SqlResult<Vec<_>>>()?;

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
        "SELECT id, content_type, project_id, date_added, size, file_path 
         FROM source_content 
         WHERE project_id = ?1 
         ORDER BY date_added ASC"
    )?;

    let sources = stmt.query_map([project_id], |row| {
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
        })
    })?.collect::<SqlResult<Vec<_>>>()?;

    Ok(sources)
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

pub fn delete_project(project_id: &str) -> SqlResult<()> {
    let conn = init_database()?;
    
    // Delete all source content first (should be handled by CASCADE, but being explicit)
    conn.execute(
        "DELETE FROM source_content WHERE project_id = ?1",
        [project_id],
    )?;
    
    // Delete the project
    conn.execute(
        "DELETE FROM projects WHERE id = ?1",
        [project_id],
    )?;

    Ok(())
}

