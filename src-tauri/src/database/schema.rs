use super::migrations::migrate_explorer_layout_to_project_content;
use rusqlite::{Connection, Result as SqlResult};

pub(crate) fn init_schema(conn: &Connection) -> SqlResult<()> {
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

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('source_content') WHERE name='custom_name'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute("ALTER TABLE source_content ADD COLUMN custom_name TEXT", [])?;
    }

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_source_project_id ON source_content(project_id)",
        [],
    )?;

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

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='frame_speed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN frame_speed INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
        conn.execute("UPDATE ascii_conversions SET frame_speed = fps", [])?;
    }

    let column_exists = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='custom_name'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute(
            "ALTER TABLE ascii_conversions ADD COLUMN custom_name TEXT",
            [],
        )?;
    }

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('ascii_conversions') WHERE name='color'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
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

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_project_id ON ascii_conversions(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_conversion_source_id ON ascii_conversions(source_file_id)",
        [],
    )?;

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

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cuts_project_id ON cuts(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_cuts_source_id ON cuts(source_file_id)",
        [],
    )?;

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

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audio_project_id ON audio(project_id)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_audio_source_id ON audio(source_file_id)",
        [],
    )?;

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

    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('clips') WHERE name='clip_speed_mode'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
        > 0;

    if !column_exists {
        conn.execute("ALTER TABLE clips ADD COLUMN clip_speed_mode TEXT", [])?;
    }

    migrate_explorer_layout_to_project_content(conn)?;

    Ok(())
}
