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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiConversion {
    pub id: String,
    pub folder_name: String,       // Name of the frames folder
    pub folder_path: String,       // Full path to the frames folder
    pub frame_count: i32,          // Number of frames
    pub source_file_id: String,    // Foreign key to source_content
    pub project_id: String,        // Foreign key to projects
    pub settings: ConversionSettings, // Conversion settings (luminance, font_ratio, columns, fps)
    pub creation_date: DateTime<Utc>,
    pub total_size: i64,           // Total size of all frame files in bytes
    pub custom_name: Option<String>, // Custom display name for the frame directory
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
    let column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('source_content') WHERE name='custom_name'",
        [],
        |row| row.get(0),
    ).unwrap_or(0) > 0;

    if !column_exists {
        // Add custom_name column for existing databases
        conn.execute(
            "ALTER TABLE source_content ADD COLUMN custom_name TEXT",
            [],
        )?;
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
    let column_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='frame_speed'",
        [],
        |row| row.get(0),
    ).unwrap_or(0) > 0;

    if !column_exists {
        // Add frame_speed column for existing databases
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN frame_speed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;

        // Update existing records to set frame_speed = fps
        conn.execute(
            "UPDATE ascii_conversions SET frame_speed = fps",
            [],
        )?;
    }

    // Check if custom_name column exists, if not add it
    let column_exists = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='custom_name'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0) > 0;

    if !column_exists {
        // Add custom_name column for existing databases
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN custom_name TEXT",
            [],
        )?;
    }

    // Create indexes for ascii_conversions
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_project_id ON ascii_conversions(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_source_id ON ascii_conversions(source_file_id)",
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
        "SELECT id, content_type, project_id, date_added, size, file_path, custom_name 
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
            custom_name: row.get(6)?,
        })
    })?.collect::<SqlResult<Vec<_>>>()?;

    Ok(sources)
}

pub fn update_source_custom_name(source_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    let conn = init_database()?;
    
    conn.execute(
        "UPDATE source_content SET custom_name = ?1 WHERE id = ?2",
        params![custom_name, source_id],
    )?;

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

pub fn delete_project(project_id: &str) -> SqlResult<()> {
    let conn = init_database()?;
    
    // Delete all ascii conversions first
    conn.execute(
        "DELETE FROM ascii_conversions WHERE project_id = ?1",
        [project_id],
    )?;
    
    // Delete all source content (should be handled by CASCADE, but being explicit)
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

pub fn add_ascii_conversion(conversion: &AsciiConversion) -> SqlResult<()> {
    let conn = init_database()?;
    
    conn.execute(
        "INSERT INTO ascii_conversions (id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            conversion.id,
            conversion.folder_name,
            conversion.folder_path,
            conversion.frame_count,
            conversion.source_file_id,
            conversion.project_id,
            conversion.settings.luminance,
            conversion.settings.font_ratio,
            conversion.settings.columns,
            conversion.settings.fps,
            conversion.settings.frame_speed,
            conversion.creation_date.to_rfc3339(),
            conversion.total_size,
            conversion.custom_name,
        ],
    )?;

    Ok(())
}

pub fn get_project_conversions(project_id: &str) -> SqlResult<Vec<AsciiConversion>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name
         FROM ascii_conversions
         WHERE project_id = ?1
         ORDER BY creation_date DESC"
    )?;

    let conversions = stmt.query_map([project_id], |row| {
        let date_str: String = row.get(10)?;
        
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
            },
            creation_date: DateTime::parse_from_rfc3339(&date_str)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc),
            total_size: row.get(12)?,
            custom_name: row.get(13)?,
        })
    })?.collect::<SqlResult<Vec<_>>>()?;

    Ok(conversions)
}

pub fn update_conversion_frame_speed(conversion_id: &str, frame_speed: u32) -> SqlResult<()> {
    println!("📝 DB: Updating frame_speed for conversion {} to {}", conversion_id, frame_speed);
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
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, frame_speed, creation_date, total_size, custom_name
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

pub fn update_conversion_custom_name(conversion_id: &str, custom_name: Option<String>) -> SqlResult<()> {
    println!("📝 DB: Updating conversion custom_name for {} to {:?}", conversion_id, custom_name);
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
    Ok(())
}
