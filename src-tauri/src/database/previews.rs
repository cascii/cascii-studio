use super::{
    default_output_mode, delete_project_content_rows_for_item, init_database, preview_display_name,
    rename_project_content_rows_for_item, Preview, PreviewSettings, ProjectContentType,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Result as SqlResult};

pub fn add_preview(preview: &Preview) -> SqlResult<()> {
    let conn = init_database()?;

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
         ORDER BY creation_date DESC",
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
         LIMIT 1",
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

pub fn get_preview_by_folder_path(folder_path: &str) -> SqlResult<Option<Preview>> {
    let conn = init_database()?;
    let mut stmt = conn.prepare(
        "SELECT id, folder_name, folder_path, frame_count, source_file_id, project_id, luminance, font_ratio, columns, fps, color, creation_date, total_size, custom_name, output_mode, foreground_color, background_color
         FROM previews
         WHERE folder_path = ?1
         LIMIT 1",
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
